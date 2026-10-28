//! plugged.host: watched channels: every member message there is checked by the AI, not only the
//! ones that hit a standing rule's keywords.
//!
//! The event worker only queues them in `rux_watch_items` ([`enqueue`]). One task at a time
//! ([`drain`]) checks them in batches of up to [`BATCH`] per AI call against the server's
//! standing rules and the always-on guidelines, so a busy channel costs a fraction of one call per
//! message; a run starts at most every [`DRAIN_GAP`] (later messages wait for the next one) and
//! the `rux` task starts one every minute while messages wait. The AI's limits can still be
//! reached: a batch the busy AI could not take stays queued for the next run, messages older than
//! ten minutes are not checked any more, and when no AI can answer at all watching pauses for ten
//! minutes. Staff are never checked. Watched channels, the queue and the batches are per server:
//! a batch only holds one server's messages and is judged by that server's rules and facts.

use super::{
    Incoming,
    judge::{self, Asked, Break, Code},
};
use crate::rux::{
    access,
    config::{Config, Guild},
    context::Me,
    discord::Rest,
    models::{RULE_COLUMNS, Rule, WATCH_ITEM_COLUMNS, WatchItem},
    roles::Names,
    tools::rules::{
        enforce::{self, Hit},
        facts, model, watches,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicI64, Ordering},
    },
    time::{Duration, Instant},
};

/// Messages per AI call, at most.
pub const BATCH: i64 = 20;
/// Messages that waited longer are not checked any more.
pub const MAX_AGE_MINUTES: i32 = 10;
/// A run checks batches for this long (and finishes the batch it is on).
const DRAIN_TIME: Duration = Duration::from_secs(110);
/// After [`DRAIN_TIME`], the messages of the last batch may still be checked one by one this
/// long.
const GRACE: Duration = Duration::from_secs(60);
/// Two runs start at least this far apart.
const DRAIN_GAP: Duration = Duration::from_secs(10);
/// A busy AI is waited for this long at most per batch.
const BUSY_WAIT: Duration = Duration::from_secs(60);
/// The run's lock: longer than a run can take.
const LOCK_SECONDS: u64 = 300;
/// How long watching pauses when no AI can answer.
const PAUSE_SECONDS: i64 = 600;
const PAUSE_KEY: &str = "rux::watch::paused";
/// A server's watched channels are looked up again after this long (the tools' cache behind it
/// is forgotten whenever a watch changes).
const FRESH: Duration = Duration::from_secs(15);
/// How long the event worker waits for the watched channels or the queue.
const WAIT: Duration = Duration::from_secs(3);

// --- in the event worker -------------------------------------------------------------------

/// Watching is paused until then (unix seconds); kept in the cache too ([`paused`]).
static PAUSED_UNTIL: AtomicI64 = AtomicI64::new(0);

fn paused_here() -> bool {
    chrono::Utc::now().timestamp() < PAUSED_UNTIL.load(Ordering::Relaxed)
}

/// Whether watching is paused (no AI could answer; it tries again within ten minutes).
pub async fn paused(state: &crate::State) -> bool {
    paused_here() || state.cache.exists(PAUSE_KEY).await.unwrap_or(false)
}

#[derive(Default)]
struct Watched {
    /// server id => when its ids were looked up, the ids
    guilds: HashMap<String, (Instant, Vec<String>)>,
    refreshing: HashSet<String>,
}

static WATCHED: LazyLock<parking_lot::Mutex<Watched>> = LazyLock::new(Default::default);

/// Looks a server's watched channels up and keeps them; on an error the last ones stay.
async fn load_watched(state: &crate::State, rest: &Rest, guild_id: &str) -> Vec<String> {
    let found = match watches::watched_channel_ids(state, rest, guild_id, false).await {
        Ok(ids) => Some(ids),
        Err(err) => {
            tracing::warn!(server = %guild_id, "rux: could not read the watched channels: {err}");
            None
        }
    };

    let mut memory = WATCHED.lock();
    memory.refreshing.remove(guild_id);
    let entry = memory
        .guilds
        .entry(guild_id.to_string())
        .or_insert_with(|| (Instant::now(), Vec::new()));
    entry.0 = Instant::now();
    if let Some(ids) = found {
        entry.1 = ids;
    }

    entry.1.clone()
}

/// A server's watched channel ids, from memory. Older than [`FRESH`]: the known ones now, looked
/// up again in the background. The first time: looked up here, [`WAIT`] at most.
async fn watched_ids(state: &crate::State, rest: &Rest, guild_id: &str) -> Vec<String> {
    let known = {
        let mut memory = WATCHED.lock();
        match memory.guilds.get(guild_id) {
            Some((at, ids)) => {
                let ids = ids.clone();
                let stale = at.elapsed() >= FRESH;
                Some((ids, stale && memory.refreshing.insert(guild_id.to_string())))
            }
            None => None,
        }
    };

    match known {
        Some((ids, refresh)) => {
            if refresh {
                let (state, rest, guild_id) = (state.clone(), rest.clone(), guild_id.to_string());
                tokio::spawn(async move {
                    load_watched(&state, &rest, &guild_id).await;
                });
            }
            ids
        }
        None => match tokio::time::timeout(WAIT, load_watched(state, rest, guild_id)).await {
            Ok(ids) => ids,
            Err(_) => {
                // try again in the background later; until then nothing here is watched
                WATCHED
                    .lock()
                    .guilds
                    .entry(guild_id.to_string())
                    .or_insert_with(|| (Instant::now(), Vec::new()));
                Vec::new()
            }
        },
    }
}

/// In the event worker: queues a member's message from a watched channel. `false` when the
/// channel is not watched (the keyword rules apply then); `true` also while watching is paused
/// (the message is then not checked).
pub async fn enqueue(
    state: &crate::State,
    rest: &Rest,
    me: &Me,
    message: &Incoming,
    raw: &Value,
) -> bool {
    let content = message.content.trim();
    if content.is_empty()
        || !watched_ids(state, rest, &message.guild_id)
            .await
            .contains(&message.channel_id)
    {
        return false;
    }
    if paused_here() {
        return true;
    }

    let insert = sqlx::query(
        "INSERT INTO rux_watch_items (guild_id, message_id, channel_id, author_id, author_name, roles, content, replied_to)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&message.guild_id)
    .bind(&message.message_id)
    .bind(&message.channel_id)
    .bind(&message.author_id)
    .bind(&message.author_name)
    .bind(json!(message.roles))
    .bind(crate::rux::text::cut(content, 1500, ""))
    .bind(model::replied_to(raw))
    .execute(state.database.write());
    match tokio::time::timeout(WAIT, insert).await {
        Ok(Ok(_)) => nudge(state, me),
        Ok(Err(err)) => tracing::warn!("rux: could not queue a watched message: {err}"),
        Err(_) => tracing::warn!("rux: queueing a watched message took too long"),
    }

    true
}

// --- the runs ------------------------------------------------------------------------------

#[derive(Default)]
struct Drains {
    /// A run is going or about to start.
    running: bool,
    /// Messages came in since the run started.
    wanted: bool,
    started: Option<Instant>,
    /// The AI said it is busy until then.
    retry_at: Option<Instant>,
}

impl Drains {
    /// How long the next run waits: [`DRAIN_GAP`] after the last start, and past a busy AI.
    fn wait(&self) -> Duration {
        let now = Instant::now();
        let gap = self.started.map_or(Duration::ZERO, |at| {
            (at + DRAIN_GAP).saturating_duration_since(now)
        });
        let busy = self
            .retry_at
            .map_or(Duration::ZERO, |at| at.saturating_duration_since(now));

        gap.max(busy)
    }
}

static DRAINS: LazyLock<parking_lot::Mutex<Drains>> = LazyLock::new(Default::default);

/// Sets `running` back if a run ends unexpectedly (a panic), so the next message starts one.
struct Running {
    armed: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.armed {
            DRAINS.lock().running = false;
        }
    }
}

/// Has the queue checked: starts a run unless one is going (that one runs again for what came
/// in meanwhile).
pub fn nudge(state: &crate::State, me: &Me) {
    let delay = {
        let mut drains = DRAINS.lock();
        drains.wanted = true;
        if drains.running {
            return;
        }
        drains.running = true;
        drains.wait()
    };

    let (state, me) = (state.clone(), me.clone());
    tokio::spawn(async move {
        let mut running = Running { armed: true };
        let mut delay = delay;
        loop {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            {
                let mut drains = DRAINS.lock();
                drains.wanted = false;
                drains.started = Some(Instant::now());
            }

            let busy = drain(&state, &me).await;

            let mut drains = DRAINS.lock();
            drains.retry_at = busy.map(|retry_after| Instant::now() + retry_after);
            if !drains.wanted {
                drains.running = false;
                running.armed = false;
                return;
            }
            delay = drains.wait();
        }
    });
}

/// Every minute (`rux::tick`): forgets messages that waited too long, and starts a run when
/// messages wait.
pub async fn tick(state: &crate::State) -> Result<(), anyhow::Error> {
    prune(state).await?;

    let waiting: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM rux_watch_items)")
        .fetch_one(state.database.read())
        .await?;
    // the bot's ids come with the gateway's READY; the messages wait until then
    if waiting && let Some(me) = crate::rux::me() {
        nudge(state, &me);
    }

    Ok(())
}

async fn prune(state: &crate::State) -> Result<(), sqlx::Error> {
    let stale = sqlx::query(
        "DELETE FROM rux_watch_items WHERE created < NOW() - make_interval(mins => $1)",
    )
    .bind(MAX_AGE_MINUTES)
    .execute(state.database.write())
    .await?
    .rows_affected();
    if stale > 0 {
        tracing::info!(
            stale,
            "rux: watch: messages waited too long and were not checked"
        );
    }

    Ok(())
}

async fn forget(state: &crate::State, ids: &[i64]) {
    if let Err(err) = sqlx::query("DELETE FROM rux_watch_items WHERE id = ANY($1)")
        .bind(ids)
        .execute(state.database.write())
        .await
    {
        tracing::warn!("rux: watch: could not take checked messages off the queue: {err}");
    }
}

/// No AI can answer: watching pauses for ten minutes and the queue is dropped.
async fn pause(state: &crate::State, reason: &str) {
    tracing::warn!("rux: watch: the AI is unavailable ({reason}); watching paused for 10 minutes");
    PAUSED_UNTIL.store(
        chrono::Utc::now().timestamp() + PAUSE_SECONDS,
        Ordering::Relaxed,
    );
    let _ = state
        .cache
        .set(PAUSE_KEY, PAUSE_SECONDS as u64, &true)
        .await;
    if let Err(err) = sqlx::query("DELETE FROM rux_watch_items")
        .execute(state.database.write())
        .await
    {
        tracing::warn!("rux: watch: could not drop the queue: {err}");
    }
}

/// One run over the queue: batches until it is empty or [`DRAIN_TIME`] has passed. `Some(retry
/// after)` when it stopped at a busy AI (the batch stays queued).
async fn drain(state: &crate::State, me: &Me) -> Option<Duration> {
    let Ok(_lock) = state
        .cache
        .lock("rux::watch::drain", Some(LOCK_SECONDS), Some(100))
        .await
    else {
        return None;
    };
    if paused(state).await {
        return None;
    }

    let until = Instant::now() + DRAIN_TIME;
    while Instant::now() < until {
        let (config, rest) = match crate::rux::running(state).await {
            Ok(Some(running)) => running,
            Ok(None) => return None,
            Err(err) => {
                tracing::warn!("rux: watch: could not read the settings: {err:#}");
                return None;
            }
        };
        if let Err(err) = prune(state).await {
            tracing::warn!("rux: watch: could not drop old messages: {err}");
        }

        let batch: Vec<WatchItem> = match sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {WATCH_ITEM_COLUMNS} FROM rux_watch_items ORDER BY id LIMIT $1"
        )))
        .bind(BATCH)
        // the primary: a replica could still show the batch just taken off
        .fetch_all(state.database.write())
        .await
        {
            Ok(batch) => batch,
            Err(err) => {
                tracing::warn!("rux: watch: could not read the queue: {err}");
                return None;
            }
        };
        if batch.is_empty() {
            return None;
        }

        // one server's messages per check: its rules, its facts, its staff
        let (groups, unserved) = by_guild(batch, &config);
        if !unserved.is_empty() {
            forget(state, &unserved).await;
        }
        let run = Run {
            state,
            config: &config,
            rest: &rest,
            me,
            until,
        };
        for (guild_id, items) in groups {
            let Some(guild) = config.guild(&guild_id) else {
                continue;
            };
            match check(&run, guild, items).await {
                Flow::Next => {}
                Flow::Busy(retry_after) => return Some(retry_after),
                Flow::Stop => return None,
            }
        }
    }

    None
}

/// The queued messages by server (rows without one are the main server's), and the ids of those
/// whose server rux does not work on any more.
fn by_guild(batch: Vec<WatchItem>, config: &Config) -> (Vec<(String, Vec<WatchItem>)>, Vec<i64>) {
    let mut groups: Vec<(String, Vec<WatchItem>)> = Vec::new();
    let mut unserved = Vec::new();
    for item in batch {
        let Some(guild) = config.guild_for(item.guild_id.as_deref()) else {
            unserved.push(item.id);
            continue;
        };
        match groups.iter_mut().find(|(id, _)| *id == guild.id) {
            Some((_, items)) => items.push(item),
            None => groups.push((guild.id.clone(), vec![item])),
        }
    }

    (groups, unserved)
}

struct Run<'a> {
    state: &'a crate::State,
    config: &'a Arc<Config>,
    rest: &'a Rest,
    me: &'a Me,
    until: Instant,
}

impl Run<'_> {
    fn left(&self) -> Duration {
        self.until.saturating_duration_since(Instant::now())
    }
}

enum Flow {
    Next,
    /// The AI is busy: the batch stays queued.
    Busy(Duration),
    /// No AI can answer (watching paused), or the queue cannot be read.
    Stop,
}

/// The server's active rules.
async fn guild_rules(state: &crate::State, guild_id: &str) -> Result<Vec<Rule>, sqlx::Error> {
    sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
        "SELECT {RULE_COLUMNS} FROM rux_rules WHERE guild_id = $1 AND active ORDER BY id"
    )))
    .bind(guild_id)
    .fetch_all(state.database.read())
    .await
}

/// Checks one server's queued messages (one batch) and acts on what breaks a rule.
async fn check(run: &Run<'_>, guild: &Guild, items: Vec<WatchItem>) -> Flow {
    let (state, config) = (run.state, run.config);
    let ids: Vec<i64> = items.iter().map(|item| item.id).collect();

    // staff are never checked
    let mut staff: HashMap<String, bool> = HashMap::new();
    let mut members: Vec<(WatchItem, Hit)> = Vec::new();
    for item in items {
        let Some(hit) = Hit::from_item(&item, config) else {
            continue;
        };
        let is_staff = match staff.get(&hit.author_id) {
            Some(is_staff) => *is_staff,
            None => {
                let is_staff = access::is_staff(
                    state,
                    run.rest,
                    &guild.id,
                    &hit.author_id,
                    hit.roles.as_deref(),
                )
                .await;
                staff.insert(hit.author_id.clone(), is_staff);
                is_staff
            }
        };
        if !is_staff {
            members.push((item, hit));
        }
    }
    if members.is_empty() {
        forget(state, &ids).await;
        return Flow::Next;
    }

    let rules = match guild_rules(state, &guild.id).await {
        Ok(rules) => rules,
        Err(err) => {
            tracing::warn!("rux: watch: could not read the standing rules: {err}");
            return Flow::Stop;
        }
    };
    let guidelines = config.settings.watch_guidelines;
    let facts = facts::lines(state, config, Some(guild)).await;
    // the server's roles, to name the authors' and the pinged ones: the staff checks just read
    // them, so they come from the cache
    let names = Names::load(run.rest, &guild.id).await;
    let prompt_for = |items: &[&WatchItem]| {
        judge::batch_prompt(
            &config.settings.name,
            guild,
            &rules,
            guidelines,
            &facts,
            items,
            &names,
        )
    };

    let all: Vec<&WatchItem> = members.iter().map(|(item, _)| item).collect();
    let Some(prompt) = prompt_for(all.as_slice()) else {
        // no rules and no guidelines: nothing to check against
        forget(state, &ids).await;
        return Flow::Next;
    };
    let found = match judge::ask(state, &prompt, BUSY_WAIT.min(run.left())).await {
        Asked::Answer(answer) => judge::breaks(&answer, all.len(), guidelines),
        Asked::Busy(retry_after) => {
            tracing::info!(
                messages = all.len(),
                retry_after,
                "rux: watch: the AI is busy; the messages wait for the next run"
            );
            return Flow::Busy(Duration::from_secs(retry_after));
        }
        Asked::Unavailable(reason) => {
            pause(state, &reason).await;
            return Flow::Stop;
        }
    };
    forget(state, &ids).await;
    let Some(found) = found else {
        tracing::warn!(
            messages = all.len(),
            "rux: watch: the AI's verdict was unreadable; the messages were not checked"
        );
        return Flow::Next;
    };

    let mut acted: HashSet<i64> = HashSet::new();
    for verdict in found {
        let Some((item, hit)) = members.get(verdict.n - 1) else {
            continue;
        };
        // one action per message per batch
        if !acted.insert(item.id) {
            continue;
        }

        // Other members' messages were in the same prompt and can steer verdicts onto this one
        // (prompt injection), so act only when the message is flagged on its own too.
        let verdict = if members
            .iter()
            .any(|(other, _)| other.author_id != item.author_id)
        {
            if Instant::now() >= run.until + GRACE {
                tracing::info!(
                    message = %item.message_id,
                    "rux: watch: out of time to confirm a verdict; the message was not acted on"
                );
                continue;
            }
            let Some(alone) = prompt_for(std::slice::from_ref(&item)) else {
                continue;
            };
            let wait = BUSY_WAIT.min(run.left()).max(Duration::from_secs(5));
            match judge::ask(state, &alone, wait).await {
                Asked::Answer(answer) => {
                    match judge::breaks(&answer, 1, guidelines)
                        .and_then(|found| found.into_iter().next())
                    {
                        Some(verdict) => verdict,
                        None => continue,
                    }
                }
                Asked::Busy(_) => continue,
                Asked::Unavailable(reason) => {
                    pause(state, &reason).await;
                    return Flow::Stop;
                }
            }
        } else {
            verdict
        };

        act(run, guild, hit, &verdict).await;
    }

    Flow::Next
}

/// The cooldown's key of a guideline (per server, guideline and member).
fn guideline_cooldown(guild_id: &str, code: &str, member: &str) -> String {
    format!("{guild_id}:{code}:{member}")
}

/// The line a guideline's breaker is told (`STRINGS`).
pub fn guideline_line(code: &str) -> String {
    format!("watch.{}", code.to_lowercase())
}

/// Acts on a message found breaking a rule or a guideline: once per cooldown per member (and
/// rule, or server and guideline).
async fn act(run: &Run<'_>, guild: &Guild, hit: &Hit, verdict: &Break) {
    let (state, config) = (run.state, run.config);
    let cooldown = match &verdict.code {
        Code::Rule(id) => super::rule_cooldown(*id, &hit.author_id),
        Code::Guideline(code) => guideline_cooldown(&guild.id, code, &hit.author_id),
    };
    if !super::cool_down(state, config, &cooldown).await {
        return;
    }

    tracing::info!(
        rule = %verdict.code.label(),
        member = %hit.author_id,
        message = %hit.message_id,
        why = %verdict.why,
        "rux: watch: acting"
    );

    let acted = match &verdict.code {
        Code::Rule(id) => match super::rule(state, *id, &guild.id).await {
            Ok(Some(rule)) if rule.applies_in(&hit.channel_id, None) => {
                super::count_hit(state, rule.id).await;
                enforce::enforce(
                    state,
                    config.clone(),
                    run.rest.clone(),
                    run.me.clone(),
                    &rule,
                    hit,
                )
                .await
            }
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        },
        Code::Guideline(code) => {
            enforce::enforce_guideline(
                state,
                config.clone(),
                run.rest.clone(),
                run.me.clone(),
                &config.say(&guideline_line(code), &[]),
                hit,
            )
            .await
        }
    };
    if let Err(err) = acted {
        tracing::warn!(
            rule = %verdict.code.label(),
            member = %hit.author_id,
            "rux: watch: the action failed: {err}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rux::watch::judge::tests::item;

    #[test]
    fn batches_are_per_server() {
        let mut config = crate::rux::tools::giveaways::service::tests::config("en");
        config.guilds.push(Guild {
            id: "100000000000000002".into(),
            primary: false,
            ..config.guilds[0].clone()
        });

        let mut other = item(2, "100000000000000006", "b");
        other.guild_id = Some("100000000000000002".into());
        let mut old = item(3, "100000000000000006", "c");
        old.guild_id = None;
        let mut gone = item(4, "100000000000000006", "d");
        gone.guild_id = Some("100000000000000099".into());
        let batch = vec![item(1, "100000000000000006", "a"), other, old, gone];

        let (groups, unserved) = by_guild(batch, &config);
        let groups: Vec<(&str, Vec<i64>)> = groups
            .iter()
            .map(|(id, items)| (id.as_str(), items.iter().map(|item| item.id).collect()))
            .collect();
        assert_eq!(
            groups,
            vec![
                ("100000000000000001", vec![1, 3]),
                ("100000000000000002", vec![2])
            ]
        );
        assert_eq!(unserved, vec![4]);
    }

    #[test]
    fn every_guideline_has_its_line() {
        for (code, _) in model::GUIDELINES {
            let key = guideline_line(code);
            for locale in ["en", "de"] {
                let line = crate::rux::say::say(locale, &key, &[]);
                assert_ne!(line, key, "{key} has no line");
                assert!(!line.contains(':'), "{key} reads like a guideline's text");
            }
        }
        assert_eq!(guideline_line("G1"), "watch.g1");
        assert_eq!(guideline_cooldown("1", "G1", "2"), "1:G1:2");
    }

    #[test]
    fn runs_keep_their_distance() {
        let drains = Drains::default();
        assert_eq!(drains.wait(), Duration::ZERO);

        let drains = Drains {
            started: Some(Instant::now()),
            ..Default::default()
        };
        assert!(drains.wait() > DRAIN_GAP - Duration::from_secs(1));

        let drains = Drains {
            started: Some(Instant::now() - DRAIN_GAP * 2),
            retry_at: Some(Instant::now() + Duration::from_secs(30)),
            ..Default::default()
        };
        assert!(drains.wait() > Duration::from_secs(29));
    }
}
