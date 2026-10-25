//! plugged.host: which Discord messages neo answers, and what it keeps of one (docs/features/neo-discord.md).
//!
//! neo answers a DM, or a message on the Discord server that @mentions it, replies to one of its
//! messages or (with name calls on) starts with its name ("hey neo, …"). Anyone may ask, linked or
//! not. Messages of bots, webhooks and the system are ignored, and so are joins, pins and the like.

use serde_json::{Value, json};

/// What one message may carry into a turn, in characters.
pub const MAX_CONTENT_CHARS: usize = 2000;
/// What is kept of a replied-to message.
const MAX_REPLY_CHARS: usize = 1000;
const MAX_NAME_CHARS: usize = 100;
/// Images one message brings at most.
pub const MAX_IMAGES: usize = crate::neo::chat::MAX_IMAGES;
/// The largest attachment fetched.
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];
/// Discord's own CDN: attachments are only ever fetched from there.
const DISCORD_HOSTS: [&str; 2] = ["cdn.discordapp.com", "media.discordapp.net"];
/// Greetings that may come before the name in a name call.
const GREETINGS: &str = "hey|hi|hello|yo|oi|ok|okay|ay|aye|sup|pls|please|dear|hiya|heya";

/// The bot as the inbox needs it.
pub struct Me<'a> {
    pub id: &'a str,
    /// The Discord server it answers on; DMs are answered from anyone.
    pub guild_id: &'a str,
    /// Matches a message that calls the bot by name; `None` while name calls are off.
    pub name_call: Option<&'a regex::Regex>,
}

/// A message neo answers, as it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub discord_message_id: Option<String>,
    pub channel_id: String,
    /// `None` in a DM.
    pub guild_id: Option<String>,
    pub author_id: String,
    pub author_name: Option<String>,
    /// The text without the bot's mention.
    pub content: String,
    /// `mentions` (the other people it names), `images` (attachment URLs) and `reply_to` (the
    /// message it replies to, when that is not the bot's own).
    pub meta: Option<Value>,
}

impl Incoming {
    pub fn conversation(&self) -> String {
        conversation_key(&self.channel_id, &self.author_id)
    }
}

/// One person in one channel (or DM) is one conversation.
pub fn conversation_key(channel_id: &str, author_id: &str) -> String {
    format!("{channel_id}:{author_id}")
}

/// The pattern of a call by name, for the bot's names (its username and display name).
pub fn name_call(names: &[&str]) -> Option<regex::Regex> {
    let names: Vec<String> = names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(regex::escape)
        .collect();
    if names.is_empty() {
        return None;
    }

    regex::Regex::new(&format!(
        r"(?i)^\W*(?:(?:{GREETINGS})\W+)?(?:{})(?:\W|$)",
        names.join("|")
    ))
    .ok()
}

fn str_at<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Whether `url` is an https link on Discord's CDN (no port, no credentials).
pub fn is_discord_cdn(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };

    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .is_some_and(|host| DISCORD_HOSTS.contains(&host.to_ascii_lowercase().as_str()))
}

/// A message's image attachments (PNG, JPEG, WebP or GIF on Discord's CDN, 10 MB at most), at
/// most [`MAX_IMAGES`].
pub fn image_urls(message: &Value) -> Vec<String> {
    message["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|file| {
            let kind = file["content_type"]
                .as_str()
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();

            IMAGE_TYPES.contains(&kind.as_str())
                && file["size"].as_u64().unwrap_or(0) <= MAX_IMAGE_BYTES
        })
        .filter_map(|file| file["url"].as_str())
        .filter(|url| is_discord_cdn(url))
        .take(MAX_IMAGES)
        .map(str::to_string)
        .collect()
}

/// Whether neo answers this `MESSAGE_CREATE`, and what is kept of it.
pub fn accept(message: &Value, me: &Me<'_>) -> Option<Incoming> {
    let author = &message["author"];
    let author_id = author["id"].as_str().unwrap_or_default();

    if author_id.is_empty()
        || author_id == me.id
        || author["bot"].as_bool().unwrap_or(false)
        || author["system"].as_bool().unwrap_or(false)
        || !message["webhook_id"].is_null()
    {
        return None;
    }

    // normal messages and replies only; joins, pins and the like have other types
    if !matches!(message["type"].as_u64().unwrap_or(0), 0 | 19) {
        return None;
    }

    let channel_id = message["channel_id"].as_str().filter(|id| !id.is_empty())?;
    let guild_id = message["guild_id"].as_str().map(str::to_string);
    let content = message["content"].as_str().unwrap_or_default();
    let reference = &message["referenced_message"];
    let replies_to_me = str_at(reference, "/author/id") == me.id;

    if let Some(guild_id) = &guild_id {
        if guild_id != me.guild_id {
            return None;
        }

        let mentioned = message["mentions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|user| user["id"].as_str() == Some(me.id));
        let called = me.name_call.is_some_and(|call| call.is_match(content));

        if !mentioned && !replies_to_me && !called {
            return None;
        }
    }

    let content = content
        .replace(&format!("<@{}>", me.id), "")
        .replace(&format!("<@!{}>", me.id), "");
    let content = content.trim();
    let images = image_urls(message);
    let reply_images = if replies_to_me {
        Vec::new()
    } else {
        image_urls(reference)
    };

    // a screenshot alone (or "@bot" in reply to one) is a question too
    if content.is_empty() && images.is_empty() && reply_images.is_empty() {
        return None;
    }

    let mentions: Vec<Value> = message["mentions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|user| user["id"].as_str().is_some_and(|id| id != me.id))
        .map(|user| {
            let mut mention = json!({
                "id": user["id"],
                "name": cut(display_name(user), MAX_NAME_CHARS),
            });
            if user["bot"].as_bool().unwrap_or(false) {
                mention["bot"] = json!(true);
            }
            mention
        })
        .collect();

    let reply_to = (!replies_to_me && reference["id"].is_string()).then(|| {
        let mut reply = json!({
            "id": reference["id"],
            "author_id": str_at(reference, "/author/id"),
            "author_name": cut(display_name(&reference["author"]), MAX_NAME_CHARS),
            "content": cut(reference["content"].as_str().unwrap_or_default(), MAX_REPLY_CHARS),
        });
        if !reply_images.is_empty() {
            reply["images"] = json!(reply_images);
        }
        reply
    });

    let mut meta = serde_json::Map::new();
    if !mentions.is_empty() {
        meta.insert("mentions".into(), Value::Array(mentions));
    }
    if !images.is_empty() {
        meta.insert("images".into(), json!(images));
    }
    if let Some(reply_to) = reply_to {
        meta.insert("reply_to".into(), reply_to);
    }

    let author_name = [
        str_at(message, "/member/nick"),
        author["global_name"].as_str().unwrap_or_default(),
        author["username"].as_str().unwrap_or_default(),
    ]
    .into_iter()
    .find(|name| !name.is_empty())
    .map(|name| cut(name, MAX_NAME_CHARS));

    Some(Incoming {
        discord_message_id: message["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_string),
        channel_id: channel_id.to_string(),
        guild_id,
        author_id: author_id.to_string(),
        author_name,
        content: cut(content, MAX_CONTENT_CHARS),
        meta: (!meta.is_empty()).then_some(Value::Object(meta)),
    })
}

/// A user's display name, else their username.
fn display_name(user: &Value) -> &str {
    user["global_name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .or_else(|| user["username"].as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOT: &str = "100000000000000001";
    const GUILD: &str = "200000000000000002";
    const CHANNEL: &str = "300000000000000003";
    const PERSON: &str = "400000000000000004";

    fn me(name_call: Option<&regex::Regex>) -> Me<'_> {
        Me {
            id: BOT,
            guild_id: GUILD,
            name_call,
        }
    }

    fn message(content: &str, guild: Option<&str>) -> Value {
        let mut message = json!({
            "id": "500000000000000005",
            "type": 0,
            "channel_id": CHANNEL,
            "content": content,
            "author": {"id": PERSON, "username": "someone", "global_name": "Some One"},
            "mentions": [],
            "attachments": [],
        });
        if let Some(guild) = guild {
            message["guild_id"] = json!(guild);
        }
        message
    }

    #[test]
    fn answers_dms_from_anyone() {
        let incoming = accept(&message("how do I link Discord?", None), &me(None)).unwrap();
        assert_eq!(incoming.guild_id, None);
        assert_eq!(incoming.content, "how do I link Discord?");
        assert_eq!(incoming.author_name.as_deref(), Some("Some One"));
        assert_eq!(incoming.conversation(), format!("{CHANNEL}:{PERSON}"));
        assert_eq!(incoming.meta, None);
    }

    #[test]
    fn on_the_server_only_when_addressed() {
        assert!(accept(&message("just chatting", Some(GUILD)), &me(None)).is_none());

        let mut mention = message(&format!("<@{BOT}> what is Plus?"), Some(GUILD));
        mention["mentions"] = json!([{"id": BOT, "username": "neo", "bot": true}]);
        assert_eq!(
            accept(&mention, &me(None)).unwrap().content,
            "what is Plus?"
        );

        let mut reply = message("and the weekly one?", Some(GUILD));
        reply["type"] = json!(19);
        reply["referenced_message"] = json!({"id": "1", "author": {"id": BOT}, "content": "..."});
        let incoming = accept(&reply, &me(None)).unwrap();
        assert_eq!(
            incoming.meta, None,
            "its own message is not kept as a reply"
        );

        // another server's messages are never answered
        let mut elsewhere = mention.clone();
        elsewhere["guild_id"] = json!("999999999999999999");
        assert!(accept(&elsewhere, &me(None)).is_none());
    }

    #[test]
    fn calls_by_name_only_when_on() {
        let call = name_call(&["neo", "neo"]).unwrap();
        for text in [
            "hey neo, how do credits work?",
            "Neo: help",
            "ok NEO what",
            "neo",
        ] {
            assert!(
                accept(&message(text, Some(GUILD)), &me(Some(&call))).is_some(),
                "{text}"
            );
        }
        for text in ["neon lights", "I asked neo yesterday", "hey neoo"] {
            assert!(
                accept(&message(text, Some(GUILD)), &me(Some(&call))).is_none(),
                "{text}"
            );
        }
        assert!(accept(&message("hey neo", Some(GUILD)), &me(None)).is_none());
        assert!(name_call(&["", " "]).is_none());

        // names are text, not patterns
        let odd = name_call(&["n.o (bot)"]).unwrap();
        assert!(odd.is_match("hi n.o (bot) there"));
        assert!(!odd.is_match("hi nxo (bot) there"));
    }

    #[test]
    fn ignores_bots_webhooks_and_system_messages() {
        let mut bot = message("hello", None);
        bot["author"]["bot"] = json!(true);
        assert!(accept(&bot, &me(None)).is_none());

        let mut hook = message("hello", None);
        hook["webhook_id"] = json!("1");
        assert!(accept(&hook, &me(None)).is_none());

        let mut pin = message("hello", None);
        pin["type"] = json!(6);
        assert!(accept(&pin, &me(None)).is_none());

        let mut own = message("hello", None);
        own["author"]["id"] = json!(BOT);
        assert!(accept(&own, &me(None)).is_none());

        assert!(accept(&message("   ", None), &me(None)).is_none());
    }

    #[test]
    fn keeps_mentions_replies_and_images() {
        let mut asked = message("why is this?", None);
        asked["member"] = json!({"nick": "Nick"});
        asked["mentions"] = json!([
            {"id": "600000000000000006", "username": "friend"},
            {"id": "700000000000000007", "username": "helper", "global_name": "Helper", "bot": true},
        ]);
        asked["attachments"] = json!([
            {"url": "https://cdn.discordapp.com/attachments/1/2/shot.png", "content_type": "image/png", "size": 1000},
            {"url": "https://example.com/x.png", "content_type": "image/png", "size": 1000},
            {"url": "https://cdn.discordapp.com/attachments/1/2/log.txt", "content_type": "text/plain", "size": 10},
        ]);
        asked["referenced_message"] = json!({
            "id": "800000000000000008",
            "author": {"id": "600000000000000006", "username": "friend"},
            "content": "x".repeat(1500),
            "attachments": [{"url": "https://media.discordapp.net/attachments/1/2/a.webp", "content_type": "image/webp; charset=x", "size": 5}],
        });

        let incoming = accept(&asked, &me(None)).unwrap();
        let meta = incoming.meta.unwrap();
        assert_eq!(incoming.author_name.as_deref(), Some("Nick"));
        assert_eq!(meta["mentions"][0]["name"], "friend");
        assert_eq!(meta["mentions"][1]["bot"], true);
        assert_eq!(
            meta["images"],
            json!(["https://cdn.discordapp.com/attachments/1/2/shot.png"])
        );
        assert_eq!(meta["reply_to"]["author_id"], "600000000000000006");
        assert_eq!(meta["reply_to"]["content"].as_str().unwrap().len(), 1000);
        assert_eq!(
            meta["reply_to"]["images"][0],
            "https://media.discordapp.net/attachments/1/2/a.webp"
        );

        // a screenshot alone is a question
        let mut only = message("", None);
        only["attachments"] = asked["attachments"].clone();
        assert!(accept(&only, &me(None)).is_some());
    }

    #[test]
    fn attachments_come_only_from_discords_cdn() {
        assert!(is_discord_cdn(
            "https://cdn.discordapp.com/attachments/1/2/a.png?ex=1"
        ));
        assert!(is_discord_cdn("https://MEDIA.discordapp.net/a.png"));
        assert!(!is_discord_cdn("http://cdn.discordapp.com/a.png"));
        assert!(!is_discord_cdn("https://cdn.discordapp.com:8443/a.png"));
        assert!(!is_discord_cdn("https://user:pw@cdn.discordapp.com/a.png"));
        assert!(!is_discord_cdn(
            "https://cdn.discordapp.com.example.com/a.png"
        ));
        assert!(!is_discord_cdn("not a url"));

        let big = json!({"attachments": [
            {"url": "https://cdn.discordapp.com/a.png", "content_type": "image/png", "size": MAX_IMAGE_BYTES + 1},
        ]});
        assert!(image_urls(&big).is_empty());

        let many = json!({"attachments": (0..6).map(|i| json!({
            "url": format!("https://cdn.discordapp.com/{i}.png"), "content_type": "image/png", "size": 1,
        })).collect::<Vec<_>>()});
        assert_eq!(image_urls(&many).len(), MAX_IMAGES);
    }

    #[test]
    fn long_messages_are_cut() {
        let incoming = accept(&message(&"é".repeat(3000), None), &me(None)).unwrap();
        assert_eq!(incoming.content.chars().count(), MAX_CONTENT_CHARS);
    }
}
