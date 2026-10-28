//! plugged.host: standing rules outside watched channels ("if anyone asks for new locations, give
//! them a warn").
//!
//! Reading every message with the AI would use up its capacity within minutes, so this works in
//! two steps. The event worker only compares each member message with the active rules' keywords
//! ([`screen`]; the rules are kept in memory and reloaded every 30 seconds, so new ones apply
//! without a restart). A hit goes to a task of its own ([`check`]), which skips staff, asks the AI
//! whether the message really breaks the rule, and only then acts. A member's messages reach the
//! AI at most three times a minute, a few checks run at once, and one rule acts on one member at
//! most once per cooldown, so a burst of messages or a wrong answer cannot pile up warnings.
//! Rules belong to one server: a rule only ever checks and acts on messages there.

use super::{
    Incoming,
    judge::{self, Asked},
};
use crate::rux::{
    access,
    config::Config,
    context::Me,
    discord::Rest,
    models::{RULE_COLUMNS, Rule},
    roles::Names,
    tools::rules::{
        enforce::{self, Hit},
        facts, model,
    },
};
use serde_json::Value;
use std::{
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

/// The active rules are read again after this long.
const RELOAD: Duration = Duration::from_secs(30);
/// A member's messages that reach the AI per minute, at most.
const CHECKS_PER_MINUTE: u64 = 3;
/// Rule checks running at once, at most (the others wait their turn).
const AT_ONCE: usize = 4;
/// How long a check waits for its turn before it is dropped.
const TURN_WAIT: Duration = Duration::from_secs(60);
/// How long the event worker waits for the rules from the database.
const LOAD_WAIT: Duration = Duration::from_secs(5);

struct Memory {
    loaded: Option<Instant>,
    /// Every served server's active rules.
    rules: Arc<Vec<Rule>>,
}

static RULES: LazyLock<parking_lot::Mutex<Memory>> = LazyLock::new(|| {
    parking_lot::Mutex::new(Memory {
        loaded: None,
        rules: Arc::new(Vec::new()),
    })
});
static TURNS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(AT_ONCE);

/// The active rules of every server rux works on, at most [`RELOAD`] old. When they cannot be
/// read, the last ones stay.
async fn active(state: &crate::State, config: &Config) -> Arc<Vec<Rule>> {
    {
        let memory = RULES.lock();
        if memory.loaded.is_some_and(|at| at.elapsed() < RELOAD) {
            return memory.rules.clone();
        }
    }

    let guilds: Vec<String> = config.guilds.iter().map(|guild| guild.id.clone()).collect();
    let loaded = tokio::time::timeout(
        LOAD_WAIT,
        sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
            "SELECT {RULE_COLUMNS} FROM rux_rules WHERE active AND guild_id = ANY($1) ORDER BY id"
        )))
        .bind(&guilds)
        .fetch_all(state.database.read()),
    )
    .await;

    let mut memory = RULES.lock();
    match loaded {
        Ok(Ok(rules)) => memory.rules = Arc::new(rules),
        Ok(Err(err)) => tracing::warn!("rux: could not read the standing rules: {err}"),
        Err(_) => tracing::warn!("rux: reading the standing rules took too long"),
    }
    memory.loaded = Some(Instant::now());

    memory.rules.clone()
}

/// The rules of `guild_id` whose keywords a message in `channel_id` hits.
pub fn hits(rules: &[Rule], guild_id: &str, channel_id: &str, content: &str) -> Vec<i64> {
    let text = model::normalize(content);

    rules
        .iter()
        .filter(|rule| {
            rule.guild_id == guild_id && rule.applies_in(channel_id, None) && rule.matches(&text)
        })
        .map(|rule| rule.id)
        .collect()
}

/// In the event worker: a member's message that hits a rule's keywords gets its check, in a
/// task of its own (at most [`CHECKS_PER_MINUTE`] a minute per member).
pub async fn screen(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    me: &Me,
    message: &Incoming,
    raw: &Value,
) -> Result<(), anyhow::Error> {
    if message.content.trim().is_empty() {
        return Ok(());
    }
    let rules = active(state, config).await;
    let ids = hits(
        &rules,
        &message.guild_id,
        &message.channel_id,
        &message.content,
    );
    if ids.is_empty() {
        return Ok(());
    }

    if state
        .cache
        .ratelimit(
            "rux::watch::rule_checks",
            CHECKS_PER_MINUTE + 1,
            60,
            &message.author_id,
        )
        .await
        .is_err()
    {
        return Ok(());
    }

    let hit = Hit {
        guild_id: message.guild_id.clone(),
        message_id: message.message_id.clone(),
        channel_id: message.channel_id.clone(),
        author_id: message.author_id.clone(),
        author_name: message.author_name.clone(),
        roles: Some(message.roles.clone()),
        content: Some(crate::rux::text::cut(message.content.trim(), 1500, "")),
    };
    let replied_to = model::replied_to(raw);
    let (state, config, rest, me) = (
        state.clone(),
        Arc::new(config.clone()),
        rest.clone(),
        me.clone(),
    );
    tokio::spawn(async move {
        check(&state, config, rest, me, hit, ids, replied_to).await;
    });

    Ok(())
}

/// In its own task: asks the AI whether the message breaks the rules it hit, one after the
/// other, and acts on the first it clearly breaks. Staff are left alone; a rule that acted on
/// this member within the cooldown is not asked about again.
async fn check(
    state: &crate::State,
    config: Arc<Config>,
    rest: Rest,
    me: Me,
    hit: Hit,
    rule_ids: Vec<i64>,
    replied_to: Option<String>,
) {
    let Ok(Ok(_turn)) = tokio::time::timeout(TURN_WAIT, TURNS.acquire()).await else {
        tracing::info!(
            message = %hit.message_id,
            "rux: too many rule checks at once; a message was not checked"
        );
        return;
    };
    let Some(guild) = config.guild(&hit.guild_id) else {
        return;
    };
    if access::is_staff(
        state,
        &rest,
        &guild.id,
        &hit.author_id,
        hit.roles.as_deref(),
    )
    .await
    {
        return;
    }

    let mut facts: Option<Vec<String>> = None;
    // the server's roles, to name the author's and the pinged ones: the staff check just read
    // them, so they come from the cache
    let names = Names::load(&rest, &guild.id).await;
    for rule_id in rule_ids {
        // only the rules of the server the message was written on, still active
        let rule = match super::rule(state, rule_id, &guild.id).await {
            Ok(Some(rule)) => rule,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(rule = rule_id, "rux: could not read a rule: {err}");
                continue;
            }
        };
        super::count_hit(state, rule.id).await;

        let cooldown = super::rule_cooldown(rule.id, &hit.author_id);
        if super::cooling(state, &cooldown).await {
            continue;
        }

        if facts.is_none() {
            facts = Some(facts::lines(state, &config, Some(guild)).await);
        }
        let prompt = judge::rule_prompt(
            &config.settings.name,
            guild,
            &rule,
            facts.as_deref().unwrap_or_default(),
            &hit,
            replied_to.as_deref(),
            &names,
        );
        let verdict = match judge::ask(state, &prompt, Duration::ZERO).await {
            Asked::Answer(answer) => judge::verdict(&answer),
            Asked::Busy(retry_after) => {
                tracing::info!(
                    rule = rule.id,
                    retry_after,
                    "rux: rule check: the AI is busy; the message was not checked"
                );
                return;
            }
            Asked::Unavailable(reason) => {
                tracing::warn!(
                    rule = rule.id,
                    "rux: rule check: the AI is unavailable ({reason}); the message was not checked"
                );
                return;
            }
        };

        tracing::info!(
            rule = rule.id,
            member = %hit.author_id,
            message = %hit.message_id,
            breaks = ?verdict.as_ref().map(|(breaks, _)| *breaks),
            why = verdict.as_ref().map_or("", |(_, why)| why.as_str()),
            "rux: rule check"
        );
        if !verdict.is_some_and(|(breaks, _)| breaks) {
            continue;
        }
        if !super::cool_down(state, &config, &cooldown).await {
            continue;
        }

        if let Err(err) =
            enforce::enforce(state, config.clone(), rest.clone(), me.clone(), &rule, &hit).await
        {
            tracing::warn!(
                rule = rule.id,
                member = %hit.author_id,
                "rux: a rule's action failed: {err}"
            );
        }

        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keywords_pick_the_rules_of_the_server_and_channel() {
        let base = crate::rux::tools::rules::model::tests::rule();
        let mut elsewhere = base.clone();
        elsewhere.id = 5;
        elsewhere.guild_id = "100000000000000002".into();
        let mut only_there = base.clone();
        only_there.id = 6;
        only_there.channel_ids = Some(json!(["100000000000000007"]));
        let rules = vec![base, elsewhere, only_there];

        assert_eq!(
            hits(
                &rules,
                "100000000000000001",
                "100000000000000009",
                "Any  new COUNTRY soon?"
            ),
            vec![4]
        );
        assert_eq!(
            hits(
                &rules,
                "100000000000000001",
                "100000000000000007",
                "singapore when"
            ),
            vec![4, 6]
        );
        assert!(
            hits(
                &rules,
                "100000000000000001",
                "100000000000000007",
                "relocation"
            )
            .is_empty()
        );
        assert_eq!(
            hits(
                &rules,
                "100000000000000002",
                "100000000000000009",
                "Location?"
            ),
            vec![5]
        );
    }
}
