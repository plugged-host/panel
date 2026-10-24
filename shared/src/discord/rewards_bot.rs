//! plugged.host: the invite and boost bot (docs/features/discord.md), a [`Handler`] on the
//! gateway with the bot token.
//!
//! It keeps the server's invite codes with their uses; a join is put down to the code whose uses
//! went up ([`invites::attribute`]), and the known count of that code goes up by one rather than
//! being read again, so a burst of joins on one code is not lost. Until the codes could be read
//! (and after a read failed) a join only takes the counts as they are and counts for nobody, so a
//! missed read never puts a join down to the wrong invite. Leaves stop an invite counting
//! (and take a paid one back), member updates bring boosts in at once. `/invites [user]`,
//! `/topinvites [limit]` and `/setboost user count` answer in the server; `/setboost` is for
//! administrators, checked here on the member's permissions and not only by Discord's command
//! permission (which a server admin can widen).

use super::{
    Context,
    gateway::{Handler, INTENT_GUILD_INVITES, INTENT_GUILD_MEMBERS, INTENT_GUILDS, Session},
    invites,
    members::{self, BoostsOutcome},
    rest,
};
use std::collections::HashMap;

/// Discord's message flag for "only you can see this".
const EPHEMERAL: u64 = 1 << 6;
/// The embeds' colours: Discord's blurple and a neutral grey.
const COLOUR: u32 = 0x5865F2;
const COLOUR_MUTED: u32 = 0x6B7280;

#[derive(Default)]
pub struct RewardsBot {
    /// The server's invite codes and their uses; `None` until they could be read.
    invites: tokio::sync::Mutex<Option<HashMap<String, i64>>>,
}

#[async_trait::async_trait]
impl Handler for RewardsBot {
    fn intents(&self) -> u64 {
        INTENT_GUILDS | INTENT_GUILD_MEMBERS | INTENT_GUILD_INVITES
    }

    fn commands(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "name": "invites",
                "type": 1,
                "description": "Show the invites and rewards of a member",
                "options": [{
                    "type": 6,
                    "name": "user",
                    "description": "The member (you, when left out)",
                    "required": false,
                }],
            }),
            serde_json::json!({
                "name": "topinvites",
                "type": 1,
                "description": "Show the members who brought the most people",
                "options": [{
                    "type": 4,
                    "name": "limit",
                    "description": "How many to show (at most 25)",
                    "required": false,
                    "min_value": 1,
                    "max_value": 25,
                }],
            }),
            serde_json::json!({
                "name": "setboost",
                "type": 1,
                "description": "Set how often a member boosts the server",
                "default_member_permissions": "8",
                "options": [
                    {
                        "type": 6,
                        "name": "user",
                        "description": "The member",
                        "required": true,
                    },
                    {
                        "type": 4,
                        "name": "count",
                        "description": "Their boosts",
                        "required": true,
                        "choices": [
                            { "name": "2 boosts", "value": 2 },
                            { "name": "1 boost", "value": 1 },
                            { "name": "No boost", "value": 0 },
                        ],
                    },
                ],
            }),
        ]
    }

    async fn event(
        &self,
        state: &crate::State,
        session: &Session,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        // events of other servers the bot is in
        let guild = match name {
            "READY" | "RESUMED" => None,
            _ => data["guild_id"].as_str(),
        };
        if guild.is_some_and(|guild| guild != session.guild_id) {
            return Ok(());
        }

        match name {
            "READY" | "RESUMED" | "INVITE_CREATE" | "INVITE_DELETE" => {
                self.refresh(session).await;
                Ok(())
            }
            "GUILD_MEMBER_ADD" => self.joined(state, session, data).await,
            "GUILD_MEMBER_REMOVE" => left(state, data).await,
            "GUILD_MEMBER_UPDATE" => updated(state, data).await,
            "INTERACTION_CREATE" => interaction(state, session, data).await,
            _ => Ok(()),
        }
    }
}

impl RewardsBot {
    /// Reads the server's invites again (after connecting, and when one is made or deleted).
    async fn refresh(&self, session: &Session) {
        match session.rest.invites(&session.guild_id).await {
            Ok(current) => *self.invites.lock().await = Some(uses(&current)),
            Err(err) => {
                *self.invites.lock().await = None;
                tracing::warn!(
                    "the Discord bot cannot read the server's invites (it needs Manage Server): {err:#}"
                );
            }
        }
    }

    async fn joined(
        &self,
        state: &crate::State,
        session: &Session,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let member: rest::Member = serde_json::from_value(data.clone())?;
        let Some(user) = member.user.as_ref().filter(|user| !user.bot) else {
            return Ok(());
        };
        let context = Context::load(state).await?;

        let mut attribution = None;
        match session.rest.invites(&session.guild_id).await {
            Ok(current) => {
                let mut known = self.invites.lock().await;
                match known.as_mut() {
                    Some(known) => {
                        if let Some(invite) = invites::attribute(known, &current) {
                            attribution = Some((
                                invite.inviter.as_ref().map(|inviter| inviter.id.clone()),
                                invite.code.clone(),
                            ));
                            // one use: a burst of joins read later must still each count
                            *known.entry(invite.code.clone()).or_insert(0) += 1;
                        }
                    }
                    // nothing to compare with: from now on these are the counts
                    None => *known = Some(uses(&current)),
                }
            }
            Err(err) => {
                // the counts may have moved without us: start over at the next read
                *self.invites.lock().await = None;
                tracing::warn!("failed to read the Discord invites for a join: {err:#}");
            }
        }

        let (inviter, code) = match &attribution {
            Some((inviter, code)) => (inviter.as_deref(), Some(code.as_str())),
            None => (None, None),
        };
        invites::record_join(state, &context, inviter, &user.id, code).await?;

        if let Some(user_uuid) = super::user_for_discord_id(state, &context, &user.id).await? {
            members::record(state, &context, &user.id, Some(user_uuid), Some(&member)).await?;
        }

        Ok(())
    }
}

/// Invite codes and their uses.
fn uses(invites: &[rest::Invite]) -> HashMap<String, i64> {
    invites
        .iter()
        .map(|invite| (invite.code.clone(), invite.uses))
        .collect()
}

async fn left(state: &crate::State, data: &serde_json::Value) -> Result<(), anyhow::Error> {
    let Some(discord_id) = data["user"]["id"].as_str() else {
        return Ok(());
    };
    let context = Context::load(state).await?;

    invites::record_leave(state, &context, discord_id).await?;
    members::left(state, &context, discord_id).await
}

/// Roles or boosting changed: a linked member's boosts come in now.
async fn updated(state: &crate::State, data: &serde_json::Value) -> Result<(), anyhow::Error> {
    let member: rest::Member = serde_json::from_value(data.clone())?;
    let Some(user) = &member.user else {
        return Ok(());
    };
    let context = Context::load(state).await?;

    if let Some(user_uuid) = super::user_for_discord_id(state, &context, &user.id).await? {
        members::record(state, &context, &user.id, Some(user_uuid), Some(&member)).await?;
    }

    Ok(())
}

/// A slash command's option.
fn option<'a>(data: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    data["data"]["options"]
        .as_array()?
        .iter()
        .find(|option| option["name"].as_str() == Some(name))
        .map(|option| &option["value"])
}

/// A user the command was given: its name and avatar from the interaction's resolved data.
fn resolved_user(data: &serde_json::Value, id: &str) -> rest::User {
    serde_json::from_value(data["data"]["resolved"]["users"][id].clone()).unwrap_or(rest::User {
        id: id.to_string(),
        ..Default::default()
    })
}

async fn interaction(
    state: &crate::State,
    session: &Session,
    data: &serde_json::Value,
) -> Result<(), anyhow::Error> {
    // application commands only
    if data["type"].as_u64() != Some(2) {
        return Ok(());
    }
    let (Some(id), Some(token), Some(command)) = (
        data["id"].as_str(),
        data["token"].as_str(),
        data["data"]["name"].as_str(),
    ) else {
        return Ok(());
    };
    if !matches!(command, "invites" | "topinvites" | "setboost") {
        return Ok(());
    }
    let context = Context::load(state).await?;

    match command {
        "invites" => {
            let invoker = data["member"]["user"].clone();
            let target = match option(data, "user").and_then(|value| value.as_str()) {
                Some(target) => resolved_user(data, target),
                None => serde_json::from_value(invoker).unwrap_or_default(),
            };
            if !super::is_snowflake(&target.id) {
                return Ok(());
            }
            let embed = invites_embed(state, &context, &target).await?;
            session
                .rest
                .reply(id, token, 4, Some(serde_json::json!({ "embeds": [embed] })))
                .await
        }
        "topinvites" => {
            let limit = option(data, "limit")
                .and_then(|value| value.as_i64())
                .unwrap_or(10)
                .clamp(1, 25);
            let embed = leaderboard_embed(state, &context, limit).await?;
            session
                .rest
                .reply(id, token, 4, Some(serde_json::json!({ "embeds": [embed] })))
                .await
        }
        _ => {
            if !super::is_administrator(data["member"]["permissions"].as_str()) {
                return session
                    .rest
                    .reply(
                        id,
                        token,
                        4,
                        Some(serde_json::json!({
                            "content": "Only server administrators can use /setboost.",
                            "flags": EPHEMERAL,
                        })),
                    )
                    .await;
            }
            let (Some(target), Some(count)) = (
                option(data, "user").and_then(|value| value.as_str()),
                option(data, "count").and_then(|value| value.as_i64()),
            ) else {
                return Ok(());
            };
            let count = count.clamp(0, 2) as i16;

            // Discord's role and member calls can take longer than an answer may
            session.rest.reply(id, token, 5, None).await?;
            let embed = match members::set_boosts(state, &context, target, count).await {
                Ok(set) => setboost_embed(&context, &resolved_user(data, target), count, &set),
                Err(err) => {
                    tracing::warn!(discord = %target, "/setboost failed: {err:#}");
                    serde_json::json!({
                        "title": "Boosts not changed",
                        "description": "Something went wrong; try again in a minute.",
                        "color": COLOUR_MUTED,
                    })
                }
            };
            session
                .rest
                .edit_reply(
                    &session.application_id,
                    token,
                    &serde_json::json!({ "embeds": [embed] }),
                )
                .await
        }
    }
}

fn plural(count: i64, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn field(name: &str, value: impl Into<String>, inline: bool) -> serde_json::Value {
    serde_json::json!({ "name": name, "value": value.into(), "inline": inline })
}

fn footer(context: &Context) -> String {
    let settings = &context.settings;
    let mut footer = format!(
        "{} - Accounts must be at least {} old",
        context.app_name,
        plural(settings.min_account_age_days.max(0) as i64, "day", "days")
    );
    if settings.invite_maturity_hours > 0 {
        footer.push_str(&format!(
            " - Rewards are held {}h",
            settings.invite_maturity_hours
        ));
        if settings.invite_clawback {
            footer.push_str(" and taken back if the member leaves");
        }
    }

    footer
}

async fn invites_embed(
    state: &crate::State,
    context: &Context,
    target: &rest::User,
) -> Result<serde_json::Value, anyhow::Error> {
    let summary = invites::summary(&state.database, &target.id).await?;
    let user_uuid = super::user_for_discord_id(state, context, &target.id).await?;
    let boosts: Option<i16> =
        sqlx::query_scalar("SELECT boosts FROM discord_members WHERE discord_id = $1")
            .bind(&target.id)
            .fetch_optional(state.database.read())
            .await?;

    let mut fields = vec![
        field(
            "Counted",
            (summary.paid + summary.waiting).to_string(),
            true,
        ),
        field("Paid", summary.paid.to_string(), true),
        field(
            &if context.settings.invite_maturity_hours > 0 {
                format!("Held ({}h)", context.settings.invite_maturity_hours)
            } else {
                "Held".to_string()
            },
            summary.waiting.to_string(),
            true,
        ),
        field(
            &format!(
                "Too new (under {}d)",
                context.settings.min_account_age_days.max(0)
            ),
            summary.too_new.to_string(),
            true,
        ),
        field("Left", summary.left.to_string(), true),
    ];
    if context.pays() {
        fields.push(field(
            "Credits from invites",
            format!("+{}", summary.credits),
            true,
        ));
        if summary.owed > 0 {
            fields.push(field(
                "Owed",
                format!(
                    "{} credits, taken from the next invite rewards",
                    summary.owed
                ),
                true,
            ));
        }
    }
    fields.push(field(
        "Boosts",
        match boosts.unwrap_or(0) {
            0 => "Not boosting".to_string(),
            count => plural(count as i64, "boost", "boosts"),
        },
        true,
    ));
    fields.push(field(
        "Panel account",
        match user_uuid {
            Some(_) => "Linked",
            None if context.pays() => "Not linked yet: rewards wait until it is",
            None => "Not linked",
        },
        false,
    ));

    let mut embed = serde_json::json!({
        "title": "Invites",
        "description": format!("Invites and rewards of <@{}>", target.id),
        "color": COLOUR,
        "fields": fields,
        "footer": { "text": footer(context) },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if let Some(avatar) = super::avatar_url(&target.id, target.avatar.as_deref()) {
        embed["thumbnail"] = serde_json::json!({ "url": avatar });
    }

    Ok(embed)
}

async fn leaderboard_embed(
    state: &crate::State,
    context: &Context,
    limit: i64,
) -> Result<serde_json::Value, anyhow::Error> {
    let top = invites::leaderboard(&state.database, limit).await?;
    let pays = context.pays();

    let description = if top.is_empty() {
        "No invites were counted yet.".to_string()
    } else {
        top.iter()
            .enumerate()
            .map(|(index, inviter)| {
                let mut line = format!(
                    "**{}.** <@{}> - **{}** counted",
                    index + 1,
                    inviter.discord_id,
                    inviter.counted
                );
                if pays {
                    line.push_str(&format!(
                        " ({} paid, +{} credits)",
                        inviter.paid, inviter.credits
                    ));
                }
                let mut extra = Vec::new();
                if inviter.waiting > 0 {
                    extra.push(format!("{} held", inviter.waiting));
                }
                if inviter.left > 0 {
                    extra.push(format!("{} left", inviter.left));
                }
                if !extra.is_empty() {
                    line.push_str(&format!(", {}", extra.join(", ")));
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut fields = Vec::new();
    if pays {
        fields.push(field(
            "Reward",
            format!(
                "{} per member who stays",
                plural(
                    context.settings.credits_per_invite.max(0) as i64,
                    "credit",
                    "credits"
                )
            ),
            true,
        ));
    }

    Ok(serde_json::json!({
        "title": "Top inviters",
        "description": description,
        "color": COLOUR,
        "fields": fields,
        "footer": { "text": format!("{} - Members who left do not count", footer(context)) },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

fn setboost_embed(
    context: &Context,
    target: &rest::User,
    count: i16,
    set: &members::BoostsSet,
) -> serde_json::Value {
    let boosts = |count: i16| match count {
        0 => "None".to_string(),
        count => plural(count as i64, "boost", "boosts"),
    };
    let (value, level) = match set.outcome {
        BoostsOutcome::Level(level) if level == count => (boosts(count), Some(level)),
        BoostsOutcome::Level(0) => (
            "None: Discord says they are not boosting".to_string(),
            Some(0),
        ),
        BoostsOutcome::Level(level) => (
            format!("{}: Discord says they are boosting", boosts(level)),
            Some(level),
        ),
        BoostsOutcome::NotInServer => ("None: not in the server".to_string(), None),
        BoostsOutcome::Unknown => (
            format!(
                "{} once Discord answers (it did not just now)",
                boosts(count)
            ),
            None,
        ),
    };
    let role = if context.settings.boost_two_role_id.is_empty() {
        "No two-boost role set"
    } else {
        match level {
            Some(2) => "Given",
            Some(_) => "Not given",
            None => "Unchanged",
        }
    };
    let account = match set.user_uuid {
        Some(_) if context.pays() && set.paid > 0 => format!("Linked, +{} credits", set.paid),
        Some(_) => "Linked".to_string(),
        None if context.pays() => "Not linked yet: boost rewards wait until it is".to_string(),
        None => "Not linked".to_string(),
    };

    serde_json::json!({
        "title": "Boosts set",
        "color": if level.unwrap_or(count) > 0 { COLOUR } else { COLOUR_MUTED },
        "fields": [
            field("Member", format!("<@{}>", target.id), true),
            field("Boosts", value, true),
            field("Two-boost role", role, true),
            field("Panel account", account, false),
        ],
        "footer": { "text": format!("{} - Admin action", context.app_name) },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_read_by_name() {
        let data = serde_json::json!({
            "data": {
                "name": "setboost",
                "options": [
                    { "name": "user", "type": 6, "value": "123456789012345678" },
                    { "name": "count", "type": 4, "value": 2 },
                ],
                "resolved": { "users": { "123456789012345678": { "id": "123456789012345678", "username": "someone" } } },
            },
        });

        assert_eq!(option(&data, "count").and_then(|v| v.as_i64()), Some(2));
        assert!(option(&data, "limit").is_none());
        assert_eq!(
            resolved_user(&data, "123456789012345678")
                .username
                .as_deref(),
            Some("someone")
        );
        assert_eq!(resolved_user(&data, "1").id, "1");
    }

    #[test]
    fn setboost_says_what_discord_allows() {
        let context = Context {
            settings: crate::settings::discord::AppSettingsDiscord::default(),
            connected: false,
            credits_enabled: true,
            accounts_url: None,
            app_name: "Panel".into(),
        };
        let target = rest::User {
            id: "123456789012345678".into(),
            ..Default::default()
        };
        let set = members::BoostsSet {
            user_uuid: None,
            outcome: BoostsOutcome::Level(0),
            paid: 0,
        };

        let embed = setboost_embed(&context, &target, 2, &set);
        assert_eq!(
            embed["fields"][1]["value"],
            "None: Discord says they are not boosting"
        );
        assert_eq!(embed["fields"][2]["value"], "No two-boost role set");
        // without a bot token the panel pays nothing, so nothing waits for a link
        assert_eq!(embed["fields"][3]["value"], "Not linked");

        let set = members::BoostsSet {
            outcome: BoostsOutcome::Level(2),
            ..set
        };
        assert_eq!(
            setboost_embed(&context, &target, 2, &set)["fields"][1]["value"],
            "2 boosts"
        );
        let set = members::BoostsSet {
            outcome: BoostsOutcome::NotInServer,
            ..set
        };
        assert_eq!(
            setboost_embed(&context, &target, 1, &set)["fields"][1]["value"],
            "None: not in the server"
        );
    }

    #[test]
    fn the_bot_asks_for_members_and_invites() {
        let bot = RewardsBot::default();
        assert_eq!(bot.intents() & INTENT_GUILD_MEMBERS, INTENT_GUILD_MEMBERS);
        assert_eq!(bot.intents() & INTENT_GUILD_INVITES, INTENT_GUILD_INVITES);

        let commands = bot.commands();
        let setboost = commands
            .iter()
            .find(|command| command["name"] == "setboost")
            .unwrap();
        assert_eq!(setboost["default_member_permissions"], "8");
    }
}
