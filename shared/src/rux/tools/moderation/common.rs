//! plugged.host: what the moderation tools share: who an action is on ([`Target`]), what a
//! moderator may not touch (staff, admins, themselves, the mod log, channels they cannot see),
//! the channel an order means, and the pieces of their schemas and answers.

use super::super::ToolError;
use crate::rux::{access, context::Context, discord::THREADS, models::Case, resolver};
use serde_json::{Map, Value, json};

/// Someone an action is about: a member of the server, or (someone who left or was banned) any
/// Discord user.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: String,
    pub name: String,
    /// Their member object, while they are in the server.
    pub member: Option<Value>,
}

impl Target {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            member: None,
        }
    }

    /// Their role ids, when they are a member.
    pub fn roles(&self) -> Option<Vec<String>> {
        self.member.as_ref().map(|member| role_ids(member))
    }

    pub fn mention(&self) -> String {
        format!("<@{}>", self.id)
    }
}

impl From<resolver::Member> for Target {
    fn from(member: resolver::Member) -> Self {
        Self {
            id: member.id,
            name: member.name,
            member: Some(member.member),
        }
    }
}

impl From<resolver::User> for Target {
    fn from(user: resolver::User) -> Self {
        Self::new(user.id, user.name)
    }
}

/// A member object's role ids.
pub fn role_ids(member: &Value) -> Vec<String> {
    member["roles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|role| role.as_str().map(str::to_string))
        .collect()
}

pub const PROTECTED: &str = "Moderators cannot do this to staff, admins, other moderators or themselves. Tell them to ask an admin.";

/// Moderators may not act on staff (the owner, people with moderation permissions, other
/// moderators), on admins whatever their Discord roles, on themselves or on the bot; admins may.
pub async fn guard(context: &Context, target: &Target) -> Result<(), ToolError> {
    if context.is_admin() {
        return Ok(());
    }

    if is_protected(context, &target.id, target.roles().as_deref()).await {
        return Err(ToolError::invalid(PROTECTED));
    }

    Ok(())
}

/// Whether a moderator must leave this person alone ([`guard`]). Works for people who are not
/// in the server too: their roles are looked up, and admins are checked either way. When the
/// account directory cannot say, they are left alone (the safe mistake).
pub async fn is_protected(context: &Context, discord_id: &str, roles: Option<&[String]>) -> bool {
    if discord_id.is_empty()
        || discord_id == context.actor_discord_id
        || discord_id == context.me.user_id
        || access::is_staff(
            &context.state,
            &context.rest,
            &context.guild_id,
            discord_id,
            roles,
        )
        .await
    {
        return true;
    }

    match access::actor(&context.state, discord_id).await {
        Ok(actor) => actor.is_some_and(|actor| actor.is_admin),
        Err(err) => {
            tracing::debug!("rux: cannot tell whether {discord_id} is an admin: {err}");
            true
        }
    }
}

/// Moderators may not act on the server's mod log channel, where rux records what they do.
pub fn guard_channel(context: &Context, channel: &Value) -> Result<(), ToolError> {
    let mod_log = context
        .guild()
        .and_then(|guild| guild.mod_log_channel_id.as_deref());

    if !context.is_admin() && mod_log.is_some() && channel["id"].as_str() == mod_log {
        return Err(ToolError::invalid(
            "Moderators cannot do this in the mod log channel. Tell them to ask an admin.",
        ));
    }

    Ok(())
}

/// Moderators only read, delete or change things in channels they can open on Discord
/// themselves: rux sees every channel, staff-only ones included.
pub async fn guard_visible(context: &Context, channel: &Value) -> Result<(), ToolError> {
    if context.is_admin()
        || access::can_view(
            &context.rest,
            &context.guild_id,
            &context.actor_discord_id,
            channel,
        )
        .await
    {
        return Ok(());
    }

    Err(ToolError::invalid(format!(
        "This moderator cannot see <#{}> on Discord, so {} does not do anything there for them.",
        channel["id"].as_str().unwrap_or_default(),
        context.config.settings.name
    )))
}

/// The channel an order names (`None`: the one it was written in; a DM must name one) among the
/// channels that hold messages of `types`, threads included.
pub async fn channel(
    context: &Context,
    input: Option<&str>,
    types: &[i64],
) -> Result<Value, ToolError> {
    let input = match input {
        Some(input) => input.to_string(),
        None if context.direct => {
            return Err(ToolError::invalid(
                "This order came by DM, so say which channel.",
            ));
        }
        None => context.channel_id.clone(),
    };

    match resolver::channel(&context.rest, &context.guild_id, &input, Some(types)).await {
        Ok(channel) => Ok(channel),
        // threads are not in the server's channel list
        Err(ToolError::Invalid(message)) => match resolver::snowflake(&input, &["#"]) {
            Some(id) => {
                let channel =
                    resolver::message_channel(&context.rest, &context.guild_id, &id).await?;
                let kind = channel["type"].as_i64().unwrap_or(-1);
                if types.contains(&kind) || THREADS.contains(&kind) {
                    Ok(channel)
                } else {
                    Err(ToolError::Invalid(message))
                }
            }
            None => Err(ToolError::Invalid(message)),
        },
        Err(err) => Err(err),
    }
}

/// Channel types that hold messages: text, voice and stage chats, announcement.
pub const MESSAGE_CHANNELS: [i64; 4] = [0, 2, 5, 13];

/// A member, or for someone who left (or was banned) the Discord user behind a mention or id.
pub async fn member_or_user(context: &Context, input: &str) -> Result<Target, ToolError> {
    match resolver::member(&context.rest, &context.guild_id, input).await {
        Ok(member) => Ok(member.into()),
        Err(ToolError::Invalid(message)) => {
            if resolver::snowflake(input, &["@!", "@"]).is_some() {
                Ok(resolver::user(&context.rest, input).await?.into())
            } else {
                Err(ToolError::Invalid(message))
            }
        }
        Err(err) => Err(err),
    }
}

/// A member of the server.
pub async fn member(context: &Context, input: &str) -> Result<Target, ToolError> {
    Ok(resolver::member(&context.rest, &context.guild_id, input)
        .await?
        .into())
}

pub const MEMBER: &str = "The member: <@mention>, user id, or their name.";

pub fn member_property(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

pub fn reason_property() -> Value {
    json!({
        "type": "string",
        "description": "Why, in a few words. Shown to the member and in the audit log.",
    })
}

pub fn channel_property() -> Value {
    json!({
        "type": "string",
        "description": "Channel (#name, <#id> or id). Leave out for the channel the order was written in.",
    })
}

/// An object without its `null` fields.
pub fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, value)| !value.is_null())
                .collect::<Map<String, Value>>(),
        ),
        other => other,
    }
}

/// What a moderation tool answers: the case, then `extra` (its `null` fields left out).
pub fn case_result(case: &Case, extra: Value) -> Value {
    let mut result = json!({
        "case": case.id,
        "action": case.action,
        "member": case.target_id.as_ref().map(|id| format!("<@{id}>")),
    });
    if let (Some(result), Value::Object(extra)) = (result.as_object_mut(), without_nulls(extra)) {
        result.extend(extra);
    }

    result
}

/// A Discord message's time.
pub fn sent_at(message: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    message["timestamp"]
        .as_str()
        .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.with_timezone(&chrono::Utc))
}

/// Words that lift a timeout, remove a nickname or make a ban permanent.
pub fn is_off(value: &str, words: &[&str]) -> bool {
    words.contains(&value.trim().to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_know_their_roles() {
        let target = Target {
            id: "1".into(),
            name: "Ann".into(),
            member: Some(json!({"roles": ["2", 3, "4"]})),
        };
        assert_eq!(target.roles(), Some(vec!["2".to_string(), "4".to_string()]));
        assert_eq!(Target::new("1", "Ann").roles(), None);
        assert_eq!(target.mention(), "<@1>");
    }

    #[test]
    fn nulls_are_left_out() {
        assert_eq!(
            without_nulls(json!({"a": 1, "b": null, "c": false})),
            json!({"a": 1, "c": false})
        );
        assert_eq!(without_nulls(json!([null])), json!([null]));
    }

    #[test]
    fn message_times() {
        let message = json!({"timestamp": "2026-10-04T10:00:00.123000+00:00"});
        assert_eq!(sent_at(&message).unwrap().timestamp(), 1_791_108_000);
        assert_eq!(sent_at(&json!({})), None);
        assert!(is_off(" Off ", &["off", "0"]));
        assert!(!is_off("2h", &["off", "0"]));
    }
}
