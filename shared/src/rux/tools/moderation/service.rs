//! plugged.host: the moderation rux does on an order (or by itself, on a standing rule or a
//! warning step). Every action is a case ([`super::cases`]), goes to the server's mod log and
//! carries the order in Discord's audit log. The member is told by DM before a kick or ban, while
//! rux can still reach them. Warnings, bans and the case history are per server: a warning on one
//! server does not count on another.

use super::{
    super::ToolError,
    cases::{self, NewCase},
    common::{self, Target},
};
use crate::rux::{access, context::Context, models::Case, say, text, time};
use chrono::{TimeDelta, Utc};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Discord's longest timeout: 28 days.
pub const MAX_TIMEOUT_SECONDS: i64 = 2_419_200;
/// The longest ban for a while (ten years); longer is a ban for good.
pub const MAX_BAN_SECONDS: i64 = 10 * 365 * 86_400;
/// The longest slowmode Discord takes: 6 hours.
pub const MAX_SLOWMODE_SECONDS: i64 = 21_600;
/// Messages a purge looks at, at most.
const PURGE_SCAN: usize = 500;

const SEND_MESSAGES: u64 = 1 << 11;
const SEND_IN_THREADS: u64 = 1 << 38;
const CREATE_THREADS: u64 = (1 << 35) | (1 << 36);

/// A warning, and the step of the warning ladder it reached.
pub struct Warned {
    pub case: Case,
    pub warnings: i64,
    pub escalation: Option<Case>,
}

/// How a ban goes.
#[derive(Debug, Clone, Copy, Default)]
pub struct BanOptions {
    /// Delete their messages of the last N hours (0 to 168).
    pub delete_message_hours: i64,
    /// They are in the server (so they get a DM first).
    pub in_server: bool,
    /// For a while; `None`: for good.
    pub seconds: Option<i64>,
    pub automatic: bool,
    pub rule_id: Option<i64>,
}

/// Discord's oldest message a bulk delete takes (14 days, minus a margin).
fn bulk_cutoff() -> chrono::DateTime<Utc> {
    Utc::now() - TimeDelta::days(14) + TimeDelta::minutes(5)
}

fn reason_of(reason: Option<&str>) -> Option<&str> {
    reason.map(str::trim).filter(|reason| !reason.is_empty())
}

/// Tells someone by DM what happened to them on this server; `false` when they do not take DMs.
async fn notify(
    context: &Context,
    user_id: &str,
    key: &str,
    reason: Option<&str>,
    replace: &[(&str, &str)],
) -> bool {
    let reason_line = reason_of(reason)
        .map(|reason| context.say("notice.reason", &[("reason", reason)]))
        .unwrap_or_default();
    let server = access::server_name(&context.rest, &context.config, &context.guild_id).await;
    let mut all = replace.to_vec();
    all.push(("server", &server));
    all.push(("reason", &reason_line));

    context
        .rest
        .send_dm(
            user_id,
            &json!({
                "content": context.say(key, &all).trim(),
                "allowed_mentions": { "parse": [] },
            }),
        )
        .await
}

/// A warning, then the step it reaches on the warning ladder (the settings: with the defaults
/// the 3rd and 4th warning also time the member out for 24 hours, the 5th and every later one
/// bans them for 24 hours). A step Discord refuses leaves the warning, with why in its case.
pub async fn warn(
    context: &Context,
    target: &Target,
    reason: Option<&str>,
    rule_id: Option<i64>,
    automatic: bool,
) -> Result<Warned, ToolError> {
    let told = notify(context, &target.id, "notice.warn", reason, &[]).await;
    let mut case = cases::record(
        context,
        NewCase::new("warn")
            .on(Some(target))
            .reason(reason)
            .automatic(automatic, rule_id)
            .meta("dm_delivered", told),
    )
    .await?;
    let warnings =
        Case::warning_count(&context.state.database, &context.guild_id, &target.id).await?;

    let settings = &context.config.settings;
    let why = say::say_count(
        context.config.locale(),
        "notice.warnings_reached",
        warnings,
        &[],
    );
    let step = if settings.warn_ban_at > 0 && warnings >= i64::from(settings.warn_ban_at) {
        let seconds = time::duration_seconds(&settings.warn_ban).filter(|seconds| *seconds > 0);
        ban(
            context,
            target,
            Some(&why),
            BanOptions {
                in_server: true,
                seconds,
                automatic: true,
                ..Default::default()
            },
        )
        .await
        .map(Some)
    } else if settings.warn_timeout_at > 0 && warnings >= i64::from(settings.warn_timeout_at) {
        let seconds = time::duration_seconds(&settings.warn_timeout)
            .unwrap_or(86_400)
            .clamp(1, MAX_TIMEOUT_SECONDS);
        timeout(context, target, seconds, Some(&why), true, None)
            .await
            .map(Some)
    } else {
        Ok(None)
    };

    let escalation = match step {
        Ok(step) => step,
        Err(err @ (ToolError::Discord(_) | ToolError::Invalid(_))) => {
            let failed = err.for_model();
            tracing::warn!(
                case = case.id,
                warnings,
                "rux: warning step failed: {failed}"
            );
            let mut meta = match case.meta.take() {
                Some(Value::Object(meta)) => meta,
                _ => Map::new(),
            };
            meta.insert("step_failed".into(), json!(text::cut(&failed, 300, "")));
            let meta = Value::Object(meta);
            sqlx::query("UPDATE rux_cases SET meta = $2, updated = NOW() WHERE id = $1")
                .bind(case.id)
                .bind(&meta)
                .execute(context.state.database.write())
                .await?;
            case.meta = Some(meta);
            None
        }
        Err(err) => return Err(err),
    };

    Ok(Warned {
        case,
        warnings,
        escalation,
    })
}

/// Takes back someone's latest warnings (all of them when `count` is `None`); answers how many,
/// and how many are left.
pub async fn clear_warnings(
    context: &Context,
    target: &Target,
    count: Option<i64>,
    reason: Option<&str>,
) -> Result<(i64, i64), ToolError> {
    let cleared: Vec<i64> = sqlx::query_scalar(
        "UPDATE rux_cases SET revoked = NOW(), updated = NOW()
        WHERE id IN (
            SELECT id FROM rux_cases
            WHERE guild_id = $1 AND target_id = $2 AND action = 'warn' AND revoked IS NULL
            ORDER BY id DESC LIMIT $3
        )
        RETURNING id",
    )
    .bind(&context.guild_id)
    .bind(&target.id)
    .bind(count.map(|count| count.max(1)))
    .fetch_all(context.state.database.write())
    .await?;
    let left = Case::warning_count(&context.state.database, &context.guild_id, &target.id).await?;

    if !cleared.is_empty() {
        cases::record(
            context,
            NewCase::new("clear_warnings")
                .on(Some(target))
                .reason(reason)
                .meta("cleared", cleared.len())
                .meta("left", left),
        )
        .await?;
    }

    Ok((cleared.len() as i64, left))
}

pub async fn timeout(
    context: &Context,
    target: &Target,
    seconds: i64,
    reason: Option<&str>,
    automatic: bool,
    rule_id: Option<i64>,
) -> Result<Case, ToolError> {
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&seconds) {
        return Err(ToolError::invalid(
            "A timeout lasts from 1 second to 28 days.",
        ));
    }

    let until = Utc::now() + TimeDelta::seconds(seconds);
    let until_text = until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    context
        .rest
        .modify_member(
            &context.guild_id,
            &target.id,
            &json!({ "communication_disabled_until": until_text }),
            Some(&context.audit_reason(reason)),
        )
        .await?;
    let told = notify(
        context,
        &target.id,
        "notice.timeout",
        reason,
        &[("until", &time::discord(until, 'f'))],
    )
    .await;

    cases::record(
        context,
        NewCase::new("timeout")
            .on(Some(target))
            .reason(reason)
            .seconds(Some(seconds))
            .automatic(automatic, rule_id)
            .meta("until", until_text)
            .meta("dm_delivered", told),
    )
    .await
}

pub async fn untimeout(
    context: &Context,
    target: &Target,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    context
        .rest
        .modify_member(
            &context.guild_id,
            &target.id,
            &json!({ "communication_disabled_until": null }),
            Some(&context.audit_reason(reason)),
        )
        .await?;

    cases::record(
        context,
        NewCase::new("untimeout").on(Some(target)).reason(reason),
    )
    .await
}

pub async fn kick(
    context: &Context,
    target: &Target,
    reason: Option<&str>,
    rule_id: Option<i64>,
) -> Result<Case, ToolError> {
    let told = notify(context, &target.id, "notice.kick", reason, &[]).await;
    context
        .rest
        .kick(
            &context.guild_id,
            &target.id,
            Some(&context.audit_reason(reason)),
        )
        .await?;

    cases::record(
        context,
        NewCase::new("kick")
            .on(Some(target))
            .reason(reason)
            .automatic(false, rule_id)
            .meta("dm_delivered", told),
    )
    .await
}

/// A ban, for good or for a while (rux lifts it then, [`cases::lift_expired_bans`]). A new ban
/// replaces a running one, so an earlier ban for a while cannot lift this one.
pub async fn ban(
    context: &Context,
    target: &Target,
    reason: Option<&str>,
    options: BanOptions,
) -> Result<Case, ToolError> {
    let delete_seconds = options.delete_message_hours.clamp(0, 168) * 3600;
    let seconds = options
        .seconds
        .map(|seconds| seconds.clamp(60, MAX_BAN_SECONDS));
    let until = seconds.map(|seconds| Utc::now() + TimeDelta::seconds(seconds));

    let told = if options.in_server {
        match until {
            Some(until) => {
                notify(
                    context,
                    &target.id,
                    "notice.ban_for",
                    reason,
                    &[("until", &time::discord(until, 'f'))],
                )
                .await
            }
            None => notify(context, &target.id, "notice.ban", reason, &[]).await,
        }
    } else {
        false
    };
    context
        .rest
        .ban(
            &context.guild_id,
            &target.id,
            delete_seconds as u32,
            Some(&context.audit_reason(reason)),
        )
        .await?;
    resolve_open_bans(context, &target.id).await?;

    let mut case = NewCase::new("ban")
        .on(Some(target))
        .reason(reason)
        .seconds(seconds)
        .automatic(options.automatic, options.rule_id)
        .meta("deleted_message_hours", delete_seconds / 3600)
        .meta("dm_delivered", told);
    case.expires = until.map(|until| until.naive_utc());

    cases::record(context, case).await
}

pub async fn unban(
    context: &Context,
    target: &Target,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    context
        .rest
        .unban(
            &context.guild_id,
            &target.id,
            Some(&context.audit_reason(reason)),
        )
        .await?;
    resolve_open_bans(context, &target.id).await?;

    cases::record(
        context,
        NewCase::new("unban").on(Some(target)).reason(reason),
    )
    .await
}

async fn resolve_open_bans(context: &Context, discord_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE rux_cases SET resolved = NOW(), updated = NOW()
        WHERE guild_id = $1 AND action = 'ban' AND target_id = $2 AND resolved IS NULL",
    )
    .bind(&context.guild_id)
    .bind(discord_id)
    .execute(context.state.database.write())
    .await?;

    Ok(())
}

/// Deletes the last `count` messages of a channel (only those of `from` when given), skipping
/// the order itself and pinned messages; with `keep_protected`, also those of people a
/// moderator must leave alone. Discord bulk deletes only the last 14 days, so the search stops
/// there; at most 500 messages are looked at. Answers how many were deleted, and the case.
pub async fn purge(
    context: &Context,
    channel: &Value,
    count: i64,
    from: Option<&Target>,
    reason: Option<&str>,
    keep_protected: bool,
) -> Result<(usize, Case), ToolError> {
    let channel_id = channel["id"].as_str().unwrap_or_default().to_string();
    let count = count.clamp(1, 200) as usize;
    let oldest = bulk_cutoff();
    let mut picked: Vec<String> = Vec::new();
    let mut protected: HashMap<String, bool> = HashMap::new();
    let mut before: Option<String> = None;
    let mut scanned = 0;

    'pages: while picked.len() < count && scanned < PURGE_SCAN {
        let mut query = vec![("limit", "100".to_string())];
        if let Some(before) = &before {
            query.push(("before", before.clone()));
        }
        let page = context.rest.messages(&channel_id, &query).await?;
        if page.is_empty() {
            break;
        }

        for message in &page {
            scanned += 1;
            let id = message["id"].as_str().unwrap_or_default().to_string();
            before = Some(id.clone());

            if common::sent_at(message).is_none_or(|sent| sent < oldest) {
                break 'pages;
            }
            if context.source_message_id.as_deref() == Some(id.as_str())
                || message["pinned"].as_bool() == Some(true)
            {
                continue;
            }
            let author = message["author"]["id"].as_str().unwrap_or_default();
            if from.is_some_and(|from| from.id != author) {
                continue;
            }
            if keep_protected {
                let keep = match protected.get(author) {
                    Some(keep) => *keep,
                    None => {
                        let keep = common::is_protected(context, author, None).await;
                        protected.insert(author.to_string(), keep);
                        keep
                    }
                };
                if keep {
                    continue;
                }
            }

            picked.push(id);
            if picked.len() >= count {
                break 'pages;
            }
        }

        if page.len() < 100 {
            break;
        }
    }

    let audit = context.audit_reason(reason);
    for chunk in picked.chunks(100) {
        context
            .rest
            .delete_messages(&channel_id, chunk, Some(&audit))
            .await?;
    }

    let case = cases::record(
        context,
        NewCase::new("purge")
            .on(from)
            .reason(reason)
            .channel(&channel_id)
            .meta("channel", channel_id.clone())
            .meta("asked", count)
            .meta("deleted", picked.len()),
    )
    .await?;

    Ok((picked.len(), case))
}

/// Deletes one message a standing rule (or a check) caught; with `record`, as a case of its
/// own.
pub async fn delete_message(
    context: &Context,
    channel_id: &str,
    message_id: &str,
    author: &Target,
    reason: Option<&str>,
    rule_id: Option<i64>,
    record: bool,
) -> Result<Option<Case>, ToolError> {
    context
        .rest
        .delete_messages(
            channel_id,
            &[message_id.to_string()],
            Some(&context.audit_reason(reason)),
        )
        .await?;

    if !record {
        return Ok(None);
    }

    Ok(Some(
        cases::record(
            context,
            NewCase::new("delete")
                .on(Some(author))
                .reason(reason)
                .channel(channel_id)
                .automatic(false, rule_id)
                .meta("message", message_id),
        )
        .await?,
    ))
}

/// The name a message's author goes by.
fn author_name(message: &Value) -> String {
    [
        &message["member"]["nick"],
        &message["author"]["global_name"],
        &message["author"]["username"],
        &message["author"]["id"],
    ]
    .into_iter()
    .filter_map(Value::as_str)
    .find(|name| !name.is_empty())
    .unwrap_or("unknown")
    .to_string()
}

/// Deletes chosen messages of one channel, whatever their age (Discord bulk deletes only the
/// last 14 days; older ones go one by one), and keeps one case per author with what they said.
/// A message that is gone by now is skipped. Answers how many were deleted, and the cases.
pub async fn delete_messages(
    context: &Context,
    channel: &Value,
    messages: &[Value],
    reason: Option<&str>,
) -> Result<(usize, Vec<Case>), ToolError> {
    let channel_id = channel["id"].as_str().unwrap_or_default().to_string();
    let recent = bulk_cutoff();
    let audit = context.audit_reason(reason);
    let mut bulk: Vec<&Value> = Vec::new();
    let mut deleted: Vec<&Value> = Vec::new();

    for message in messages {
        if common::sent_at(message).is_some_and(|sent| sent > recent) {
            bulk.push(message);
            continue;
        }

        let id = message["id"].as_str().unwrap_or_default().to_string();
        match context
            .rest
            .delete_messages(&channel_id, &[id], Some(&audit))
            .await
        {
            Ok(()) => deleted.push(message),
            Err(err) if err.is_not_found() => {}
            Err(err) => return Err(err.into()),
        }
    }

    for chunk in bulk.chunks(100) {
        let ids: Vec<String> = chunk
            .iter()
            .map(|message| message["id"].as_str().unwrap_or_default().to_string())
            .collect();
        context
            .rest
            .delete_messages(&channel_id, &ids, Some(&audit))
            .await?;
        deleted.extend(chunk);
    }

    // one case per author, in the order they came
    let mut by_author: Vec<(String, Vec<&Value>)> = Vec::new();
    for message in deleted.iter().copied() {
        let author = message["author"]["id"].as_str().unwrap_or("0").to_string();
        match by_author.iter_mut().find(|(id, _)| *id == author) {
            Some((_, group)) => group.push(message),
            None => by_author.push((author, vec![message])),
        }
    }

    let mut cases_written = Vec::new();
    for (author, group) in by_author {
        let said = group
            .iter()
            .map(|message| text::message_text(message))
            .filter(|said| !said.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");
        let ids: Vec<&str> = group
            .iter()
            .filter_map(|message| message["id"].as_str())
            .collect();
        let target = Target::new(author, author_name(group[0]));

        cases_written.push(
            cases::record(
                context,
                NewCase::new("delete")
                    .on(Some(&target))
                    .reason(reason)
                    .channel(&channel_id)
                    .meta("deleted", group.len())
                    .meta("messages", ids)
                    .meta("said", text::cut(&said, 300, "")),
            )
            .await?,
        );
    }

    Ok((deleted.len(), cases_written))
}

/// Credits given on the main admin's order, for the case log and the mod log (the ledger row is
/// where the credits went).
pub async fn credits(
    context: &Context,
    target: &Target,
    amount: i64,
    reason: Option<&str>,
    account: Value,
) -> Result<Case, ToolError> {
    cases::record(
        context,
        NewCase::new("credits")
            .on(Some(target))
            .reason(reason)
            .meta("credits", amount)
            .meta("account", account),
    )
    .await
}

/// Changes a member's server nickname, or removes it (`None`).
pub async fn nickname(
    context: &Context,
    target: &Target,
    nick: Option<&str>,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    context
        .rest
        .modify_member(
            &context.guild_id,
            &target.id,
            &json!({ "nick": nick }),
            Some(&context.audit_reason(reason)),
        )
        .await?;
    let from = target
        .member
        .as_ref()
        .and_then(|member| member["nick"].as_str())
        .map(str::to_string);

    cases::record(
        context,
        NewCase::new("nickname")
            .on(Some(target))
            .reason(reason)
            .meta("from", from)
            .meta("to", nick),
    )
    .await
}

pub async fn slowmode(
    context: &Context,
    channel: &Value,
    seconds: i64,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    let channel_id = channel["id"].as_str().unwrap_or_default();
    let seconds = seconds.clamp(0, MAX_SLOWMODE_SECONDS);
    context
        .rest
        .modify_channel(
            channel_id,
            &json!({ "rate_limit_per_user": seconds }),
            Some(&context.audit_reason(reason)),
        )
        .await?;

    cases::record(
        context,
        NewCase::new("slowmode")
            .reason(reason)
            .seconds(Some(seconds))
            .channel(channel_id)
            .meta("channel", channel_id),
    )
    .await
}

/// `@everyone`'s overwrite with sending locked or unlocked, the rest of it as it was.
pub fn lock_bits(allow: u64, deny: u64, locked: bool) -> (u64, u64) {
    let bits = SEND_MESSAGES | SEND_IN_THREADS | CREATE_THREADS;

    if locked {
        (allow & !bits, deny | bits)
    } else {
        (allow, deny & !bits)
    }
}

/// Stops (or allows again) `@everyone` sending messages in a channel, keeping every other part
/// of its `@everyone` overwrite as it was.
pub async fn lock(
    context: &Context,
    channel: &Value,
    locked: bool,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    let channel_id = channel["id"].as_str().unwrap_or_default();
    let fresh = context.rest.channel(channel_id).await?;
    let everyone = &context.guild_id;
    let (allow, deny) = fresh["permission_overwrites"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|overwrite| overwrite["id"].as_str() == Some(everyone.as_str()))
        .map_or((0, 0), |overwrite| {
            (
                access::bits(&overwrite["allow"]),
                access::bits(&overwrite["deny"]),
            )
        });
    let (allow, deny) = lock_bits(allow, deny, locked);

    context
        .rest
        .set_permission_overwrite(
            channel_id,
            everyone,
            0,
            &allow.to_string(),
            &deny.to_string(),
            Some(&context.audit_reason(reason)),
        )
        .await?;

    cases::record(
        context,
        NewCase::new(if locked { "lock" } else { "unlock" })
            .reason(reason)
            .channel(channel_id)
            .meta("channel", channel_id),
    )
    .await
}

/// Gives a member a role or takes it away. Roles managed by Discord or an integration are
/// refused, and a role that comes with rewards is never handed out: rux only ever gives credits.
pub async fn role(
    context: &Context,
    target: &Target,
    role: &Value,
    add: bool,
    reason: Option<&str>,
) -> Result<Case, ToolError> {
    let role_id = role["id"].as_str().unwrap_or_default();
    let name = role["name"].as_str().unwrap_or(role_id);

    if role["managed"].as_bool() == Some(true) || role_id == context.guild_id {
        return Err(ToolError::invalid(format!(
            "The role {name} is managed by Discord or an integration (or is @everyone) and cannot be given or taken by hand."
        )));
    }
    if add && access::is_reward_role(role, &context.config.boost_two_role_id) {
        return Err(ToolError::invalid(format!(
            "The role {name} comes with rewards (a booster role), and {} only ever gives credits: it cannot hand this role out. An admin can give it by hand on Discord.",
            context.config.settings.name
        )));
    }

    context
        .rest
        .set_role(
            &context.guild_id,
            &target.id,
            role_id,
            add,
            Some(&context.audit_reason(reason)),
        )
        .await?;

    cases::record(
        context,
        NewCase::new(if add { "role_add" } else { "role_remove" })
            .on(Some(target))
            .reason(reason)
            .meta("role", role_id)
            .meta("role_name", name),
    )
    .await
}

/// A change to rux itself (a rule, a watched channel, a moderator role) in the case log and the
/// mod log.
pub async fn note(
    context: &Context,
    action: &str,
    reason: Option<&str>,
    meta: Map<String, Value>,
) -> Result<Case, ToolError> {
    let mut case = NewCase::new(action).reason(reason);
    if let Some(channel) = meta.get("channel").and_then(Value::as_str) {
        case.channel_id = Some(channel.to_string());
    }
    case.meta = meta;

    cases::record(context, case).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locking_keeps_the_rest_of_the_overwrite() {
        let send = SEND_MESSAGES | SEND_IN_THREADS | CREATE_THREADS;
        let view = access::VIEW_CHANNEL;

        // a lock denies sending, and takes back an explicit allow of it
        assert_eq!(lock_bits(view | SEND_MESSAGES, 0, true), (view, send));
        // an unlock lifts only the denies a lock puts on
        assert_eq!(lock_bits(view, send | (1 << 6), false), (view, 1 << 6));
        assert_eq!(lock_bits(0, 0, false), (0, 0));
    }

    #[test]
    fn authors_go_by_their_server_name() {
        assert_eq!(
            author_name(&json!({"member": {"nick": "Ann"}, "author": {"username": "ann"}})),
            "Ann"
        );
        assert_eq!(
            author_name(&json!({"author": {"id": "1", "username": "ann"}})),
            "ann"
        );
        assert_eq!(author_name(&json!({})), "unknown");
    }
}
