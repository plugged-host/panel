//! plugged.host: acting on a message the checks found breaking a standing rule (what the rule
//! says: warn, time out, delete, kick or ban) or one of a watched channel's guidelines (deleted,
//! and its author warned), plus a short notice in the channel, so the member sees it even with
//! DMs closed and others learn the rule. Whether the message breaks it is the checks' call (the
//! watch); this only acts, on the message's own server, never on staff or the bot, and writes the
//! same cases an order would.

use super::super::{
    ToolError,
    moderation::{
        common::Target,
        service::{self, BanOptions, MAX_TIMEOUT_SECONDS},
    },
};
use crate::rux::{
    access,
    config::Config,
    context::{Context, Me},
    discord::Rest,
    models::{Rule, WatchItem},
};
use serde_json::{Value, json};
use std::sync::Arc;

/// A timeout rule without a duration times out for an hour.
const DEFAULT_TIMEOUT_SECONDS: i64 = 3_600;

/// The message a check caught.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub guild_id: String,
    pub message_id: String,
    pub channel_id: String,
    pub author_id: String,
    pub author_name: String,
    /// The author's role ids, when the message carried them (spares a member lookup).
    pub roles: Option<Vec<String>>,
    pub content: Option<String>,
}

impl Hit {
    /// A queued message of a watched channel (rows without a server are the main server's).
    pub fn from_item(item: &WatchItem, config: &Config) -> Option<Self> {
        let guild = config.guild_for(item.guild_id.as_deref())?;

        Some(Self {
            guild_id: guild.id.clone(),
            message_id: item.message_id.clone(),
            channel_id: item.channel_id.clone(),
            author_id: item.author_id.clone(),
            author_name: item
                .author_name
                .clone()
                .unwrap_or_else(|| item.author_id.clone()),
            roles: item.roles.as_ref().and_then(Value::as_array).map(|roles| {
                roles
                    .iter()
                    .filter_map(|role| role.as_str().map(str::to_string))
                    .collect()
            }),
            content: Some(item.content.clone()),
        })
    }

    fn target(&self) -> Target {
        Target::new(&self.author_id, &self.author_name)
    }

    /// The ids are Discord's, and it is a server rux works on.
    fn check(&self, config: &Config) -> Result<(), ToolError> {
        let ids = [
            &self.guild_id,
            &self.message_id,
            &self.channel_id,
            &self.author_id,
        ];
        if !ids.iter().all(|id| crate::discord::is_snowflake(id)) {
            return Err(ToolError::invalid("The caught message has no valid ids."));
        }
        if !config.serves(Some(&self.guild_id)) {
            return Err(ToolError::invalid(
                "rux does not work on the server of this message.",
            ));
        }

        Ok(())
    }
}

/// Staff and the bot are never acted on by a check.
async fn spared(context: &Context, hit: &Hit) -> bool {
    hit.author_id == context.me.user_id
        || access::is_staff(
            &context.state,
            &context.rest,
            &hit.guild_id,
            &hit.author_id,
            hit.roles.as_deref(),
        )
        .await
}

/// "…, you were warned: …", and the step the warnings reached.
fn warned_line(context: &Context, hit: &Hit, reason: &str, warned: &service::Warned) -> String {
    let line = context.say(
        "rule.warn",
        &[
            ("member", &format!("<@{}>", hit.author_id)),
            ("reason", reason),
        ],
    );
    if warned.escalation.is_none() {
        return line;
    }

    format!(
        "{line} {}",
        crate::rux::say::say_count(
            context.config.locale(),
            "notice.warnings_reached",
            warned.warnings,
            &[],
        )
    )
}

/// Acts on a message that breaks `rule`, as the rule's author ("rule #N" in the logs), on the
/// rule's own server only. The cooldown per rule and member is the caller's.
pub async fn enforce(
    state: &crate::State,
    config: Arc<Config>,
    rest: Rest,
    me: Me,
    rule: &Rule,
    hit: &Hit,
) -> Result<(), ToolError> {
    hit.check(&config)?;
    if hit.guild_id != rule.guild_id {
        return Err(ToolError::Invalid(format!(
            "Rule #{} belongs to another server than the message.",
            rule.id
        )));
    }

    let mut context = Context::itself(
        state,
        config,
        rest,
        me,
        &rule.guild_id,
        &hit.channel_id,
        hit.content.clone(),
    );
    context.actor_discord_id = rule.created_by_discord_id.clone();
    context.actor_name = format!("rule #{}", rule.id);
    context.actor_account_id = rule.created_by_account_id;
    context.actor_user_uuid = rule.created_by_user_uuid;
    context.source_message_id = Some(hit.message_id.clone());
    if spared(&context, hit).await {
        return Ok(());
    }

    let target = hit.target();
    let reason = rule.told();
    let mut deleted = false;
    if rule.delete_message || rule.action == "delete" {
        match service::delete_message(
            &context,
            &hit.channel_id,
            &hit.message_id,
            &target,
            Some(reason.as_str()),
            Some(rule.id),
            rule.action == "delete",
        )
        .await
        {
            Ok(_) => deleted = true,
            Err(ToolError::Discord(err)) => tracing::warn!(
                rule = rule.id,
                "rux: could not delete a message a rule caught: {}",
                err.message
            ),
            Err(err) => return Err(err),
        }
    }

    let notice = match rule.action.as_str() {
        "warn" => {
            let warned = service::warn(
                &context,
                &target,
                Some(reason.as_str()),
                Some(rule.id),
                true,
            )
            .await?;
            Some(warned_line(&context, hit, &reason, &warned))
        }
        "timeout" => {
            let seconds = rule
                .duration_seconds
                .map(i64::from)
                .filter(|seconds| *seconds > 0)
                .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
                .min(MAX_TIMEOUT_SECONDS);
            service::timeout(
                &context,
                &target,
                seconds,
                Some(reason.as_str()),
                true,
                Some(rule.id),
            )
            .await?;
            Some(context.say(
                "rule.timeout",
                &[
                    ("member", &format!("<@{}>", hit.author_id)),
                    ("reason", &reason),
                ],
            ))
        }
        "kick" => {
            service::kick(&context, &target, Some(reason.as_str()), Some(rule.id)).await?;
            Some(context.say(
                "rule.acted",
                &[
                    ("member", &hit.author_name),
                    ("reason", &context.say("log.kick", &[])),
                ],
            ))
        }
        "ban" => {
            service::ban(
                &context,
                &target,
                Some(reason.as_str()),
                BanOptions {
                    in_server: true,
                    seconds: rule
                        .duration_seconds
                        .map(i64::from)
                        .filter(|seconds| *seconds > 0),
                    automatic: true,
                    rule_id: Some(rule.id),
                    ..Default::default()
                },
            )
            .await?;
            Some(context.say(
                "rule.acted",
                &[
                    ("member", &hit.author_name),
                    ("reason", &context.say("log.ban", &[])),
                ],
            ))
        }
        _ => None,
    };

    sqlx::query(
        "UPDATE rux_rules SET actions = actions + 1, last_action = $2, updated = NOW() WHERE id = $1",
    )
    .bind(rule.id)
    .bind(chrono::Utc::now().naive_utc())
    .execute(state.database.write())
    .await?;

    notice_in_channel(&context, hit, notice, deleted).await;

    Ok(())
}

/// A message in a watched channel that breaks one of the always-on guidelines
/// ([`super::model::guidelines`]; `guideline` is its text, as the member is told): deleted, and
/// its author warned, by rux itself on the message's server.
pub async fn enforce_guideline(
    state: &crate::State,
    config: Arc<Config>,
    rest: Rest,
    me: Me,
    guideline: &str,
    hit: &Hit,
) -> Result<(), ToolError> {
    hit.check(&config)?;
    let mut context = Context::itself(
        state,
        config,
        rest,
        me,
        &hit.guild_id,
        &hit.channel_id,
        hit.content.clone(),
    );
    context.source_message_id = Some(hit.message_id.clone());
    if spared(&context, hit).await {
        return Ok(());
    }

    let target = hit.target();
    let deleted = match service::delete_message(
        &context,
        &hit.channel_id,
        &hit.message_id,
        &target,
        Some(guideline),
        None,
        false,
    )
    .await
    {
        Ok(_) => true,
        Err(ToolError::Discord(err)) => {
            tracing::warn!(
                message = %hit.message_id,
                "rux: could not delete a message that broke a guideline: {}",
                err.message
            );
            false
        }
        Err(err) => return Err(err),
    };

    let warned = service::warn(&context, &target, Some(guideline), None, true).await?;
    let line = warned_line(&context, hit, guideline, &warned);
    notice_in_channel(&context, hit, Some(line), deleted).await;

    Ok(())
}

/// The notice under the message (or in its place when it was deleted), pinging only its author.
pub fn notice_payload(hit: &Hit, notice: &str, deleted: bool) -> Value {
    let mut payload = json!({
        "content": crate::rux::text::cut(notice, 2000, ""),
        "allowed_mentions": { "parse": [], "users": [hit.author_id] },
    });
    if !deleted {
        payload["message_reference"] =
            json!({ "message_id": hit.message_id, "fail_if_not_exists": false });
    }

    payload
}

async fn notice_in_channel(context: &Context, hit: &Hit, notice: Option<String>, deleted: bool) {
    let Some(notice) = notice else {
        return;
    };

    if let Err(err) = context
        .rest
        .create_message(&hit.channel_id, &notice_payload(hit, &notice, deleted))
        .await
    {
        tracing::warn!(
            message = %hit.message_id,
            "rux: could not post a moderation notice: {}",
            err.message
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit() -> Hit {
        Hit {
            guild_id: "100000000000000001".into(),
            message_id: "100000000000000005".into(),
            channel_id: "100000000000000004".into(),
            author_id: "100000000000000006".into(),
            author_name: "ann".into(),
            roles: None,
            content: Some("hello".into()),
        }
    }

    #[test]
    fn notices_ping_only_the_author() {
        let payload = notice_payload(&hit(), "<@100000000000000006>, you were warned: no.", false);
        assert_eq!(
            payload["allowed_mentions"],
            json!({ "parse": [], "users": ["100000000000000006"] })
        );
        assert_eq!(
            payload["message_reference"]["message_id"],
            "100000000000000005"
        );

        // a deleted message cannot be replied to
        let payload = notice_payload(&hit(), "x", true);
        assert!(payload.get("message_reference").is_none());
    }

    #[test]
    fn caught_messages_need_discord_ids() {
        let config = crate::rux::tools::giveaways::service::tests::config("en");
        assert!(hit().check(&config).is_ok());

        let mut other = hit();
        other.guild_id = "100000000000000099".into();
        assert!(other.check(&config).is_err());

        let mut broken = hit();
        broken.author_id = "ann".into();
        assert!(broken.check(&config).is_err());
    }
}
