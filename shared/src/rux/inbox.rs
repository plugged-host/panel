//! plugged.host: which messages are orders. It runs in the gateway's event worker for every
//! message, so it is cheap: no Discord call, only the cache and one insert.
//!
//! An order is a DM to rux, or a message on one of rux's servers that @mentions rux, replies to
//! it, or calls it by name ("hey rux, …", "yo rux …", "rux, …"; this needs the Message Content
//! intent). Messages of servers rux is in but does not work on are ignored, and so are bots,
//! webhooks, system messages and everything but plain messages and replies. People recently
//! found not allowed are dropped here (until moderator roles change), and everyone gets a few
//! orders a minute at most.

use super::{
    access,
    config::Config,
    context::Me,
    models::{Message, NewOrder, conversation_key},
};
use serde_json::{Map, Value, json};

/// The longest order kept, in characters.
pub const MAX_ORDER_CHARS: usize = 4000;
/// How much of a replied-to message is kept.
const MAX_REPLY_CHARS: usize = 1000;
/// How long "not allowed" is remembered.
pub const DENIED_SECONDS: u64 = 300;

/// "hey rux, …", "yo rux …", "rux: …", "ok rux …" at the start of a message, for the bot's name.
pub fn name_call(name: &str) -> Option<regex::Regex> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    regex::Regex::new(&format!(
        r"(?iu)^\W*(?:(?:hey|hi|hello|yo|oi|ok|okay|ay|aye|sup|pls|please|dear|hiya|heya)\W+)?{}\b",
        regex::escape(name)
    ))
    .ok()
}

/// The last name's [`name_call`], so the pattern is not built again for every message.
static NAME_CALL: std::sync::LazyLock<parking_lot::Mutex<Option<(String, Option<regex::Regex>)>>> =
    std::sync::LazyLock::new(Default::default);

/// Whether `content` calls the bot by `name`.
fn calls_by_name(name: &str, content: &str) -> bool {
    let mut call = NAME_CALL.lock();
    if call.as_ref().is_none_or(|(cached, _)| cached != name) {
        *call = Some((name.to_string(), name_call(name)));
    }

    call.as_ref()
        .and_then(|(_, pattern)| pattern.as_ref())
        .is_some_and(|pattern| pattern.is_match(content))
}

fn id_of(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn display_name(user: &Value) -> String {
    [&user["global_name"], &user["username"]]
        .into_iter()
        .filter_map(Value::as_str)
        .find(|name| !name.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The order in a `MESSAGE_CREATE`, or `None` when it is none. Pure: the denied and rate limit
/// checks are [`take`]'s.
pub fn accept(message: &Value, me: &Me, config: &Config) -> Option<NewOrder> {
    let author = &message["author"];
    let author_id = id_of(&author["id"]);
    if author_id.is_empty()
        || author_id == me.user_id
        || author["bot"].as_bool() == Some(true)
        || author["system"].as_bool() == Some(true)
        || !message["webhook_id"].is_null()
    {
        return None;
    }
    // plain messages and replies only; joins, pins and the like have other types
    if !matches!(message["type"].as_i64().unwrap_or(0), 0 | 19) {
        return None;
    }

    let guild_id = message["guild_id"].as_str().map(str::to_string);
    let content = message["content"].as_str().unwrap_or_default();
    let reference = &message["referenced_message"];
    let replies_to_me = !me.user_id.is_empty() && id_of(&reference["author"]["id"]) == me.user_id;

    if let Some(guild_id) = &guild_id {
        if !config.serves(Some(guild_id)) {
            return None;
        }

        let mentioned = message["mentions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|user| !me.user_id.is_empty() && id_of(&user["id"]) == me.user_id);
        let called = calls_by_name(&config.settings.name, content);
        if !mentioned && !replies_to_me && !called {
            return None;
        }
    }

    let content = if me.user_id.is_empty() {
        content.trim().to_string()
    } else {
        content
            .replace(&format!("<@{}>", me.user_id), "")
            .replace(&format!("<@!{}>", me.user_id), "")
            .trim()
            .to_string()
    };
    let images = super::images::attachments(message);
    let reply_images = if replies_to_me {
        Vec::new()
    } else {
        super::images::attachments(reference)
    };
    // a screenshot alone (or "@rux" in reply to one) is a question too
    if content.is_empty() && images.is_empty() && reply_images.is_empty() {
        return None;
    }

    let channel_id = message["channel_id"].as_str()?.to_string();
    let mentions: Vec<Value> = message["mentions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|user| id_of(&user["id"]) != me.user_id && !id_of(&user["id"]).is_empty())
        .map(|user| {
            let mut mention = json!({
                "id": id_of(&user["id"]),
                "name": display_name(user),
            });
            if user["bot"].as_bool() == Some(true) {
                mention["bot"] = Value::Bool(true);
            }
            mention
        })
        .collect();

    let mut meta = Map::new();
    if !mentions.is_empty() {
        meta.insert("mentions".into(), Value::Array(mentions));
    }
    // the author's roles come with a server message: the prompt names them without a lookup
    if guild_id.is_some()
        && let Some(roles) = message["member"]["roles"].as_array()
    {
        meta.insert(
            "author_roles".into(),
            roles
                .iter()
                .filter(|role| role.as_str().is_some_and(crate::discord::is_snowflake))
                .cloned()
                .collect(),
        );
    }
    if !images.is_empty() {
        meta.insert(
            "images".into(),
            images
                .into_iter()
                .map(|url| json!({ "url": url }))
                .collect(),
        );
    }
    if let Some(reference_id) = reference["id"].as_str()
        && !replies_to_me
    {
        let mut reply = json!({
            "id": reference_id,
            "author_id": id_of(&reference["author"]["id"]),
            "author_name": display_name(&reference["author"]),
            "content": chars(reference["content"].as_str().unwrap_or_default(), MAX_REPLY_CHARS),
        });
        if !reply_images.is_empty() {
            reply["images"] = reply_images
                .into_iter()
                .map(|url| json!({ "url": url }))
                .collect();
        }
        meta.insert("reply_to".into(), reply);
    }

    let author_name = [
        &message["member"]["nick"],
        &author["global_name"],
        &author["username"],
    ]
    .into_iter()
    .filter_map(Value::as_str)
    .find(|name| !name.is_empty())
    .map(|name| chars(name, 100));

    Some(NewOrder {
        conversation: conversation_key(&channel_id, author_id),
        discord_message_id: message["id"].as_str().map(str::to_string),
        channel_id,
        guild_id,
        author_id: author_id.to_string(),
        author_name,
        content: chars(&content, MAX_ORDER_CHARS),
        meta: (!meta.is_empty()).then_some(Value::Object(meta)),
    })
}

/// Stores the order in this message, unless its author was found not allowed a moment ago or
/// sends too many; answers its id for the agent.
pub async fn take(
    state: &crate::State,
    me: &Me,
    config: &Config,
    message: &Value,
) -> Result<Option<i64>, anyhow::Error> {
    let Some(order) = accept(message, me, config) else {
        return Ok(None);
    };

    let generation = access::generation(state).await;
    let denied = access::denied_key(generation, order.guild_id.as_deref(), &order.author_id);
    if state.cache.get::<bool>(&denied).await.ok().flatten() == Some(true) {
        return Ok(None);
    }

    // `ratelimit` counts this call too: the limit is the orders allowed plus one
    let per_minute = config.settings.messages_per_minute.max(1) as u64;
    if state
        .cache
        .ratelimit("rux::orders", per_minute + 1, 60, &order.author_id)
        .await
        .is_err()
    {
        tracing::debug!(author = %order.author_id, "rux: too many orders");
        return Ok(None);
    }

    Ok(Some(Message::insert_order(&state.database, &order).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::rux::AppSettingsRux;
    use std::sync::Arc;

    const GUILD: &str = "100000000000000001";
    const OTHER_GUILD: &str = "100000000000000008";
    const BOT: &str = "100000000000000002";
    const AUTHOR: &str = "100000000000000003";
    const CHANNEL: &str = "100000000000000004";

    fn me() -> Me {
        Me {
            user_id: BOT.into(),
            application_id: BOT.into(),
        }
    }

    fn config() -> Arc<Config> {
        let settings = AppSettingsRux {
            guild_id: GUILD.into(),
            ..Default::default()
        };

        Arc::new(Config {
            guilds: super::super::config::guilds(&settings, "", "Example"),
            settings,
            enabled: true,
            app_name: "Example".into(),
            portal: "Example".into(),
            link_url: "https://panel.example.com".into(),
            boost_two_role_id: String::new(),
            managed: None,
            managed_bot_token: None,
            memberships: Default::default(),
        })
    }

    fn message(content: &str) -> Value {
        json!({
            "id": "100000000000000005",
            "type": 0,
            "channel_id": CHANNEL,
            "guild_id": GUILD,
            "content": content,
            "author": {"id": AUTHOR, "username": "ann", "global_name": "Ann"},
            "member": {"nick": "Annie"},
            "mentions": [],
        })
    }

    #[test]
    fn name_calls() {
        let call = name_call("rux").unwrap();
        for yes in [
            "hey rux, ban him",
            "Rux: purge 10",
            "yo rux",
            "  ok, rux do it",
            "RUX!",
        ] {
            assert!(call.is_match(yes), "{yes}");
        }
        for no in ["ruxbot do it", "I asked rux", "hey there rux", "ruxx"] {
            assert!(!call.is_match(no), "{no}");
        }
        assert!(name_call("  ").is_none());
        assert!(name_call("m.o.d").unwrap().is_match("hey m.o.d go"));
        assert!(!name_call("m.o.d").unwrap().is_match("hey mxoxd go"));
    }

    #[test]
    fn server_messages_need_to_address_rux() {
        let config = config();
        assert!(accept(&message("just chatting"), &me(), &config).is_none());

        let order = accept(&message("hey rux, warn him"), &me(), &config).unwrap();
        assert_eq!(order.content, "hey rux, warn him");
        assert_eq!(order.conversation, format!("{CHANNEL}:{AUTHOR}"));
        assert_eq!(order.author_name.as_deref(), Some("Annie"));
        assert_eq!(order.guild_id.as_deref(), Some(GUILD));

        let mut mentioned = message(&format!("<@{BOT}> purge 10"));
        mentioned["mentions"] = json!([{"id": BOT, "username": "rux", "bot": true}, {"id": "100000000000000006", "username": "bob", "bot": true}]);
        let order = accept(&mentioned, &me(), &config).unwrap();
        assert_eq!(order.content, "purge 10");
        assert_eq!(
            order.meta.unwrap()["mentions"],
            json!([{"id": "100000000000000006", "name": "bob", "bot": true}])
        );

        let mut reply = message("do it");
        reply["type"] = json!(19);
        reply["referenced_message"] = json!({"id": "1", "author": {"id": BOT}, "content": "Sure?"});
        let order = accept(&reply, &me(), &config).unwrap();
        // a reply to rux itself is not kept as data
        assert!(order.meta.is_none());

        let mut elsewhere = message("hey rux");
        elsewhere["guild_id"] = json!(OTHER_GUILD);
        assert!(accept(&elsewhere, &me(), &config).is_none());
    }

    #[test]
    fn the_authors_roles_come_along() {
        let config = config();
        let mut with_roles = message("hey rux, who am i");
        with_roles["member"]["roles"] = json!(["100000000000000009", "../x", 7]);
        let order = accept(&with_roles, &me(), &config).unwrap();
        assert_eq!(
            order.meta.unwrap()["author_roles"],
            json!(["100000000000000009"])
        );

        // no roles is something to know too
        let mut none = message("hey rux");
        none["member"]["roles"] = json!([]);
        assert_eq!(
            accept(&none, &me(), &config).unwrap().meta.unwrap()["author_roles"],
            json!([])
        );

        // a DM has no member
        let mut dm = with_roles.clone();
        dm.as_object_mut().unwrap().remove("guild_id");
        assert!(accept(&dm, &me(), &config).unwrap().meta.is_none());
    }

    #[test]
    fn dms_are_orders_and_bots_never() {
        let config = config();
        let mut dm = message("hello");
        dm.as_object_mut().unwrap().remove("guild_id");
        let order = accept(&dm, &me(), &config).unwrap();
        assert!(order.guild_id.is_none());

        let mut bot = dm.clone();
        bot["author"]["bot"] = json!(true);
        assert!(accept(&bot, &me(), &config).is_none());

        let mut webhook = dm.clone();
        webhook["webhook_id"] = json!("1");
        assert!(accept(&webhook, &me(), &config).is_none());

        let mut pin = dm.clone();
        pin["type"] = json!(6);
        assert!(accept(&pin, &me(), &config).is_none());

        let mut own = dm.clone();
        own["author"]["id"] = json!(BOT);
        assert!(accept(&own, &me(), &config).is_none());

        assert!(
            accept(
                &json!({"author": {"id": AUTHOR}, "channel_id": CHANNEL, "content": "  "}),
                &me(),
                &config
            )
            .is_none()
        );
    }

    #[test]
    fn replied_messages_and_screenshots_are_kept_as_data() {
        let config = config();
        let mut reply = message(&format!("<@{BOT}>"));
        reply["mentions"] = json!([{"id": BOT}]);
        reply["referenced_message"] = json!({
            "id": "100000000000000007",
            "author": {"id": "100000000000000006", "username": "bob"},
            "content": "x".repeat(1500),
            "attachments": [{"url": "https://cdn.discordapp.com/a.png", "content_type": "image/png", "size": 1}]
        });

        let order = accept(&reply, &me(), &config).unwrap();
        assert_eq!(order.content, "");
        let meta = order.meta.unwrap();
        assert_eq!(meta["reply_to"]["author_name"], "bob");
        assert_eq!(
            meta["reply_to"]["content"].as_str().unwrap().len(),
            MAX_REPLY_CHARS
        );
        assert_eq!(
            meta["reply_to"]["images"],
            json!([{"url": "https://cdn.discordapp.com/a.png"}])
        );
    }
}
