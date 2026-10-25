//! plugged.host: what rux checks by itself (docs/features/rux.md, "Watching channels"): spam in
//! every channel without AI ([`spam`]), every member message of a watched channel ([`channels`])
//! and, elsewhere, messages that hit a standing rule's keywords ([`rules`]); the last two are
//! checked by the AI ([`judge`]), never on premium models.
//!
//! [`on_message`] runs in the gateway's event worker, in order, for every message of a server
//! rux works on. It only does quick things there (counters in memory, the rules and watched
//! channels kept in memory, one insert into the queue) and leaves the AI and Discord to tasks of
//! their own. Bots, webhooks, system messages and rux itself are skipped there; staff in those
//! tasks (a member's roles come with the message, so that check needs no lookup), so staff are
//! never acted on. What acts is the rules tools' ([`crate::rux::tools::rules::enforce`]), as an
//! order would: the same cases, mod log and notices.

mod channels;
mod judge;
mod rules;
mod spam;

use crate::rux::{
    config::Config,
    context::Me,
    discord::Rest,
    models::{RULE_COLUMNS, Rule},
    say::Strings,
};
use serde_json::Value;

/// The checks' lines (`spam.*`, `watch.*`; what a member caught is told): `(key, English,
/// German)`.
pub const STRINGS: Strings = &[
    (
        "spam.flood",
        "Spam: too many messages at once",
        "Spam: zu viele Nachrichten auf einmal",
    ),
    (
        "spam.repeats",
        "Spam: the same message again and again",
        "Spam: immer wieder dieselbe Nachricht",
    ),
    (
        "spam.mentions",
        "Spam: mentioning many people at once",
        "Spam: viele Leute auf einmal erwähnt",
    ),
    (
        "spam.emoji",
        "Spam: a wall of emoji",
        "Spam: eine Wand aus Emojis",
    ),
    // the guidelines of watched channels (rules::model::GUIDELINES), as their breakers are told
    ("watch.g1", "Scam or phishing", "Betrug oder Phishing"),
    (
        "watch.g2",
        "Slurs or hate against a group of people",
        "Beleidigungen oder Hass gegen eine Gruppe von Menschen",
    ),
    (
        "watch.g3",
        "Harassing, threatening or bullying someone",
        "Jemanden belästigen, bedrohen oder mobben",
    ),
    (
        "watch.g4",
        "Sexual or gore content",
        "Sexuelle oder brutale Inhalte",
    ),
    (
        "watch.g6",
        "Advertising another Discord server, game host or paid service",
        "Werbung für einen anderen Discord-Server, Game-Host oder bezahlten Dienst",
    ),
];

/// A member's message as the checks read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub author_id: String,
    /// Their nickname, else their display name, else their username (100 characters at most).
    pub author_name: String,
    /// Their role ids (Discord sends them with the message).
    pub roles: Vec<String>,
    pub content: String,
}

/// A message the checks look at: a member's plain message or reply on a server, with Discord
/// ids. Bots (rux too), webhooks and system messages are not.
pub fn incoming(message: &Value, me: &Me) -> Option<Incoming> {
    let author = &message["author"];
    if author["bot"].as_bool() == Some(true)
        || author["system"].as_bool() == Some(true)
        || !message["webhook_id"].is_null()
        || !matches!(message["type"].as_i64().unwrap_or(0), 0 | 19)
    {
        return None;
    }

    let id = |value: &Value| {
        value
            .as_str()
            .filter(|id| crate::discord::is_snowflake(id))
            .map(str::to_string)
    };
    let author_id = id(&author["id"])?;
    if author_id == me.user_id {
        return None;
    }
    let author_name = [
        &message["member"]["nick"],
        &author["global_name"],
        &author["username"],
    ]
    .into_iter()
    .filter_map(Value::as_str)
    .map(str::trim)
    .find(|name| !name.is_empty())
    .unwrap_or(author_id.as_str());

    Some(Incoming {
        guild_id: id(&message["guild_id"])?,
        channel_id: id(&message["channel_id"])?,
        message_id: id(&message["id"])?,
        author_name: crate::rux::text::cut(author_name, 100, ""),
        roles: message["member"]["roles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        content: message["content"].as_str().unwrap_or_default().to_string(),
        author_id,
    })
}

/// A `MESSAGE_CREATE` on a server rux works on (orders included: staff are left alone by the
/// checks themselves). It runs in the gateway's event worker, in order: anything slow (an AI
/// check, a Discord call) is spawned or queued, never awaited here.
///
/// Spam is caught first, without AI, and not checked again. Then every member message of a
/// watched channel is queued for the AI; elsewhere only the ones that hit a standing rule's
/// keywords go to it.
pub async fn on_message(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    me: &Me,
    message: &Value,
) -> Result<(), anyhow::Error> {
    let Some(incoming) = incoming(message, me) else {
        return Ok(());
    };
    if !config.serves(Some(&incoming.guild_id)) {
        return Ok(());
    }

    if spam::screen(state, config, rest, me, &incoming, message) {
        return Ok(());
    }
    if channels::enqueue(state, rest, me, &incoming, message).await {
        return Ok(());
    }

    rules::screen(state, config, rest, me, &incoming, message).await
}

/// Every minute on the primary instance (`rux::tick`): forgets queued messages that waited too
/// long and starts a run over the queue when messages wait (the run is a task of its own, so the
/// tick is quick).
pub async fn tick(
    state: &crate::State,
    _config: &Config,
    _rest: &Rest,
) -> Result<(), anyhow::Error> {
    channels::tick(state).await
}

/// Whether checking watched channels is paused: no AI could answer, and rux tries again within
/// ten minutes (for `list_rules`).
pub async fn paused(state: &crate::State) -> bool {
    channels::paused(state).await
}

// --- shared by the checks ------------------------------------------------------------------

/// Where the cooldowns are kept (`ratelimit` keys: an atomic "set once" with a lifetime).
const COOLDOWNS: &str = "rux::watch::cooldown";

/// The rule cooldown in seconds (30 at least).
fn cooldown_seconds(config: &Config) -> u64 {
    config.settings.rule_cooldown_seconds.max(30) as u64
}

/// The cooldown of one rule for one member: the keyword checks and the watched channels share
/// it.
fn rule_cooldown(rule_id: i64, member: &str) -> String {
    format!("rule:{rule_id}:{member}")
}

/// Whether a cooldown runs.
async fn cooling(state: &crate::State, key: &str) -> bool {
    state.cache.ratelimit_reached(COOLDOWNS, 1, key).await
}

/// Starts a cooldown; `false` when it already ran (someone else acted).
async fn cool_down(state: &crate::State, config: &Config, key: &str) -> bool {
    state
        .cache
        .ratelimit(COOLDOWNS, 2, cooldown_seconds(config), key)
        .await
        .is_ok()
}

/// A server's active rule.
async fn rule(state: &crate::State, id: i64, guild_id: &str) -> Result<Option<Rule>, sqlx::Error> {
    sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
        "SELECT {RULE_COLUMNS} FROM rux_rules WHERE id = $1 AND guild_id = $2 AND active"
    )))
    .bind(id)
    .bind(guild_id)
    .fetch_optional(state.database.read())
    .await
}

/// Counts a message checked against a rule (`list_rules`' "messages checked").
async fn count_hit(state: &crate::State, id: i64) {
    if let Err(err) = sqlx::query("UPDATE rux_rules SET hits = hits + 1 WHERE id = $1")
        .bind(id)
        .execute(state.database.write())
        .await
    {
        tracing::warn!(rule = id, "rux: could not count a rule's check: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BOT: &str = "100000000000000003";

    fn me() -> Me {
        Me {
            user_id: BOT.into(),
            application_id: BOT.into(),
        }
    }

    fn message() -> Value {
        json!({
            "id": "100000000000000005",
            "type": 0,
            "guild_id": "100000000000000001",
            "channel_id": "100000000000000004",
            "content": "hello",
            "author": { "id": "100000000000000006", "username": "ann", "global_name": "Ann" },
            "member": { "nick": null, "roles": ["100000000000000008", 7] },
        })
    }

    #[test]
    fn members_messages_are_read() {
        let read = incoming(&message(), &me()).unwrap();
        assert_eq!(
            read,
            Incoming {
                guild_id: "100000000000000001".into(),
                channel_id: "100000000000000004".into(),
                message_id: "100000000000000005".into(),
                author_id: "100000000000000006".into(),
                author_name: "Ann".into(),
                roles: vec!["100000000000000008".into()],
                content: "hello".into(),
            }
        );

        let mut nick = message();
        nick["member"]["nick"] = json!(format!(" {} ", "n".repeat(150)));
        assert_eq!(
            incoming(&nick, &me()).unwrap().author_name.chars().count(),
            100
        );
        let mut nameless = message();
        nameless["author"] = json!({ "id": "100000000000000006" });
        assert_eq!(
            incoming(&nameless, &me()).unwrap().author_name,
            "100000000000000006"
        );
        let mut reply = message();
        reply["type"] = json!(19);
        assert!(incoming(&reply, &me()).is_some());
    }

    #[test]
    fn bots_webhooks_system_messages_and_rux_are_skipped() {
        let mut bot = message();
        bot["author"]["bot"] = json!(true);
        let mut webhook = message();
        webhook["webhook_id"] = json!("100000000000000009");
        let mut joined = message();
        joined["type"] = json!(7);
        let mut system = message();
        system["author"]["system"] = json!(true);
        let mut itself = message();
        itself["author"]["id"] = json!(BOT);
        let mut dm = message();
        dm["guild_id"] = Value::Null;
        let mut bad = message();
        bad["channel_id"] = json!("../x");

        for skipped in [bot, webhook, joined, system, itself, dm, bad] {
            assert_eq!(incoming(&skipped, &me()), None, "{skipped}");
        }
    }

    #[test]
    fn cooldowns_are_per_rule_and_member() {
        assert_eq!(
            rule_cooldown(4, "100000000000000006"),
            "rule:4:100000000000000006"
        );
        let mut config = crate::rux::tools::giveaways::service::tests::config("en");
        config.settings.rule_cooldown_seconds = 0;
        assert_eq!(cooldown_seconds(&config), 30);
        config.settings.rule_cooldown_seconds = 300;
        assert_eq!(cooldown_seconds(&config), 300);
    }
}
