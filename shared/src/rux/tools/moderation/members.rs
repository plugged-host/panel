//! plugged.host: the moderation tools about members: looking them up, warnings, timeouts, kicks,
//! bans (and the ban list), nicknames and the case log. Moderators get all of them, never against
//! staff ([`common::guard`]).

use super::{
    super::{Tool, ToolError, ToolResult, boolean, int, required, schema, string},
    common::{self, MEMBER, Target, case_result, member_property, reason_property, without_nulls},
    service::{self, BanOptions},
};
use crate::rux::{
    access::{self, Level},
    context::Context,
    models::{CASE_COLUMNS, Case},
    text, time,
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Words that lift a timeout.
const TIMEOUT_OFF: [&str; 4] = ["0", "off", "none", "remove"];
/// Words that make a ban one for good.
const PERMANENT: [&str; 3] = ["permanent", "forever", "0"];
/// Words that remove a nickname.
const NICKNAME_OFF: [&str; 5] = ["reset", "remove", "none", "clear", "off"];
/// Bans `list_bans` shows.
const BANS_SHOWN: usize = 40;

/// The panel account linked to a Discord account, by the panel's own records.
async fn panel_user(state: &crate::State, discord_id: &str) -> Option<uuid::Uuid> {
    let context = crate::discord::Context::load(state).await.ok()?;

    crate::discord::user_for_discord_id(state, &context, discord_id)
        .await
        .ok()
        .flatten()
}

fn utc(text: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    text.and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|time| time.with_timezone(&chrono::Utc))
}

pub struct LookupMember;

#[async_trait::async_trait]
impl Tool for LookupMember {
    fn name(&self) -> &'static str {
        "lookup_member"
    }

    fn description(&self, context: &Context) -> String {
        let config = &context.config;
        let accounts = if config.portal != config.app_name {
            format!("{} and {} accounts", config.portal, config.app_name)
        } else {
            format!("{} account", config.app_name)
        };

        format!(
            "Who a member is: Discord profile, roles, join date, timeout, their {accounts}, giveaway wins and past moderation cases. Also finds members by name."
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(json!({ "member": member_property(MEMBER) }), &["member"])
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        let state = &context.state;
        let member = target.member.clone().unwrap_or_default();
        let member_roles = common::role_ids(&member);
        let zone = context.config.tz();
        let roles = context.rest.roles(&context.guild_id, false).await?;
        let role_names: Vec<String> = member_roles
            .iter()
            .map(|id| {
                roles
                    .iter()
                    .find(|role| role["id"].as_str() == Some(id.as_str()))
                    .and_then(|role| role["name"].as_str())
                    .unwrap_or(id)
                    .to_string()
            })
            .collect();

        let timed_out_until = utc(member["communication_disabled_until"].as_str())
            .filter(|until| *until > chrono::Utc::now())
            .map(|until| time::human(until, zone));

        let actor = access::actor(state, &target.id).await;
        let account = match &actor {
            Ok(Some(actor)) => without_nulls(json!({
                "account_id": actor.account_id,
                "name": text::name(&actor.name, 64),
                "admin": actor.is_admin,
            })),
            Ok(None) => json!("not linked (cannot enter giveaways)"),
            Err(_) => json!("unknown (the account directory did not answer)"),
        };
        let user_uuid = match &actor {
            Ok(Some(actor)) => actor.user_uuid,
            Ok(None) => None,
            Err(_) => panel_user(state, &target.id).await,
        };

        let mut panel_account = Value::Null;
        if let Some(user_uuid) = user_uuid {
            let credits_enabled = state.settings.get().await?.economy.credits_enabled;
            let row: Option<(String, i64, i64)> = sqlx::query_as(
                "SELECT u.username, u.credits, (SELECT COUNT(*) FROM servers s WHERE s.owner_uuid = u.uuid)
                FROM users u WHERE u.uuid = $1",
            )
            .bind(user_uuid)
            .fetch_optional(state.database.read())
            .await?;
            if let Some((username, credits, servers)) = row {
                panel_account = without_nulls(json!({
                    "username": text::name(&username, 64),
                    "servers": servers,
                    "credits": credits_enabled.then_some(credits),
                }));
            }
        }

        let warnings = Case::warning_count(&state.database, &context.guild_id, &target.id).await?;
        let moderator = access::moderator_role(state, &context.guild_id, &member_roles)
            .await?
            .is_some();
        let wins: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rux_giveaway_entries e
            JOIN rux_giveaways g ON g.id = e.giveaway_id
            WHERE e.discord_id = $1 AND e.won IS NOT NULL AND g.guild_id = $2",
        )
        .bind(&target.id)
        .bind(&context.guild_id)
        .fetch_one(state.database.read())
        .await?;
        let cases: Vec<String> = Case::history(&state.database, &context.guild_id, &target.id, 8)
            .await?
            .iter()
            .map(|case| case.line(false, zone))
            .collect();

        Ok(ToolResult::json(&without_nulls(json!({
            "member": target.mention(),
            "id": target.id,
            "username": member["user"]["username"],
            "display_name": text::name(&target.name, 64),
            "bot": (member["user"]["bot"].as_bool() == Some(true)).then_some(true),
            "account_created": crate::discord::created_at(&target.id).map(|created| time::human(created, zone)),
            "joined_server": utc(member["joined_at"].as_str()).map(|joined| time::human(joined, zone)),
            "roles": role_names,
            "timed_out_until": timed_out_until,
            "account": account,
            "panel_account": panel_account,
            "warnings": warnings,
            "rux_moderator": moderator.then_some(true),
            "giveaway_wins": wins,
            "cases": cases,
        }))))
    }
}

pub struct WarnMember;

#[async_trait::async_trait]
impl Tool for WarnMember {
    fn name(&self) -> &'static str {
        "warn_member"
    }

    fn description(&self, context: &Context) -> String {
        let settings = &context.config.settings;
        let mut steps = Vec::new();
        if settings.warn_timeout_at > 0 {
            steps.push(format!(
                "{} warnings = {} timeout",
                settings.warn_timeout_at, settings.warn_timeout
            ));
        }
        if settings.warn_ban_at > 0 {
            steps.push(format!(
                "{} = {} ban",
                settings.warn_ban_at, settings.warn_ban
            ));
        }

        if steps.is_empty() {
            "Warn a member: they get a DM with the reason, and it shows in their cases.".into()
        } else {
            format!(
                "Warn a member: they get a DM with the reason and it counts towards automatic steps ({}).",
                steps.join(", ")
            )
        }
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({ "member": member_property(MEMBER), "reason": reason_property() }),
            &["member", "reason"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        common::guard(context, &target).await?;
        let reason = string(arguments, "reason");
        let warned = service::warn(context, &target, reason.as_deref(), None, false).await?;
        let meta = warned.case.meta.clone().unwrap_or_default();

        Ok(ToolResult::json(&case_result(
            &warned.case,
            json!({
                "warnings_total": warned.warnings,
                "dm_delivered": meta["dm_delivered"].as_bool() == Some(true),
                "automatic_step": warned.escalation.as_ref().map(|step| match step.duration_seconds {
                    Some(seconds) if seconds > 0 => format!("{} for {}", step.action, time::human_duration(i64::from(seconds))),
                    _ => step.action.clone(),
                }),
                "automatic_step_failed": meta["step_failed"],
            }),
        )))
    }
}

pub struct TimeoutMember;

#[async_trait::async_trait]
impl Tool for TimeoutMember {
    fn name(&self) -> &'static str {
        "timeout_member"
    }

    fn description(&self, _context: &Context) -> String {
        "Time a member out (they cannot talk) for a while, at most 28 days, or lift a timeout with duration \"0\".".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property(MEMBER),
                "duration": { "type": "string", "description": "How long: \"10m\", \"2h\", \"7d\". \"0\" lifts the timeout." },
                "reason": reason_property(),
            }),
            &["member", "duration"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        common::guard(context, &target).await?;
        let duration = required(arguments, "duration")?;
        let reason = string(arguments, "reason");

        if common::is_off(&duration, &TIMEOUT_OFF) {
            let case = service::untimeout(context, &target, reason.as_deref()).await?;
            return Ok(ToolResult::json(&case_result(&case, json!({}))));
        }

        let seconds = time::duration(&duration).map_err(ToolError::Invalid)?;
        let case =
            service::timeout(context, &target, seconds, reason.as_deref(), false, None).await?;
        let until = case
            .meta
            .as_ref()
            .and_then(|meta| utc(meta["until"].as_str()))
            .map(|until| time::human(until, context.config.tz()));

        Ok(ToolResult::json(&case_result(
            &case,
            json!({
                "until": until,
                "dm_delivered": case.meta.as_ref().is_some_and(|meta| meta["dm_delivered"].as_bool() == Some(true)),
            }),
        )))
    }
}

pub struct KickMember;

#[async_trait::async_trait]
impl Tool for KickMember {
    fn name(&self) -> &'static str {
        "kick_member"
    }

    fn description(&self, _context: &Context) -> String {
        "Kick a member out of the server (they can rejoin with an invite). They get a DM with the reason first.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({ "member": member_property(MEMBER), "reason": reason_property() }),
            &["member"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        common::guard(context, &target).await?;
        let case = service::kick(
            context,
            &target,
            string(arguments, "reason").as_deref(),
            None,
        )
        .await?;

        Ok(ToolResult::json(&case_result(&case, json!({}))))
    }
}

pub struct BanMember;

#[async_trait::async_trait]
impl Tool for BanMember {
    fn name(&self) -> &'static str {
        "ban_member"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Ban someone, for good or for a while ({} lifts it when the time is up), optionally deleting their recent messages. Works for people who already left (by id or mention).",
            context.config.settings.name
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property("The person: <@mention>, user id, or a member's name."),
                "reason": reason_property(),
                "duration": { "type": "string", "description": "How long, like \"24h\" or \"7d\". Leave out for a permanent ban." },
                "delete_message_hours": { "type": "integer", "description": "Delete their messages from the last N hours (0 to 168). Default 0." },
            }),
            &["member"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let input = required(arguments, "member")?;
        let target = common::member_or_user(context, &input).await?;
        let in_server = target.member.is_some();
        // also for people who are not in the server: an admin or staff member who left must not
        // be bannable by id
        common::guard(context, &target).await?;

        let seconds = match string(arguments, "duration") {
            Some(duration) if !common::is_off(&duration, &PERMANENT) => {
                Some(time::duration(&duration).map_err(ToolError::Invalid)?)
            }
            _ => None,
        };
        let case = service::ban(
            context,
            &target,
            string(arguments, "reason").as_deref(),
            BanOptions {
                delete_message_hours: int(arguments, "delete_message_hours").unwrap_or(0),
                in_server,
                seconds,
                ..Default::default()
            },
        )
        .await?;

        Ok(ToolResult::json(&case_result(
            &case,
            json!({
                "was_in_server": in_server,
                "until": case.expires.map_or_else(
                    || "permanent".to_string(),
                    |expires| time::human(expires.and_utc(), context.config.tz()),
                ),
            }),
        )))
    }
}

pub struct UnbanUser;

#[async_trait::async_trait]
impl Tool for UnbanUser {
    fn name(&self) -> &'static str {
        "unban_user"
    }

    fn description(&self, _context: &Context) -> String {
        "Lift a ban. Needs the user id or <@mention> (banned people are not members, so names do not work).".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "user": { "type": "string", "description": "User id or <@mention>." },
                "reason": reason_property(),
            }),
            &["user"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target: Target =
            crate::rux::resolver::user(&context.rest, &required(arguments, "user")?)
                .await?
                .into();
        let case = service::unban(context, &target, string(arguments, "reason").as_deref()).await?;

        Ok(ToolResult::json(&case_result(&case, json!({}))))
    }
}

pub struct ListCases;

#[async_trait::async_trait]
impl Tool for ListCases {
    fn name(&self) -> &'static str {
        "list_cases"
    }

    fn description(&self, context: &Context) -> String {
        let name = &context.config.settings.name;

        format!(
            "{name}'s moderation log, newest first: warnings, timeouts, kicks, bans, deleted messages, rule changes, by {name} itself or on someone's order. Automatic cases show what the member said. Filter by member, action or time."
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property("Only cases about this person, in the server or not (optional)."),
                "action": { "type": "string", "description": "Only this action: warn, timeout, untimeout, kick, ban, unban, delete, purge, nickname, clear_warnings, lock, slowmode, role_add, rule_add, rule_edit… (optional)." },
                "automatic": { "type": "boolean", "description": "true: only what rux did by itself (rules, watched channels, warning steps); false: only ordered actions." },
                "since": { "type": "string", "description": "How far back, like \"24h\" or \"7d\" (optional)." },
                "count": { "type": "integer", "description": "How many, 1 to 40. Default 15." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let member = match string(arguments, "member") {
            Some(input) => Some(common::member_or_user(context, &input).await?.id),
            None => None,
        };
        let action = string(arguments, "action").map(|action| action.to_lowercase());
        let automatic = match &arguments["automatic"] {
            Value::Null => None,
            _ => Some(boolean(arguments, "automatic", false)),
        };
        let since = match string(arguments, "since") {
            Some(since) => Some(time::duration(&since).map_err(ToolError::Invalid)? as f64),
            None => None,
        };
        let count = int(arguments, "count").unwrap_or(15).clamp(1, 40);

        const FILTER: &str = "guild_id = $1
            AND ($2::text IS NULL OR target_id = $2)
            AND ($3::text IS NULL OR action = $3)
            AND ($4::boolean IS NULL OR automatic = $4)
            AND ($5::float8 IS NULL OR created >= NOW() - make_interval(secs => $5::float8))";
        let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM rux_cases WHERE {FILTER}"
        )))
        .bind(&context.guild_id)
        .bind(&member)
        .bind(&action)
        .bind(automatic)
        .bind(since)
        .fetch_one(context.state.database.read())
        .await?;
        let rows = sqlx::query_as::<_, Case>(sqlx::AssertSqlSafe(format!(
            "SELECT {CASE_COLUMNS} FROM rux_cases WHERE {FILTER} ORDER BY id DESC LIMIT $6"
        )))
        .bind(&context.guild_id)
        .bind(&member)
        .bind(&action)
        .bind(automatic)
        .bind(since)
        .bind(count)
        .fetch_all(context.state.database.read())
        .await?;
        let zone = context.config.tz();

        Ok(ToolResult::json(&json!({
            "matching": total,
            "cases": rows.iter().map(|case| case.line(true, zone)).collect::<Vec<_>>(),
        })))
    }
}

pub struct ListBans;

#[async_trait::async_trait]
impl Tool for ListBans {
    fn name(&self) -> &'static str {
        "list_bans"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Who is banned from the server, with the reason and, for {}'s bans, the case and when the ban ends. Finds a banned person by name or id (they are not members, so lookup_member cannot).",
            context.config.settings.name
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "search": { "type": "string", "description": "Only bans whose username, display name or id contains this (optional)." },
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let mut bans = context.rest.bans(&context.guild_id, 5000).await?;
        if let Some(search) = string(arguments, "search") {
            let needle = search
                .trim_matches(|c: char| c.is_whitespace() || matches!(c, '<' | '@' | '!' | '>'))
                .to_lowercase();
            bans.retain(|ban| {
                [
                    &ban["user"]["id"],
                    &ban["user"]["username"],
                    &ban["user"]["global_name"],
                ]
                .into_iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
                .contains(&needle)
            });
        }

        let ids: Vec<String> = bans
            .iter()
            .filter_map(|ban| ban["user"]["id"].as_str().map(str::to_string))
            .collect();
        let open: HashMap<String, Case> = sqlx::query_as::<_, Case>(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT ON (target_id) {CASE_COLUMNS} FROM rux_cases
            WHERE guild_id = $1 AND action = 'ban' AND resolved IS NULL AND target_id = ANY($2)
            ORDER BY target_id, id DESC"
        )))
        .bind(&context.guild_id)
        .bind(&ids)
        .fetch_all(context.state.database.read())
        .await?
        .into_iter()
        .filter_map(|case| Some((case.target_id.clone()?, case)))
        .collect();

        // the bot's own bans first, newest first; Discord lists the rest by user id
        let case_of = |ban: &Value| {
            ban["user"]["id"]
                .as_str()
                .and_then(|id| open.get(id))
                .map_or(0, |case| case.id)
        };
        bans.sort_by_key(|ban| std::cmp::Reverse(case_of(ban)));

        let zone = context.config.tz();
        let lines: Vec<String> = bans
            .iter()
            .take(BANS_SHOWN)
            .map(|ban| {
                let user = &ban["user"];
                let id = user["id"].as_str().unwrap_or_default();
                let name = user["global_name"]
                    .as_str()
                    .or(user["username"].as_str())
                    .unwrap_or("?");
                let reason = ban["reason"]
                    .as_str()
                    .filter(|reason| !reason.is_empty())
                    .map_or_else(
                        || "no reason given".to_string(),
                        |reason| text::untrusted(reason, 150),
                    );
                let case = open.get(id).map(|case| {
                    format!(
                        " ({} case #{}, {})",
                        context.config.settings.name,
                        case.id,
                        case.expires.map_or_else(
                            || "permanent".to_string(),
                            |expires| format!("ends {}", time::human(expires.and_utc(), zone)),
                        )
                    )
                });

                format!(
                    "<@{id}> ({}): {reason}{}",
                    text::untrusted(name, 40),
                    case.unwrap_or_default()
                )
            })
            .collect();

        Ok(ToolResult::json(&without_nulls(json!({
            "banned": bans.len(),
            "bans": lines,
            "note": (bans.len() > BANS_SHOWN).then(|| format!("Only the first {BANS_SHOWN} are shown: search by name or id.")),
        }))))
    }
}

pub struct SetNickname;

#[async_trait::async_trait]
impl Tool for SetNickname {
    fn name(&self) -> &'static str {
        "set_nickname"
    }

    fn description(&self, _context: &Context) -> String {
        "Change a member's nickname on the server (e.g. an offensive or impersonating name), or remove it so their own name shows.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property(MEMBER),
                "nickname": { "type": "string", "description": "The new nickname, up to 32 characters. Empty or \"reset\" removes it." },
                "reason": reason_property(),
            }),
            &["member", "nickname"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        common::guard(context, &target).await?;

        let nickname = string(arguments, "nickname")
            .filter(|nickname| !common::is_off(nickname, &NICKNAME_OFF));
        if nickname
            .as_ref()
            .is_some_and(|nickname| nickname.chars().count() > 32)
        {
            return Err(ToolError::invalid("A nickname is at most 32 characters."));
        }

        let case = service::nickname(
            context,
            &target,
            nickname.as_deref(),
            string(arguments, "reason").as_deref(),
        )
        .await?;

        Ok(ToolResult::json(&case_result(
            &case,
            json!({ "nickname": nickname.unwrap_or_else(|| "removed, their own name shows".into()) }),
        )))
    }
}

pub struct ClearWarnings;

#[async_trait::async_trait]
impl Tool for ClearWarnings {
    fn name(&self) -> &'static str {
        "clear_warnings"
    }

    fn description(&self, _context: &Context) -> String {
        "Take back a member's warnings (the latest N, or all), so they stop counting towards the automatic timeout and ban.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property(MEMBER),
                "count": { "type": "integer", "description": "How many of the latest warnings. Leave out for all." },
                "reason": reason_property(),
            }),
            &["member"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member_or_user(context, &required(arguments, "member")?).await?;
        common::guard(context, &target).await?;
        let (cleared, left) = service::clear_warnings(
            context,
            &target,
            int(arguments, "count"),
            string(arguments, "reason").as_deref(),
        )
        .await?;

        Ok(ToolResult::json(&json!({
            "member": target.mention(),
            "cleared": cleared,
            "warnings_left": left,
        })))
    }
}
