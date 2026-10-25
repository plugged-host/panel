//! plugged.host: the moderation tools about messages: reading them, deleting chosen ones,
//! purging the latest, and (admins only) posting, editing and DMing as the bot. A moderator only
//! reads or deletes where they can see on Discord, never in the mod log, never staff messages.

use super::{
    super::{Tool, ToolError, ToolResult, boolean, int, required, schema, string, strings},
    common::{
        self, MESSAGE_CHANNELS, case_result, channel_property, member_property, reason_property,
        without_nulls,
    },
    service,
};
use crate::rux::{access::Level, context::Context, resolver, text};
use serde_json::{Value, json};

/// Characters of message text `read_messages` answers with, so it fits in a tool's output.
const TEXT_BUDGET: usize = 5000;
/// Messages `delete_messages` takes at once.
const DELETE_MAX: usize = 25;
const TOO_LONG: &str = "The message is longer than Discord's 2000 characters. Shorten it.";

/// The channel a message given by id is looked for in: the one named, else the order's (a DM has
/// none).
fn message_channel_input(context: &Context, arguments: &Value) -> String {
    string(arguments, "channel").unwrap_or_else(|| {
        if context.direct {
            String::new()
        } else {
            context.channel_id.clone()
        }
    })
}

/// One message for the model: "id · time · author: text".
pub fn line(message: &Value, limit: usize, zone: chrono_tz::Tz) -> String {
    let author = &message["author"];
    let name = author["global_name"]
        .as_str()
        .or(author["username"].as_str())
        .unwrap_or("?");
    let said = text::message_text(message);
    let attachments = message["attachments"].as_array().map_or(0, Vec::len);
    let time = common::sent_at(message)
        .map(|sent| {
            sent.with_timezone(&zone)
                .format("%a %-d %b %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "?".into());

    let mut line = format!(
        "{} · {time} · <@{}> ({}{})",
        message["id"].as_str().unwrap_or("?"),
        author["id"].as_str().unwrap_or("?"),
        text::untrusted(name, 40),
        if author["bot"].as_bool() == Some(true) {
            ", a bot"
        } else {
            ""
        }
    );
    if let Some(replied) = message["message_reference"]["message_id"].as_str() {
        line.push_str(&format!(", replying to {replied}"));
    }
    if message["pinned"].as_bool() == Some(true) {
        line.push_str(", pinned");
    }
    line.push_str(": ");
    line.push_str(&if said.is_empty() {
        "(no text)".to_string()
    } else {
        text::untrusted(&said, limit)
    });
    if attachments > 0 {
        line.push_str(&format!(
            " [{attachments} attachment{}]",
            if attachments > 1 { "s" } else { "" }
        ));
    }

    line
}

pub struct ReadMessages;

#[async_trait::async_trait]
impl Tool for ReadMessages {
    fn name(&self) -> &'static str {
        "read_messages"
    }

    fn description(&self, _context: &Context) -> String {
        "Read messages: the latest in a channel (optionally only one member's), or one message by its link. Gives each message's id, time, author and text (embeds too): to find a message to delete, see what someone said, or read the rules posted in the server.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "channel": channel_property(),
                "from": member_property("Only this person's messages (optional)."),
                "message": { "type": "string", "description": "Just this message: its link, or its id (with channel)." },
                "count": { "type": "integer", "description": "How many of the latest messages, 1 to 50. Default 15." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let zone = context.config.tz();
        let times_in = context.config.settings.timezone.to_string();

        if let Some(input) = string(arguments, "message") {
            let found = resolver::message(
                &context.rest,
                &context.guild_id,
                &input,
                &message_channel_input(context, arguments),
            )
            .await?;
            common::guard_visible(context, &found.channel).await?;

            return Ok(ToolResult::json(&json!({
                "channel": format!("<#{}>", found.channel["id"].as_str().unwrap_or_default()),
                "times_in": times_in,
                "messages": [line(&found.message, 4000, zone)],
            })));
        }

        let channel = common::channel(
            context,
            string(arguments, "channel").as_deref(),
            &MESSAGE_CHANNELS,
        )
        .await?;
        common::guard_visible(context, &channel).await?;
        let channel_id = channel["id"].as_str().unwrap_or_default().to_string();

        let count = int(arguments, "count").unwrap_or(15).clamp(1, 50) as usize;
        let from = match string(arguments, "from") {
            Some(input) => Some(common::member_or_user(context, &input).await?.id),
            None => None,
        };

        // one person's messages are looked for in the last 500; otherwise one page is enough
        let limit = if from.is_none() {
            (count + 1).min(100)
        } else {
            100
        };
        let pages = if from.is_none() { 1 } else { 5 };
        let mut picked: Vec<Value> = Vec::new();
        let mut before: Option<String> = None;

        'pages: for _ in 0..pages {
            let mut query = vec![("limit", limit.to_string())];
            if let Some(before) = &before {
                query.push(("before", before.clone()));
            }
            let page = context.rest.messages(&channel_id, &query).await?;

            for message in &page {
                let id = message["id"].as_str().unwrap_or_default().to_string();
                before = Some(id.clone());
                if context.source_message_id.as_deref() == Some(id.as_str())
                    || from
                        .as_deref()
                        .is_some_and(|from| message["author"]["id"].as_str() != Some(from))
                {
                    continue;
                }

                picked.push(message.clone());
                if picked.len() >= count {
                    break 'pages;
                }
            }

            if page.len() < limit {
                break;
            }
        }

        let each = (TEXT_BUDGET / picked.len().max(1)).clamp(120, 1500);
        let note = picked.is_empty().then(|| match &from {
            Some(from) => format!("No messages from <@{from}> among the last 500."),
            None => "The channel has no messages.".to_string(),
        });

        Ok(ToolResult::json(&without_nulls(json!({
            "channel": format!("<#{channel_id}>"),
            "times_in": times_in,
            "messages": picked.iter().rev().map(|message| line(message, each, zone)).collect::<Vec<_>>(),
            "note": note,
        }))))
    }
}

pub struct DeleteMessages;

#[async_trait::async_trait]
impl Tool for DeleteMessages {
    fn name(&self) -> &'static str {
        "delete_messages"
    }

    fn description(&self, _context: &Context) -> String {
        "Delete particular messages, of any age: by link, or by id (read_messages shows ids; a replied-to message's id is in the order). For \"the last N messages\" use purge_messages.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "messages": { "type": "array", "items": { "type": "string" }, "description": "Message links or ids, up to 25." },
                "channel": { "type": "string", "description": "The channel of messages given by id (#name, <#id> or id). Leave out for the channel the order was written in." },
                "reason": reason_property(),
            }),
            &["messages"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let mut refs: Vec<String> = Vec::new();
        for reference in strings(arguments, "messages") {
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
        if refs.is_empty() {
            return Err(ToolError::invalid(
                "Say which messages: links, or ids with their channel (read_messages finds them).",
            ));
        }
        if refs.len() > DELETE_MAX {
            return Err(ToolError::invalid(format!(
                "At most {DELETE_MAX} messages at a time. purge_messages deletes the latest N in one go."
            )));
        }

        let channel_input = message_channel_input(context, arguments);
        let mut by_channel: Vec<(Value, Vec<Value>)> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();

        for reference in &refs {
            let found = match resolver::message(
                &context.rest,
                &context.guild_id,
                reference,
                &channel_input,
            )
            .await
            {
                Ok(found) => found,
                Err(ToolError::Invalid(why)) => {
                    skipped.push(format!("{reference}: {why}"));
                    continue;
                }
                Err(err) => return Err(err),
            };
            common::guard_channel(context, &found.channel)?;
            common::guard_visible(context, &found.channel).await?;

            let message_id = found.message["id"].as_str().unwrap_or_default().to_string();
            let author = found.message["author"]["id"].as_str().unwrap_or_default();
            if !context.is_admin() && common::is_protected(context, author, None).await {
                skipped.push(format!(
                    "{message_id}: written by staff, an admin, the bot or this moderator, so a moderator cannot delete it"
                ));
                continue;
            }

            let channel_id = found.channel["id"].as_str().unwrap_or_default().to_string();
            match by_channel
                .iter_mut()
                .find(|(channel, _)| channel["id"].as_str() == Some(channel_id.as_str()))
            {
                Some((_, messages)) => {
                    if !messages
                        .iter()
                        .any(|message| message["id"].as_str() == Some(message_id.as_str()))
                    {
                        messages.push(found.message);
                    }
                }
                None => by_channel.push((found.channel, vec![found.message])),
            }
        }

        if by_channel.is_empty() {
            return Err(ToolError::Invalid(format!(
                "Nothing was deleted. {}",
                skipped.join(" ")
            )));
        }

        let reason = string(arguments, "reason");
        let mut deleted = 0;
        let mut cases = Vec::new();
        for (channel, messages) in &by_channel {
            let (count, written) =
                service::delete_messages(context, channel, messages, reason.as_deref()).await?;
            deleted += count;
            cases.extend(written.iter().map(|case| {
                format!(
                    "#{} (<@{}>)",
                    case.id,
                    case.target_id.as_deref().unwrap_or_default()
                )
            }));
        }

        Ok(ToolResult::json(&without_nulls(json!({
            "deleted": deleted,
            "channels": by_channel
                .iter()
                .map(|(channel, _)| format!("<#{}>", channel["id"].as_str().unwrap_or_default()))
                .collect::<Vec<_>>(),
            "cases": cases,
            "skipped": (!skipped.is_empty()).then_some(skipped),
        }))))
    }
}

pub struct PurgeMessages;

#[async_trait::async_trait]
impl Tool for PurgeMessages {
    fn name(&self) -> &'static str {
        "purge_messages"
    }

    fn description(&self, _context: &Context) -> String {
        "Delete the most recent messages in a channel, optionally only one member's. Pinned messages and messages older than 14 days are kept.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "count": { "type": "integer", "description": "How many messages (1 to 200)." },
                "channel": channel_property(),
                "from": member_property("Only this person's messages (optional)."),
                "reason": reason_property(),
            }),
            &["count"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let count =
            int(arguments, "count").ok_or_else(|| ToolError::invalid("count is required."))?;
        let channel_input = string(arguments, "channel");
        if channel_input.is_none() && context.direct {
            return Err(ToolError::invalid(
                "This order came by DM, so say which channel to clean up.",
            ));
        }
        let channel = common::channel(context, channel_input.as_deref(), &MESSAGE_CHANNELS).await?;
        // someone who left (or was just banned) is not a member any more
        let from = match string(arguments, "from") {
            Some(input) => Some(common::member_or_user(context, &input).await?),
            None => None,
        };

        common::guard_channel(context, &channel)?;
        common::guard_visible(context, &channel).await?;
        if let Some(from) = &from {
            common::guard(context, from).await?;
        }

        // a moderator's channel-wide cleanup keeps staff and admin messages
        let keep_protected = from.is_none() && !context.is_admin();
        let (deleted, case) = service::purge(
            context,
            &channel,
            count,
            from.as_ref(),
            string(arguments, "reason").as_deref(),
            keep_protected,
        )
        .await?;

        Ok(ToolResult::json(&case_result(
            &case,
            json!({
                "deleted": deleted,
                "channel": format!("<#{}>", channel["id"].as_str().unwrap_or_default()),
            }),
        )))
    }
}

pub struct SendMessage;

#[async_trait::async_trait]
impl Tool for SendMessage {
    fn name(&self) -> &'static str {
        "send_message"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Post a message as {} in a channel (announcements, rules, replies). Use the admin's wording when they give it.",
            context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "channel": { "type": "string", "description": "Channel (#name, <#id> or id)." },
                "content": { "type": "string", "description": "The message, Discord markdown, up to 2000 characters." },
                "ping_everyone": { "type": "boolean", "description": "Let @everyone/@here and role mentions in the text actually ping. Only when the admin asked for a ping." },
            }),
            &["channel", "content"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let channel = resolver::postable_channel(
            &context.rest,
            &context.guild_id,
            &required(arguments, "channel")?,
        )
        .await?;
        let content = required(arguments, "content")?;
        if content.chars().count() > 2000 {
            return Err(ToolError::invalid(TOO_LONG));
        }

        let parse = if boolean(arguments, "ping_everyone", false) {
            json!(["users", "roles", "everyone"])
        } else {
            json!(["users"])
        };
        let channel_id = channel["id"].as_str().unwrap_or_default();
        let message = context
            .rest
            .create_message(
                channel_id,
                &json!({ "content": content, "allowed_mentions": { "parse": parse } }),
            )
            .await?;

        Ok(ToolResult::json(&json!({
            "posted": true,
            "channel": format!("<#{channel_id}>"),
            "link": format!(
                "https://discord.com/channels/{}/{channel_id}/{}",
                context.guild_id,
                message["id"].as_str().unwrap_or_default()
            ),
        })))
    }
}

pub struct EditMessage;

#[async_trait::async_trait]
impl Tool for EditMessage {
    fn name(&self) -> &'static str {
        "edit_message"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Change the text of a message {} posted (fix an announcement), and/or pin or unpin any message.",
            context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "message": { "type": "string", "description": "The message: its link, or its id (with channel)." },
                "channel": { "type": "string", "description": "The channel, when the message is given by id. Leave out for the channel the order was written in." },
                "content": { "type": "string", "description": "The new text, replacing the old (the bot's own messages only), up to 2000 characters." },
                "pinned": { "type": "boolean", "description": "true pins the message, false unpins it." },
            }),
            &["message"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let found = resolver::message(
            &context.rest,
            &context.guild_id,
            &required(arguments, "message")?,
            &message_channel_input(context, arguments),
        )
        .await?;
        let channel_id = found.channel["id"].as_str().unwrap_or_default();
        let message_id = found.message["id"].as_str().unwrap_or_default();
        let mut done = serde_json::Map::new();
        done.insert(
            "message".into(),
            json!(format!(
                "https://discord.com/channels/{}/{channel_id}/{message_id}",
                context.guild_id
            )),
        );

        if let Some(content) = string(arguments, "content") {
            let name = &context.config.settings.name;
            if found.message["author"]["id"].as_str() != Some(context.me.user_id.as_str()) {
                return Err(ToolError::invalid(format!(
                    "{name} can only edit its own messages. It can delete this one (delete_messages) or post a new one."
                )));
            }
            let giveaway: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM rux_giveaways WHERE message_id = $1)",
            )
            .bind(message_id)
            .fetch_one(context.state.database.read())
            .await?;
            if giveaway {
                return Err(ToolError::invalid(
                    "That is a giveaway post: change the giveaway with edit_giveaway instead.",
                ));
            }
            if content.chars().count() > 2000 {
                return Err(ToolError::invalid(TOO_LONG));
            }

            context
                .rest
                .edit_message(
                    channel_id,
                    message_id,
                    &json!({ "content": content, "allowed_mentions": { "parse": ["users"] } }),
                )
                .await?;
            done.insert("edited".into(), json!(true));
        }

        if !arguments["pinned"].is_null() {
            let pinned = boolean(arguments, "pinned", false);
            context
                .rest
                .pin(
                    channel_id,
                    message_id,
                    pinned,
                    Some(&context.audit_reason(None)),
                )
                .await?;
            done.insert("pinned".into(), json!(pinned));
        }

        if done.len() == 1 {
            return Err(ToolError::invalid(
                "Say the new text (content), or whether to pin or unpin it.",
            ));
        }

        Ok(ToolResult::json(&Value::Object(done)))
    }
}

pub struct SendDm;

#[async_trait::async_trait]
impl Tool for SendDm {
    fn name(&self) -> &'static str {
        "send_dm"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Send a member a direct message as {}. It fails only if they turned off DMs from server members.",
            context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property(common::MEMBER),
                "content": { "type": "string", "description": "The message, Discord markdown, up to 2000 characters." },
            }),
            &["member", "content"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        let content = required(arguments, "content")?;
        if content.chars().count() > 2000 {
            return Err(ToolError::invalid(TOO_LONG));
        }

        let sent = context
            .rest
            .send_dm(
                &target.id,
                &json!({ "content": content, "allowed_mentions": { "parse": [] } }),
            )
            .await;

        Ok(if sent {
            ToolResult::json(&json!({ "sent": true, "to": target.mention() }))
        } else {
            ToolResult::error(format!(
                "{} does not accept DMs from {} (they turned off DMs from server members).",
                target.mention(),
                context.config.settings.name
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_read_as_one_line() {
        let message = json!({
            "id": "100000000000000005",
            "timestamp": "2026-10-04T10:00:00+00:00",
            "author": {"id": "100000000000000002", "username": "ann", "global_name": "<Ann>"},
            "content": "hi <@100000000000000003>",
            "message_reference": {"message_id": "100000000000000004"},
            "pinned": true,
            "attachments": [{}, {}],
        });
        assert_eq!(
            line(&message, 100, chrono_tz::Europe::Paris),
            "100000000000000005 · Sun 4 Oct 12:00 · <@100000000000000002> (‹Ann›), replying to 100000000000000004, pinned: hi @100000000000000003 [2 attachments]"
        );

        let bot =
            json!({"id": "1", "author": {"id": "2", "username": "b", "bot": true}, "content": ""});
        assert_eq!(
            line(&bot, 100, chrono_tz::UTC),
            "1 · ? · <@2> (b, a bot): (no text)"
        );
    }
}
