//! plugged.host: the moderation tools about the server itself: channel locks and slowmode,
//! roles (giving them is for admins; looking at one for moderators too) and an overview of the
//! channels, roles and what rux keeps per server.

use super::{
    super::{Tool, ToolError, ToolResult, boolean, required, schema, string},
    common::{
        self, case_result, channel_property, member_property, reason_property, without_nulls,
    },
    service,
};
use crate::rux::{
    access::{self, Level},
    context::Context,
    models::{Case, ModeratorRole},
    resolver, time,
};
use serde_json::{Map, Value, json};

/// Words that turn slowmode off.
const SLOWMODE_OFF: [&str; 3] = ["0", "off", "none"];
/// Channel types `server_overview` lists, with their names.
const KINDS: [(i64, &str); 5] = [
    (0, "text"),
    (2, "voice"),
    (5, "announcement"),
    (13, "stage"),
    (15, "forum"),
];

pub struct ChannelSettings;

#[async_trait::async_trait]
impl Tool for ChannelSettings {
    fn name(&self) -> &'static str {
        "channel_settings"
    }

    fn description(&self, _context: &Context) -> String {
        "Lock or unlock a channel for @everyone, or set its slowmode.".into()
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "channel": channel_property(),
                "locked": { "type": "boolean", "description": "true locks (nobody but staff can write), false unlocks." },
                "slowmode": { "type": "string", "description": "Slowmode per member, like \"10s\", \"1m\", \"6h\" (max). \"0\" turns it off." },
                "reason": reason_property(),
            }),
            &[],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let input = string(arguments, "channel");
        if input.is_none() && context.direct {
            return Err(ToolError::invalid(
                "This order came by DM, so say which channel.",
            ));
        }
        let channel = resolver::channel(
            &context.rest,
            &context.guild_id,
            input.as_deref().unwrap_or(&context.channel_id),
            Some(&[0, 5, 15]),
        )
        .await?;
        common::guard_channel(context, &channel)?;
        common::guard_visible(context, &channel).await?;
        let channel_id = channel["id"].as_str().unwrap_or_default();
        let reason = string(arguments, "reason");
        let mut done = Map::new();

        if !arguments["locked"].is_null() {
            let locked = boolean(arguments, "locked", false);
            // unlocking clears @everyone's send denies, which would open read-only channels
            // (rules, announcements): moderators may only undo a lock the bot put on
            if !locked
                && !context.is_admin()
                && !Case::locked_by_rux(&context.state.database, &context.guild_id, channel_id)
                    .await?
            {
                return Err(ToolError::invalid(format!(
                    "Moderators can only unlock channels {} locked. Tell them to ask an admin.",
                    context.config.settings.name
                )));
            }

            service::lock(context, &channel, locked, reason.as_deref()).await?;
            done.insert("locked".into(), json!(locked));
        }

        if let Some(slowmode) = string(arguments, "slowmode") {
            let seconds = if common::is_off(&slowmode, &SLOWMODE_OFF) {
                0
            } else {
                time::duration(&slowmode).map_err(ToolError::Invalid)?
            };
            let case = service::slowmode(context, &channel, seconds, reason.as_deref()).await?;
            done.insert("slowmode_seconds".into(), json!(case.duration_seconds));
        }

        if done.is_empty() {
            return Err(ToolError::invalid(
                "Say whether to lock/unlock or which slowmode to set.",
            ));
        }

        let mut result = Map::new();
        result.insert("channel".into(), json!(format!("<#{channel_id}>")));
        result.extend(done);

        Ok(ToolResult::json(&Value::Object(result)))
    }
}

pub struct ManageRole;

#[async_trait::async_trait]
impl Tool for ManageRole {
    fn name(&self) -> &'static str {
        "manage_role"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Give a member a role or take it away. Roles that come with rewards (booster roles) cannot be given: {} only ever gives credits.",
            context.config.settings.name
        )
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "member": member_property(common::MEMBER),
                "role": { "type": "string", "description": "The role: name, <@&mention> or id." },
                "add": { "type": "boolean", "description": "true gives the role, false takes it away." },
                "reason": reason_property(),
            }),
            &["member", "role", "add"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let target = common::member(context, &required(arguments, "member")?).await?;
        let role = resolver::role(
            &context.rest,
            &context.guild_id,
            &required(arguments, "role")?,
        )
        .await?;
        let case = service::role(
            context,
            &target,
            &role,
            boolean(arguments, "add", true),
            string(arguments, "reason").as_deref(),
        )
        .await?;

        Ok(ToolResult::json(&case_result(
            &case,
            json!({ "role": role["name"] }),
        )))
    }
}

pub struct RoleInfo;

#[async_trait::async_trait]
impl Tool for RoleInfo {
    fn name(&self) -> &'static str {
        "role_info"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "A role's Discord permissions and settings (position, colour, shown separately, mentionable), and whether it is a {} moderator role.",
            context.config.settings.name
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "role": { "type": "string", "description": "The role: name, <@&mention> or id." },
            }),
            &["role"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        let role = resolver::role(
            &context.rest,
            &context.guild_id,
            &required(arguments, "role")?,
        )
        .await?;
        let id = role["id"].as_str().unwrap_or_default();
        let everyone = id == context.guild_id;
        let permissions = access::bits(&role["permissions"]);
        let colour = role["color"].as_u64().unwrap_or(0);
        let moderator_role = !everyone
            && ModeratorRole::by_role(&context.state.database, &context.guild_id, id)
                .await?
                .is_some();
        let permission_names = if permissions & access::ADMINISTRATOR != 0 {
            vec!["Administrator: every permission, in every channel".to_string()]
        } else {
            let names = access::permission_names(permissions);
            if names.is_empty() {
                vec!["none".to_string()]
            } else {
                names
            }
        };

        Ok(ToolResult::json(&without_nulls(json!({
            "role": if everyone { "@everyone".to_string() } else { format!("<@&{id}>") },
            "name": role["name"],
            "position": format!("{} (higher is above more roles)", role["position"].as_i64().unwrap_or(0)),
            "colour": if colour != 0 { format!("#{colour:06x}") } else { "none".to_string() },
            "shown_separately": role["hoist"].as_bool() == Some(true),
            "mentionable": role["mentionable"].as_bool() == Some(true),
            "managed_by_integration": (role["managed"].as_bool() == Some(true)).then_some(true),
            "permissions": permission_names,
            "note": (!everyone).then_some("Members also have @everyone's permissions, and channel settings can add or take away some per channel."),
            "rux_moderator_role": moderator_role,
            "comes_with_rewards": (!everyone && access::is_reward_role(&role, &context.config.boost_two_role_id))
                .then(|| format!("yes: {} will not give this role to anyone", context.config.settings.name)),
        }))))
    }
}

/// The channels `server_overview` lists, by category (the categories in their order, then the
/// channels in theirs): "#name kind id".
pub fn channels_by_category(channels: &[Value]) -> Map<String, Value> {
    let categories: Vec<&Value> = channels
        .iter()
        .filter(|channel| channel["type"].as_i64() == Some(4))
        .collect();
    let category = |channel: &Value| {
        channel["parent_id"]
            .as_str()
            .and_then(|parent| {
                categories
                    .iter()
                    .find(|category| category["id"].as_str() == Some(parent))
            })
            .copied()
    };
    let kind = |channel: &Value| {
        let kind = channel["type"].as_i64()?;
        KINDS
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, name)| *name)
    };

    let mut listed: Vec<(i64, i64, String, String)> = channels
        .iter()
        .filter_map(|channel| {
            let kind = kind(channel)?;
            let parent = category(channel);
            Some((
                parent.map_or(-1, |parent| parent["position"].as_i64().unwrap_or(0)),
                channel["position"].as_i64().unwrap_or(0),
                parent
                    .and_then(|parent| parent["name"].as_str())
                    .unwrap_or("no category")
                    .to_string(),
                format!(
                    "#{} {kind} {}",
                    channel["name"].as_str().unwrap_or_default(),
                    channel["id"].as_str().unwrap_or_default()
                ),
            ))
        })
        .collect();
    listed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    let mut grouped = Map::new();
    for (_, _, category, line) in listed {
        match grouped.get_mut(&category) {
            Some(Value::Array(lines)) => lines.push(json!(line)),
            _ => {
                grouped.insert(category, json!([line]));
            }
        }
    }

    grouped
}

pub struct ServerOverview;

#[async_trait::async_trait]
impl Tool for ServerOverview {
    fn name(&self) -> &'static str {
        "server_overview"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "The server's channels (by category) and roles with their ids, {}'s watched channels, moderator roles, and default giveaway and mod log channels.",
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
        let channels = context.rest.channels(&context.guild_id, true).await?;
        let mut roles = context.rest.roles(&context.guild_id, true).await?;
        roles.retain(|role| role["id"].as_str() != Some(context.guild_id.as_str()));
        roles.sort_by_key(|role| std::cmp::Reverse(role["position"].as_i64().unwrap_or(0)));
        let roles: Vec<String> = roles
            .iter()
            .map(|role| {
                format!(
                    "{} {}{}",
                    role["name"].as_str().unwrap_or_default(),
                    role["id"].as_str().unwrap_or_default(),
                    if role["managed"].as_bool() == Some(true) {
                        " (managed)"
                    } else {
                        ""
                    }
                )
            })
            .collect();

        let watched = crate::rux::tools::rules::watches::watched_channel_ids(
            &context.state,
            &context.rest,
            &context.guild_id,
            true,
        )
        .await?;
        let moderator_roles: Vec<String> =
            ModeratorRole::all(&context.state.database, &context.guild_id)
                .await?
                .iter()
                .map(|role| {
                    format!(
                        "<@&{}> ({})",
                        role.role_id,
                        role.role_name.as_deref().unwrap_or_default()
                    )
                })
                .collect();
        let guild = context.guild();
        let mention = |channel: Option<&String>| {
            channel.map_or_else(|| "none set".to_string(), |id| format!("<#{id}>"))
        };

        Ok(ToolResult::json(&json!({
            "server": guild.map_or_else(|| context.guild_id.clone(), |guild| guild.label()),
            "watched_channels": watched.iter().map(|id| format!("<#{id}>")).collect::<Vec<_>>(),
            "rux_moderator_roles": moderator_roles,
            "channels": channels_by_category(&channels),
            "roles_highest_first": roles,
            "default_giveaway_channel": mention(guild.and_then(|guild| guild.giveaway_channel_id.as_ref())),
            "mod_log_channel": mention(guild.and_then(|guild| guild.mod_log_channel_id.as_ref())),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_listed_by_category() {
        let channels = vec![
            json!({"id": "10", "type": 4, "name": "Talk", "position": 1}),
            json!({"id": "11", "type": 4, "name": "Info", "position": 0}),
            json!({"id": "1", "type": 0, "name": "general", "position": 2, "parent_id": "10"}),
            json!({"id": "2", "type": 2, "name": "voice", "position": 1, "parent_id": "10"}),
            json!({"id": "3", "type": 5, "name": "news", "position": 0, "parent_id": "11"}),
            json!({"id": "4", "type": 0, "name": "loose", "position": 0}),
            json!({"id": "5", "type": 11, "name": "a-thread", "parent_id": "1"}),
        ];

        let listed = channels_by_category(&channels);
        assert_eq!(
            listed.keys().collect::<Vec<_>>(),
            vec!["no category", "Info", "Talk"]
        );
        assert_eq!(
            Value::Object(listed),
            json!({
                "no category": ["#loose text 4"],
                "Info": ["#news announcement 3"],
                "Talk": ["#voice voice 2", "#general text 1"],
            })
        );
    }
}
