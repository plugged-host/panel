//! plugged.host: spam, caught without AI in every channel rux reads (the settings' spam limits):
//! flooding, the same text again and again (across channels too, the way hijacked accounts
//! post), mass mentions and emoji walls. The AI checks no longer judge spam: they warned members
//! for a single "." and for a complaint posted twice, and spam has to be caught when the AI is
//! out of quota too.
//!
//! The gateway's event worker only counts ([`screen`]: memory only, no network). What it catches
//! waits per member and goes to a task at most every [`SPAWN_GAP`] seconds, so a flood starts one
//! or two tasks, not one per message. The task ([`act`]) leaves staff alone, deletes the spam and
//! warns the member at most once per rule cooldown; the warning counts towards the warning
//! ladder. Everything is per server: a member's messages on two servers are counted apart, and
//! the warning, the staff check and the cooldown are the server's where the spam was.

use super::Incoming;
use crate::rux::{
    access,
    config::Config,
    context::{self, Me},
    discord::Rest,
    text,
    tools::rules::enforce::{self, Hit},
};
use crate::settings::rux::AppSettingsRux;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

/// Seconds between two tasks for the same member.
pub const SPAWN_GAP: f64 = 3.0;
/// A member's last messages kept, at most.
const RECENT: usize = 60;
/// What the counters know is forgotten after this many seconds.
const FORGET: f64 = 600.0;
/// The counters are tidied at most this often (seconds).
const PRUNE_EVERY: f64 = 10.0;
/// What a caught message keeps of its text (for the case).
const SAID: usize = 300;

static EMOJI: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<a?:\w+:\d+>|\p{Extended_Pictographic}").unwrap());

/// The settings' spam limits, each at least what makes sense (two messages, one second).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub flood_messages: usize,
    pub flood_seconds: f64,
    pub duplicates: usize,
    pub duplicate_seconds: f64,
    pub mentions: usize,
    pub emoji: usize,
}

impl Limits {
    pub fn of(settings: &AppSettingsRux) -> Self {
        let at_least = |value: i32, min: i32| value.max(min) as usize;

        Self {
            flood_messages: at_least(settings.spam_flood_messages, 2),
            flood_seconds: f64::from(settings.spam_flood_seconds.max(1)),
            duplicates: at_least(settings.spam_duplicates, 2),
            duplicate_seconds: f64::from(settings.spam_duplicate_seconds.max(1)),
            mentions: at_least(settings.spam_mentions, 2),
            emoji: at_least(settings.spam_emoji, 2),
        }
    }
}

/// Why messages were taken for spam; the member is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Flood,
    Repeats,
    Mentions,
    Emoji,
}

impl Reason {
    /// Its line (`STRINGS`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Flood => "spam.flood",
            Self::Repeats => "spam.repeats",
            Self::Mentions => "spam.mentions",
            Self::Emoji => "spam.emoji",
        }
    }
}

/// A message taken for spam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spam {
    pub id: String,
    pub channel_id: String,
    /// What it said, cut short.
    pub said: String,
}

/// A member's spam, waiting for its task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caught {
    pub guild_id: String,
    pub member: String,
    pub name: String,
    pub roles: Vec<String>,
    /// Why the first of them was caught.
    pub reason: Reason,
    pub messages: Vec<Spam>,
}

struct Seen {
    at: f64,
    spam: Spam,
    /// [`fingerprint`]
    text: String,
}

/// What [`Tracker::screen`] made of a message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screened {
    Clean,
    /// Spam. `start_in`: start the member's task in this many seconds (`None`: one is already
    /// waiting and takes it along).
    Spam {
        start_in: Option<f64>,
    },
}

/// The counters, per "server:member" (times are seconds on a monotonic clock).
#[derive(Default)]
pub struct Tracker {
    recent: HashMap<String, Vec<Seen>>,
    pending: HashMap<String, Caught>,
    /// When a member's task last took their spam.
    spawned: HashMap<String, f64>,
    /// Members whose task is waiting to start.
    scheduled: HashSet<String>,
    /// Messages already taken for spam (never twice).
    handled: HashMap<String, f64>,
    pruned: f64,
}

/// "server:member"
pub fn who(guild_id: &str, member: &str) -> String {
    format!("{guild_id}:{member}")
}

impl Tracker {
    /// Counts a member's message (`mentions`: the people and roles it pings).
    pub fn screen(
        &mut self,
        message: &Incoming,
        mentions: usize,
        limits: &Limits,
        now: f64,
    ) -> Screened {
        if now - self.pruned >= PRUNE_EVERY {
            self.prune(now);
        }

        let who = who(&message.guild_id, &message.author_id);
        let window = limits.flood_seconds.max(limits.duplicate_seconds);
        let text = fingerprint(&message.content);
        let recent = self.recent.entry(who.clone()).or_default();
        recent.retain(|seen| now - seen.at <= window);
        recent.push(Seen {
            at: now,
            spam: Spam {
                id: message.message_id.clone(),
                channel_id: message.channel_id.clone(),
                said: text::cut(&message.content, SAID, "…"),
            },
            text: text.clone(),
        });
        if recent.len() > RECENT {
            recent.drain(..recent.len() - RECENT);
        }

        let flood: Vec<&Seen> = recent
            .iter()
            .filter(|seen| now - seen.at <= limits.flood_seconds)
            .collect();
        let copies: Vec<&Seen> = if text.is_empty() {
            Vec::new()
        } else {
            recent
                .iter()
                .filter(|seen| seen.text == text && now - seen.at <= limits.duplicate_seconds)
                .collect()
        };
        let last = recent.last().into_iter().collect::<Vec<_>>();

        let (reason, caught) = if flood.len() >= limits.flood_messages {
            (Reason::Flood, flood)
        } else if copies.len() >= limits.duplicates {
            // the first copy stays: it was a message like any other
            (Reason::Repeats, copies[1..].to_vec())
        } else if mentions >= limits.mentions {
            (Reason::Mentions, last)
        } else if emoji(&message.content) >= limits.emoji {
            (Reason::Emoji, last)
        } else {
            return Screened::Clean;
        };
        let caught: Vec<Spam> = caught.into_iter().map(|seen| seen.spam.clone()).collect();

        let pending = self.pending.entry(who.clone()).or_insert_with(|| Caught {
            guild_id: message.guild_id.clone(),
            member: message.author_id.clone(),
            name: message.author_name.clone(),
            roles: message.roles.clone(),
            reason,
            messages: Vec::new(),
        });
        for spam in caught {
            if !self.handled.contains_key(&spam.id) {
                self.handled.insert(spam.id.clone(), now);
                pending.messages.push(spam);
            }
        }

        if pending.messages.is_empty() || !self.scheduled.insert(who.clone()) {
            return Screened::Spam { start_in: None };
        }
        let since = self.spawned.get(&who).map_or(f64::INFINITY, |at| now - at);

        Screened::Spam {
            start_in: Some((SPAWN_GAP - since).max(0.0)),
        }
    }

    /// What a member's task takes: their spam so far (`None` when there is none).
    pub fn take(&mut self, who: &str, now: f64) -> Option<Caught> {
        self.scheduled.remove(who);
        self.spawned.insert(who.to_string(), now);

        self.pending
            .remove(who)
            .filter(|caught| !caught.messages.is_empty())
    }

    /// Forgets what is too old to matter.
    fn prune(&mut self, now: f64) {
        self.pruned = now;
        self.handled.retain(|_, at| now - *at < FORGET);
        self.spawned.retain(|_, at| now - *at < 60.0);
        self.recent
            .retain(|_, seen| seen.last().is_some_and(|last| now - last.at < FORGET));
        let scheduled = &self.scheduled;
        self.pending
            .retain(|who, caught| !caught.messages.is_empty() || scheduled.contains(who));
    }
}

/// The text two messages share when they are the same message: case, spaces and punctuation do
/// not count (links do). Empty for very short ones ("lol", "?"), which only count towards
/// flooding.
pub fn fingerprint(content: &str) -> String {
    let text: String = content
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ':' | '/' | '.'))
        .collect();

    if text.chars().count() >= 4 {
        text
    } else {
        String::new()
    }
}

/// Emoji in a message: Discord's own (`<:name:id>`) and unicode pictographs.
pub fn emoji(content: &str) -> usize {
    EMOJI.find_iter(content).count()
}

/// The people (other than the author) and roles a message pings, each once.
pub fn mentions(message: &Value, author_id: &str) -> usize {
    let mut pinged: HashSet<String> = HashSet::new();
    for user in message["mentions"].as_array().into_iter().flatten() {
        if let Some(id) = user["id"].as_str()
            && id != author_id
        {
            pinged.insert(format!("u{id}"));
        }
    }
    for role in message["mention_roles"].as_array().into_iter().flatten() {
        if let Some(id) = role.as_str() {
            pinged.insert(format!("r{id}"));
        }
    }

    pinged.len()
}

/// Message ids by channel, in the order they came.
pub fn by_channel(messages: &[Spam]) -> Vec<(String, Vec<String>)> {
    let mut channels: Vec<(String, Vec<String>)> = Vec::new();
    for spam in messages {
        match channels
            .iter_mut()
            .find(|(channel, _)| *channel == spam.channel_id)
        {
            Some((_, ids)) => ids.push(spam.id.clone()),
            None => channels.push((spam.channel_id.clone(), vec![spam.id.clone()])),
        }
    }

    channels
}

static TRACKER: LazyLock<parking_lot::Mutex<Tracker>> = LazyLock::new(Default::default);
static CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);

fn now() -> f64 {
    CLOCK.elapsed().as_secs_f64()
}

/// In the event worker: counts a member's message. `true` when it is spam: its task deletes it,
/// and the AI checks (watched channels, standing rules) skip it.
pub fn screen(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    me: &Me,
    message: &Incoming,
    raw: &Value,
) -> bool {
    if !config.settings.spam_enabled {
        return false;
    }

    let limits = Limits::of(&config.settings);
    let screened =
        TRACKER
            .lock()
            .screen(message, mentions(raw, &message.author_id), &limits, now());
    let start_in = match screened {
        Screened::Clean => return false,
        Screened::Spam { start_in: None } => return true,
        Screened::Spam {
            start_in: Some(start_in),
        } => start_in,
    };

    let who = who(&message.guild_id, &message.author_id);
    let (state, config, rest, me) = (
        state.clone(),
        Arc::new(config.clone()),
        rest.clone(),
        me.clone(),
    );
    tokio::spawn(async move {
        if start_in > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(start_in)).await;
        }
        let caught = TRACKER.lock().take(&who, now());
        if let Some(caught) = caught {
            act(&state, config, rest, me, caught).await;
        }
    });

    true
}

/// The spam cooldown's key: a member is warned for spam at most once per cooldown per server.
fn cooldown(guild_id: &str, member: &str) -> String {
    format!("spam:{guild_id}:{member}")
}

/// In its own task: deletes a member's spam and warns them (at most once per cooldown; the
/// warning counts towards the warning ladder). Staff are left alone.
async fn act(state: &crate::State, config: Arc<Config>, rest: Rest, me: Me, caught: Caught) {
    let Some(guild) = config.guild(&caught.guild_id) else {
        return;
    };
    if access::is_staff(state, &rest, &guild.id, &caught.member, Some(&caught.roles)).await {
        return;
    }

    let reason = config.say(caught.reason.key(), &[]);
    let audit = context::audit_reason(&config.settings.name, &config.settings.name, Some(&reason));
    let mut messages = caught.messages;
    let Some(last) = messages.pop() else {
        return;
    };
    for (channel_id, ids) in by_channel(&messages) {
        delete(&rest, &channel_id, &ids, &audit).await;
    }

    tracing::info!(
        member = %caught.member,
        server = %guild.id,
        reason = caught.reason.key(),
        messages = messages.len() + 1,
        "rux: spam"
    );

    let hit = Hit {
        guild_id: guild.id.clone(),
        message_id: last.id,
        channel_id: last.channel_id,
        author_id: caught.member,
        author_name: caught.name,
        roles: Some(caught.roles),
        content: Some(last.said),
    };
    if !super::cool_down(state, &config, &cooldown(&hit.guild_id, &hit.author_id)).await {
        delete(
            &rest,
            &hit.channel_id,
            std::slice::from_ref(&hit.message_id),
            &audit,
        )
        .await;
        return;
    }

    if let Err(err) =
        enforce::enforce_guideline(state, config.clone(), rest, me, &reason, &hit).await
    {
        tracing::warn!(member = %hit.author_id, "rux: spam: the warning failed: {err}");
    }
}

/// Deletes messages of one channel: in bulk, 100 at a time; one by one when a bulk delete fails
/// (it fails as a whole when one of them is already gone). Messages already gone are no error.
async fn delete(rest: &Rest, channel_id: &str, ids: &[String], audit: &str) {
    for chunk in ids.chunks(100) {
        let single = chunk.len() == 1;
        match rest.delete_messages(channel_id, chunk, Some(audit)).await {
            Ok(()) => continue,
            Err(err) if single => {
                if !err.is_not_found() {
                    tracing::warn!(channel = %channel_id, "rux: spam: could not delete a message: {err}");
                }
                continue;
            }
            Err(_) => {}
        }

        for id in chunk {
            if let Err(err) = rest
                .delete_messages(channel_id, std::slice::from_ref(id), Some(audit))
                .await
                && !err.is_not_found()
            {
                tracing::warn!(channel = %channel_id, "rux: spam: could not delete a message: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GUILD: &str = "100000000000000001";
    const ANN: &str = "100000000000000006";

    fn limits() -> Limits {
        Limits::of(&AppSettingsRux::default())
    }

    fn message(id: u64, channel: &str, content: &str) -> Incoming {
        Incoming {
            guild_id: GUILD.into(),
            channel_id: channel.into(),
            message_id: format!("{}", 100_000_000_000_000_100 + id),
            author_id: ANN.into(),
            author_name: "Ann".into(),
            roles: vec!["100000000000000008".into()],
            content: content.into(),
        }
    }

    fn spam(start_in: Option<f64>) -> Screened {
        Screened::Spam { start_in }
    }

    #[test]
    fn limits_come_from_the_settings() {
        let limits = limits();
        assert_eq!(limits.flood_messages, 7);
        assert_eq!(limits.flood_seconds, 10.0);
        assert_eq!(limits.duplicates, 3);
        assert_eq!(limits.duplicate_seconds, 120.0);
        assert_eq!(limits.mentions, 6);
        assert_eq!(limits.emoji, 25);

        let silly = AppSettingsRux {
            spam_flood_messages: 0,
            spam_flood_seconds: -5,
            ..Default::default()
        };
        assert_eq!(Limits::of(&silly).flood_messages, 2);
        assert_eq!(Limits::of(&silly).flood_seconds, 1.0);
    }

    #[test]
    fn same_messages_share_a_fingerprint() {
        assert_eq!(fingerprint("Free NITRO!!! here"), "freenitrohere");
        assert_eq!(
            fingerprint("free nitro here"),
            fingerprint("FREE, nitro here!")
        );
        assert_eq!(
            fingerprint("see https://x.example/a"),
            "seehttps://x.example/a"
        );
        assert_eq!(fingerprint("lol"), "");
        assert_eq!(fingerprint("?!?!?!"), "");
        assert_eq!(fingerprint("Grüße"), "grüße");
    }

    #[test]
    fn emoji_and_mentions_are_counted() {
        assert_eq!(
            emoji("hi 😀😀 <:wave:100000000000000004> <a:spin:100000000000000005> ❤️"),
            5
        );
        assert_eq!(emoji("no emoji here: 123"), 0);

        let raw = json!({
            "mentions": [{"id": "1"}, {"id": "2"}, {"id": "1"}, {"id": ANN}, "junk"],
            "mention_roles": ["1", "3"],
        });
        assert_eq!(mentions(&raw, ANN), 4);
        assert_eq!(mentions(&json!({}), ANN), 0);
    }

    #[test]
    fn a_flood_is_caught_from_the_limit_on() {
        let mut tracker = Tracker::default();
        let limits = limits();
        for n in 0..6u32 {
            assert_eq!(
                tracker.screen(
                    &message(u64::from(n), "1", &format!("message {n}")),
                    0,
                    &limits,
                    100.0 + f64::from(n)
                ),
                Screened::Clean
            );
        }
        // the 7th within 10 seconds: all seven are spam, the task starts at once
        assert_eq!(
            tracker.screen(&message(6, "1", "message 6"), 0, &limits, 106.0),
            spam(Some(0.0))
        );
        // the next one waits for the task already scheduled
        assert_eq!(
            tracker.screen(&message(7, "2", "message 7"), 0, &limits, 106.25),
            spam(None)
        );

        let caught = tracker.take(&who(GUILD, ANN), 106.5).unwrap();
        assert_eq!(caught.reason, Reason::Flood);
        assert_eq!(caught.messages.len(), 8);
        assert_eq!(caught.name, "Ann");
        assert_eq!(
            by_channel(&caught.messages)
                .iter()
                .map(|(channel, ids)| (channel.as_str(), ids.len()))
                .collect::<Vec<_>>(),
            vec![("1", 7), ("2", 1)]
        );

        // more: the next task waits out the gap; nothing is taken twice
        assert_eq!(
            tracker.screen(&message(8, "1", "message 8"), 0, &limits, 107.0),
            spam(Some(SPAWN_GAP - 0.5))
        );
        let caught = tracker.take(&who(GUILD, ANN), 109.5).unwrap();
        assert_eq!(caught.messages.len(), 1);
        assert_eq!(tracker.take(&who(GUILD, ANN), 109.75), None);
    }

    #[test]
    fn repeats_keep_the_first_copy() {
        let mut tracker = Tracker::default();
        let limits = limits();
        assert_eq!(
            tracker.screen(&message(1, "1", "Join my server!"), 0, &limits, 0.0),
            Screened::Clean
        );
        assert_eq!(
            tracker.screen(&message(2, "2", "join my server"), 0, &limits, 30.0),
            Screened::Clean
        );
        // the third copy, in another channel, within two minutes
        assert_eq!(
            tracker.screen(&message(3, "3", "JOIN, my server!"), 0, &limits, 60.0),
            spam(Some(0.0))
        );
        let caught = tracker.take(&who(GUILD, ANN), 60.0).unwrap();
        assert_eq!(caught.reason, Reason::Repeats);
        let ids: Vec<&str> = caught
            .messages
            .iter()
            .map(|spam| spam.id.as_str())
            .collect();
        assert_eq!(ids, vec!["100000000000000102", "100000000000000103"]);

        // too short to compare: never repeats
        let mut tracker = Tracker::default();
        for n in 0..5u32 {
            assert_eq!(
                tracker.screen(
                    &message(u64::from(n), "1", "ok"),
                    0,
                    &limits,
                    f64::from(n) * 5.0
                ),
                Screened::Clean
            );
        }
    }

    #[test]
    fn mentions_and_emoji_walls_are_one_message() {
        let mut tracker = Tracker::default();
        let limits = limits();
        assert_eq!(
            tracker.screen(&message(1, "1", "hey all"), 6, &limits, 0.0),
            spam(Some(0.0))
        );
        let caught = tracker.take(&who(GUILD, ANN), 0.0).unwrap();
        assert_eq!(caught.reason, Reason::Mentions);
        assert_eq!(caught.messages.len(), 1);

        assert_eq!(
            tracker.screen(&message(2, "1", &"😀".repeat(25)), 0, &limits, 50.0),
            spam(Some(0.0))
        );
        assert_eq!(
            tracker.take(&who(GUILD, ANN), 50.0).unwrap().reason,
            Reason::Emoji
        );
        assert_eq!(
            tracker.screen(&message(3, "1", &"😀".repeat(24)), 0, &limits, 100.0),
            Screened::Clean
        );
    }

    #[test]
    fn servers_are_counted_apart_and_old_messages_forgotten() {
        let mut tracker = Tracker::default();
        let limits = limits();
        // eight messages in eight seconds, four on each server: no flood on either
        for n in 0..8u32 {
            let mut other = message(u64::from(n), "1", &format!("hello {n}"));
            if n % 2 == 1 {
                other.guild_id = "100000000000000002".into();
            }
            assert_eq!(
                tracker.screen(&other, 0, &limits, f64::from(n)),
                Screened::Clean
            );
        }

        // long after: nothing is left of the earlier messages
        tracker.screen(&message(10, "1", "fresh"), 0, &limits, 2_000.0);
        assert_eq!(tracker.recent.len(), 1);
        assert!(tracker.handled.is_empty());
    }

    #[test]
    fn spam_lines_exist_in_both_languages() {
        for reason in [
            Reason::Flood,
            Reason::Repeats,
            Reason::Mentions,
            Reason::Emoji,
        ] {
            assert_ne!(crate::rux::say::say("de", reason.key(), &[]), reason.key());
            assert!(crate::rux::say::say("en", reason.key(), &[]).starts_with("Spam: "));
        }
        assert_eq!(cooldown(GUILD, ANN), format!("spam:{GUILD}:{ANN}"));
    }
}
