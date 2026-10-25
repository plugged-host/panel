//! plugged.host: the tools for what rux enforces by itself (`add_rule`, `list_rules`,
//! `edit_rule`, `remove_rule`, `watch_channel`) and who moderates with it
//! (`set_moderator_role`). Each acts on the order's server only and is logged as a case; listing
//! is open to moderators, everything else is for admins.

use super::{
    super::{
        Tool, ToolError, ToolResult, boolean, int,
        moderation::{
            common::is_off,
            service::{self, MAX_BAN_SECONDS, MAX_TIMEOUT_SECONDS},
        },
        required, schema, string, strings,
    },
    model::{self, ACTIONS},
    watches,
};
use crate::rux::{
    access::{self, Level},
    context::Context,
    models::{RULE_COLUMNS, Rule},
    resolver, text, time,
};
use serde_json::{Map, Value, json};

/// The channels a rule can be limited to: text, announcement and forum channels.
const RULE_CHANNELS: [i64; 3] = [0, 5, 15];
/// Words that make a ban rule's ban permanent.
const PERMANENT: [&str; 5] = ["0", "permanent", "forever", "none", "off"];

fn action(arguments: &Value) -> Result<Option<String>, ToolError> {
    let Some(action) = string(arguments, "action").map(|action| action.to_lowercase()) else {
        return Ok(None);
    };
    if !ACTIONS.contains(&action.as_str()) {
        return Err(ToolError::Invalid(format!(
            "action must be one of {}.",
            ACTIONS.join(", ")
        )));
    }

    Ok(Some(action))
}

/// How long a rule's timeout or ban lasts: a timeout from 1 minute to 28 days (1 hour without
/// one), a ban for good (`None`) or from 1 minute to 10 years; nothing for the other actions.
pub fn rule_seconds(action: &str, seconds: Option<i64>) -> Option<i64> {
    match action {
        "timeout" => Some(seconds.unwrap_or(3_600).clamp(60, MAX_TIMEOUT_SECONDS)),
        "ban" => seconds
            .filter(|seconds| *seconds > 0)
            .map(|seconds| seconds.clamp(60, MAX_BAN_SECONDS)),
        _ => None,
    }
}

async fn channels(context: &Context, inputs: &[String]) -> Result<Option<Value>, ToolError> {
    let mut ids: Vec<String> = Vec::new();
    for input in inputs {
        let channel = resolver::channel(
            &context.rest,
            &context.guild_id,
            input,
            Some(&RULE_CHANNELS),
        )
        .await?;
        let id = channel["id"].as_str().unwrap_or_default().to_string();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    Ok((!ids.is_empty()).then(|| json!(ids)))
}

/// `{flag: true}`, then a rule's summary, then the keywords dropped as too common.
fn rule_result(flag: (&str, Value), rule: &Rule, dropped: &[String]) -> Value {
    let mut out = Map::new();
    out.insert(flag.0.to_string(), flag.1);
    if let Value::Object(summary) = model::summary(rule) {
        out.extend(summary);
    }
    if !dropped.is_empty() {
        out.insert("keywords_dropped_as_too_common".into(), json!(dropped));
    }

    Value::Object(out)
}

fn meta(entries: &[(&str, Value)]) -> Map<String, Value> {
    entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

async fn active_rule(context: &Context, arguments: &Value) -> Result<Rule, ToolError> {
    let id = int(arguments, "rule_id")
        .ok_or_else(|| ToolError::invalid("rule_id is required: list_rules shows the numbers."))?;

    sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
        "SELECT {RULE_COLUMNS} FROM rux_rules WHERE id = $1 AND guild_id = $2 AND active"
    )))
    .bind(id)
    .bind(&context.guild_id)
    .fetch_optional(context.state.database.read())
    .await?
    .ok_or_else(|| {
        ToolError::invalid(
            "There is no active rule with that number on this server. Use list_rules.",
        )
    })
}

fn keywords_property(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": description })
}

fn channels_property(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": description })
}

pub struct AddRule;

#[async_trait::async_trait]
impl Tool for AddRule {
    fn name(&self) -> &'static str {
        "add_rule"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Make a standing rule {name} enforces by itself from now on, e.g. \"if anyone asks for new locations, warn them\". Every member message containing one of the keywords (and every message in watched channels) is checked by AI against `when` and `not_when` before {name} acts; that AI sees only the rule and the message, never this conversation. Staff are never affected.",
            name = context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "when": { "type": "string", "description": "What breaks the rule: the message a member writes, completing \"a member's message that …\", clear to someone who never saw this conversation, with what the admin meant (not their exact words). E.g. \"asks staff to add or announce a server location the host does not have yet (a new country, city or region)\"." },
                "not_when": { "type": "string", "description": "What looks close but must be left alone: every exception the admin gave, plus the obvious near misses. E.g. \"asking about, for or how to get a location the host already has; asking which location to pick; talking about locations without asking for a new one\". The check already knows the host's own links and current locations." },
                "keywords": keywords_property("10 to 40 lowercase topic words or short phrases such a message would very likely contain: synonyms, slang, typos, plurals, place names, common non-English words. Every hit costs an AI check, so no everyday words (add, new, when, where, server, pls, coming, any): they match almost every message and are dropped."),
                "action": { "type": "string", "enum": ACTIONS, "description": "What rux does: warn (counts towards the automatic timeout and ban), timeout, delete (the message only), kick or ban." },
                "duration": { "type": "string", "description": "For timeout or ban: how long, like \"1h\" or \"24h\". A ban without one is permanent." },
                "delete_message": { "type": "boolean", "description": "Also delete the message. Default false (true for action delete)." },
                "reason": { "type": "string", "description": "What the member is told, short and friendly, e.g. \"Please don't ask for new locations, we announce them when they come.\"" },
                "channels": channels_property("Only in these channels (#name or id). Leave out for the whole server."),
            }),
            &["when", "keywords", "action"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let when = text::cut(&required(arguments, "when")?, 600, "");
        let Some(action) = action(arguments)? else {
            return Err(ToolError::Invalid(format!(
                "action is required: one of {}.",
                ACTIONS.join(", ")
            )));
        };
        let (keywords, dropped) = model::keywords(&strings(arguments, "keywords"))?;
        let seconds = string(arguments, "duration")
            .filter(|duration| !is_off(duration, &PERMANENT))
            .map(|duration| time::duration(&duration))
            .transpose()
            .map_err(ToolError::Invalid)?;
        let seconds = rule_seconds(&action, seconds);
        let channel_ids = channels(context, &strings(arguments, "channels")).await?;

        let rule = sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
            "INSERT INTO rux_rules
                (guild_id, description, exceptions, keywords, action, duration_seconds, delete_message, reason,
                 channel_ids, active, created_by_discord_id, created_by_account_id, created_by_user_uuid)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $11, $12)
            RETURNING {RULE_COLUMNS}"
        )))
        .bind(&context.guild_id)
        .bind(&when)
        .bind(string(arguments, "not_when").map(|text| text::cut(&text, 600, "")))
        .bind(json!(keywords))
        .bind(&action)
        .bind(seconds.map(|seconds| seconds as i32))
        .bind(action == "delete" || boolean(arguments, "delete_message", false))
        .bind(string(arguments, "reason").map(|text| text::cut(&text, 300, "")))
        .bind(channel_ids)
        .bind(&context.actor_discord_id)
        .bind(context.actor_account_id)
        .bind(context.actor_user_uuid)
        .fetch_one(context.state.database.write())
        .await?;

        service::note(
            context,
            "rule_add",
            Some(rule.description.as_str()),
            meta(&[("rule", json!(rule.id)), ("action", json!(action))]),
        )
        .await?;

        Ok(ToolResult::json(&rule_result(
            ("created", json!(true)),
            &rule,
            &dropped,
        )))
    }
}

/// The settings' spam limits, as the model reads them.
fn spam_limits(context: &Context) -> String {
    let settings = &context.config.settings;

    format!(
        "{} messages within {}s, the same message {} times within {}s, {} or more mentions, or {} or more emoji in one message: deleted, and the member warned at most once per {} minutes",
        settings.spam_flood_messages,
        settings.spam_flood_seconds,
        settings.spam_duplicates,
        settings.spam_duplicate_seconds,
        settings.spam_mentions,
        settings.spam_emoji,
        (settings.rule_cooldown_seconds.max(30) / 60).max(1),
    )
}

pub struct ListRules;

#[async_trait::async_trait]
impl Tool for ListRules {
    fn name(&self) -> &'static str {
        "list_rules"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "The standing rules {} enforces on this server, its watched channels, and the automatic warning steps.",
            context.config.settings.name
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(json!({}), &[])
    }

    async fn execute(
        &self,
        context: &Context,
        _arguments: &Value,
    ) -> Result<ToolResult, ToolError> {
        let settings = &context.config.settings;
        let rules = sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
            "SELECT {RULE_COLUMNS} FROM rux_rules WHERE guild_id = $1 AND active ORDER BY id"
        )))
        .bind(&context.guild_id)
        .fetch_all(context.state.database.read())
        .await?;
        // the rules still show when Discord cannot list the channels right now
        let watched = match watches::watched_channel_ids(
            &context.state,
            &context.rest,
            &context.guild_id,
            true,
        )
        .await
        {
            Ok(ids) => json!(ids.iter().map(|id| format!("<#{id}>")).collect::<Vec<_>>()),
            Err(err @ ToolError::Discord(_)) => json!(format!("unknown ({})", err.for_model())),
            Err(err) => return Err(err),
        };
        let host = context.guild().map_or_else(
            || context.config.app_name.clone(),
            |guild| guild.host_name.clone(),
        );

        Ok(ToolResult::json(&json!({
            "rules": rules.iter().map(model::summary).collect::<Vec<_>>(),
            "watched_channels": watched,
            "watched_channel_guidelines": if settings.watch_guidelines {
                json!(model::guidelines(&host).into_iter().map(|(_, text)| text).collect::<Vec<_>>())
            } else {
                json!("off")
            },
            "automatic_spam": if settings.spam_enabled {
                format!("on, in every channel, without AI: {}", spam_limits(context))
            } else {
                "off".to_string()
            },
            "warning_steps": {
                "timeout": if settings.warn_timeout_at > 0 {
                    format!("from {} warnings: timeout {}", settings.warn_timeout_at, settings.warn_timeout)
                } else {
                    "off".to_string()
                },
                "ban": if settings.warn_ban_at > 0 {
                    format!("from {} warnings: ban {}", settings.warn_ban_at, settings.warn_ban)
                } else {
                    "off".to_string()
                },
            },
        })))
    }
}

pub struct EditRule;

#[async_trait::async_trait]
impl Tool for EditRule {
    fn name(&self) -> &'static str {
        "edit_rule"
    }

    fn description(&self, _context: &Context) -> String {
        "Change a standing rule (list_rules shows the numbers): what breaks it, what does not, keywords, action, duration, message deletion, the reason members are told, or its channels. Only the fields given change. When a rule caught the wrong message or missed one, fix its when / not_when here instead of adding another rule.".into()
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "rule_id": { "type": "integer", "description": "The rule's number." },
                "when": { "type": "string", "description": "New description of what breaks the rule, as for add_rule." },
                "not_when": { "type": "string", "description": "The full new list of what must be left alone, as for add_rule (replaces the old one; list_rules shows it). \"none\" clears it." },
                "keywords": keywords_property("The full new keyword list (replaces the old one; list_rules shows it), as for add_rule."),
                "action": { "type": "string", "enum": ACTIONS, "description": "New action: warn, timeout, delete, kick or ban." },
                "duration": { "type": "string", "description": "For timeout or ban: how long, like \"1h\". \"permanent\" for a ban for good." },
                "delete_message": { "type": "boolean", "description": "Also delete the message, or not." },
                "reason": { "type": "string", "description": "What the member is told." },
                "channels": channels_property("Only in these channels (#name or id); an empty list means the whole server."),
            }),
            &["rule_id"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let rule = active_rule(context, arguments).await?;
        let given = |key: &str| arguments.get(key).is_some();
        let mut changed: Vec<&str> = Vec::new();
        let mut dropped = Vec::new();

        let mut description = rule.description.clone();
        if let Some(when) = string(arguments, "when") {
            description = text::cut(&when, 600, "");
        }

        let mut exceptions = rule.exceptions.clone();
        if given("not_when") {
            let not_when = arguments["not_when"].as_str().unwrap_or_default().trim();
            exceptions =
                (!is_off(not_when, &["", "none", "-"])).then(|| text::cut(not_when, 600, ""));
        }

        let mut keywords = rule.keywords.clone();
        if given("keywords") {
            let (kept, too_common) = model::keywords(&strings(arguments, "keywords"))?;
            keywords = json!(kept);
            dropped = too_common;
        }

        let new_action = action(arguments)?;
        let action = new_action.clone().unwrap_or_else(|| rule.action.clone());

        let mut duration_seconds = rule.duration_seconds;
        let duration = string(arguments, "duration");
        if duration.is_some() || new_action.is_some() {
            let seconds = match &duration {
                None => rule.duration_seconds.map(i64::from),
                Some(duration) if is_off(duration, &PERMANENT) => None,
                Some(duration) => Some(time::duration(duration).map_err(ToolError::Invalid)?),
            };
            duration_seconds = rule_seconds(&action, seconds).map(|seconds| seconds as i32);
        }

        let mut delete_message = rule.delete_message;
        if given("delete_message") || action == "delete" {
            delete_message = action == "delete" || boolean(arguments, "delete_message", false);
        }

        let mut reason = rule.reason.clone();
        if let Some(new_reason) = string(arguments, "reason") {
            reason = Some(text::cut(&new_reason, 300, ""));
        }

        let mut channel_ids = rule.channel_ids.clone();
        if given("channels") {
            channel_ids = channels(context, &strings(arguments, "channels")).await?;
        }

        for (field, differs) in [
            ("when", description != rule.description),
            ("not_when", exceptions != rule.exceptions),
            ("keywords", keywords != rule.keywords),
            ("action", action != rule.action),
            ("duration", duration_seconds != rule.duration_seconds),
            ("delete_message", delete_message != rule.delete_message),
            ("reason", reason != rule.reason),
            ("channels", channel_ids != rule.channel_ids),
        ] {
            if differs {
                changed.push(field);
            }
        }
        if changed.is_empty() {
            return Err(ToolError::invalid(
                "Nothing to change: say what the rule should do differently.",
            ));
        }

        let Some(updated) = sqlx::query_as::<_, Rule>(sqlx::AssertSqlSafe(format!(
            "UPDATE rux_rules SET description = $3, exceptions = $4, keywords = $5, action = $6,
                duration_seconds = $7, delete_message = $8, reason = $9, channel_ids = $10, updated = NOW()
            WHERE id = $1 AND guild_id = $2 AND active
            RETURNING {RULE_COLUMNS}"
        )))
        .bind(rule.id)
        .bind(&context.guild_id)
        .bind(&description)
        .bind(&exceptions)
        .bind(&keywords)
        .bind(&action)
        .bind(duration_seconds)
        .bind(delete_message)
        .bind(&reason)
        .bind(&channel_ids)
        .fetch_optional(context.state.database.write())
        .await?
        else {
            return Err(ToolError::invalid(
                "That rule was removed meanwhile. Use list_rules.",
            ));
        };

        service::note(
            context,
            "rule_edit",
            Some(updated.description.as_str()),
            meta(&[("rule", json!(updated.id)), ("changed", json!(changed))]),
        )
        .await?;

        Ok(ToolResult::json(&rule_result(
            ("changed", json!(changed)),
            &updated,
            &dropped,
        )))
    }
}

pub struct RemoveRule;

#[async_trait::async_trait]
impl Tool for RemoveRule {
    fn name(&self) -> &'static str {
        "remove_rule"
    }

    fn description(&self, _context: &Context) -> String {
        "Stop enforcing a standing rule (list_rules shows the numbers).".into()
    }

    fn parameters(&self) -> Value {
        schema(
            json!({ "rule_id": { "type": "integer", "description": "The rule's number." } }),
            &["rule_id"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let rule = active_rule(context, arguments).await?;
        let removed = sqlx::query(
            "UPDATE rux_rules SET active = false, updated = NOW() WHERE id = $1 AND guild_id = $2 AND active",
        )
        .bind(rule.id)
        .bind(&context.guild_id)
        .execute(context.state.database.write())
        .await?
        .rows_affected();
        if removed == 0 {
            return Err(ToolError::invalid(
                "There is no active rule with that number on this server. Use list_rules.",
            ));
        }

        service::note(
            context,
            "rule_remove",
            Some(rule.description.as_str()),
            meta(&[("rule", json!(rule.id))]),
        )
        .await?;

        Ok(ToolResult::json(&json!({
            "removed": true,
            "rule": rule.id,
            "was": rule.description,
        })))
    }
}

pub struct WatchChannel;

#[async_trait::async_trait]
impl Tool for WatchChannel {
    fn name(&self) -> &'static str {
        "watch_channel"
    }

    fn description(&self, _context: &Context) -> String {
        "Start or stop checking EVERY member message in a channel with AI (against the standing rules and the always-on guidelines: scams, slurs, harassment, sexual content, spam, advertising). Busy channels mean more AI calls (about one per 20 messages).".into()
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "channel": { "type": "string", "description": "Channel (#name, <#id> or id)." },
                "watch": { "type": "boolean", "description": "true starts watching it, false stops." },
            }),
            &["channel", "watch"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let (state, guild_id) = (&context.state, &context.guild_id);
        let channel = resolver::channel(
            &context.rest,
            guild_id,
            &required(arguments, "channel")?,
            None,
        )
        .await?;
        let id = channel["id"].as_str().unwrap_or_default().to_string();
        let watch = boolean(arguments, "watch", true);

        if watch {
            sqlx::query(
                "INSERT INTO rux_watches (guild_id, channel, added_by_discord_id) VALUES ($1, $2, $3)
                ON CONFLICT (guild_id, channel) DO NOTHING",
            )
            .bind(guild_id)
            .bind(&id)
            .bind(&context.actor_discord_id)
            .execute(state.database.write())
            .await?;
        } else {
            let ids = watches::watched_channel_ids(state, &context.rest, guild_id, true).await?;
            if !ids.contains(&id) {
                return Err(ToolError::Invalid(format!("<#{id}> is not watched.")));
            }

            // watched by its id, or by a name that is this channel's (on this server)
            let name = resolver::plain(channel["name"].as_str().unwrap_or_default());
            let rows: Vec<(i64, String)> =
                sqlx::query_as("SELECT id, channel FROM rux_watches WHERE guild_id = $1")
                    .bind(guild_id)
                    .fetch_all(state.database.read())
                    .await?;
            let gone: Vec<i64> = rows
                .into_iter()
                .filter(|(_, channel)| {
                    *channel == id
                        || (!name.is_empty()
                            && resolver::plain(channel.trim_start_matches('#')) == name)
                })
                .map(|(row, _)| row)
                .collect();
            sqlx::query("DELETE FROM rux_watches WHERE guild_id = $1 AND id = ANY($2)")
                .bind(guild_id)
                .bind(&gone)
                .execute(state.database.write())
                .await?;
        }
        watches::forget(state, guild_id).await;

        service::note(
            context,
            if watch { "watch_add" } else { "watch_remove" },
            None,
            meta(&[("channel", json!(id))]),
        )
        .await?;

        let all = watches::watched_channel_ids(state, &context.rest, guild_id, true).await?;

        Ok(ToolResult::json(&json!({
            "channel": format!("<#{id}>"),
            "watched": watch,
            "all_watched": all.iter().map(|id| format!("<#{id}>")).collect::<Vec<_>>(),
        })))
    }
}

pub struct SetModeratorRole;

#[async_trait::async_trait]
impl Tool for SetModeratorRole {
    fn name(&self) -> &'static str {
        "set_moderator_role"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Give a role mod permissions, or take them away. Its members can then give {} moderation orders (warn, timeout, kick, ban, purge, lock, look up members; not giveaways, rules or roles, and never against staff). By default the role also gets Discord's own Timeout, Kick, Ban and Manage Messages permissions.",
            context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "role": { "type": "string", "description": "The role: <@&mention>, name or id." },
                "moderator": { "type": "boolean", "description": "true gives mod permissions, false takes them away." },
                "discord_permissions": { "type": "boolean", "description": "Also give (or take back) Discord's own moderation permissions on the role. Default true." },
            }),
            &["role", "moderator"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let role = resolver::role(
            &context.rest,
            &context.guild_id,
            &required(arguments, "role")?,
        )
        .await?;
        let role_id = role["id"].as_str().unwrap_or_default().to_string();
        let role_name = role["name"].as_str().unwrap_or_default().to_string();
        let discord_permissions = boolean(arguments, "discord_permissions", true);
        let role_meta = || meta(&[("role", json!(role_id)), ("role_name", json!(role_name))]);

        if boolean(arguments, "moderator", true) {
            let (_, added) = access::grant(context, &role, discord_permissions).await?;
            service::note(context, "mod_role_add", None, role_meta()).await?;

            return Ok(ToolResult::json(&json!({
                "role": format!("<@&{role_id}>"),
                "rux_moderators": true,
                "discord_permissions_added": if added.is_empty() {
                    json!("none (it already had them, or not asked)")
                } else {
                    json!(added)
                },
            })));
        }

        let removed = access::revoke(context, &role, discord_permissions).await?;
        service::note(context, "mod_role_remove", None, role_meta()).await?;

        Ok(ToolResult::json(&json!({
            "role": format!("<@&{role_id}>"),
            "rux_moderators": false,
            "discord_permissions_removed": if removed.is_empty() { json!("none") } else { json!(removed) },
        })))
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(AddRule),
        Box::new(ListRules),
        Box::new(EditRule),
        Box::new(RemoveRule),
        Box::new(WatchChannel),
        Box::new(SetModeratorRole),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_durations_are_clamped() {
        assert_eq!(rule_seconds("timeout", None), Some(3_600));
        assert_eq!(rule_seconds("timeout", Some(5)), Some(60));
        assert_eq!(
            rule_seconds("timeout", Some(i64::MAX)),
            Some(MAX_TIMEOUT_SECONDS)
        );
        assert_eq!(rule_seconds("ban", None), None);
        assert_eq!(rule_seconds("ban", Some(0)), None);
        assert_eq!(rule_seconds("ban", Some(86_400)), Some(86_400));
        assert!(rule_seconds("ban", Some(i64::MAX)).unwrap() <= i64::from(i32::MAX));
        assert_eq!(rule_seconds("warn", Some(86_400)), None);
    }

    #[test]
    fn only_listing_is_for_moderators() {
        for tool in tools() {
            let expected = if tool.name() == "list_rules" {
                Level::Moderator
            } else {
                Level::Admin
            };
            assert_eq!(tool.level(), expected, "{}", tool.name());
        }
    }

    #[test]
    fn results_lead_with_what_happened() {
        let rule = super::super::model::tests::rule();
        let out = rule_result(("created", json!(true)), &rule, &["new".into()]);
        let keys: Vec<&String> = out.as_object().unwrap().keys().collect();
        assert_eq!(keys.first().map(|key| key.as_str()), Some("created"));
        assert_eq!(
            keys.last().map(|key| key.as_str()),
            Some("keywords_dropped_as_too_common")
        );
    }
}
