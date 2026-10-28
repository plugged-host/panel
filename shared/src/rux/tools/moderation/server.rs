//! plugged.host: the moderation tools about the server itself: channel locks and slowmode,
//! roles (giving them is for admins; looking at one, and who has it, for moderators too) and an
//! overview of the channels, roles and what rux keeps per server.

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
    discord::{DiscordError, member_name},
    models::{Case, ModeratorRole},
    resolver, text, time,
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

/// Members `role_members` looks through at most (Discord's pages of [`MEMBERS_PAGE`], by user
/// id).
pub const MEMBERS_SCANNED: usize = 10_000;
/// One page of the member list.
const MEMBERS_PAGE: usize = 1000;
/// Members `role_members` names at most.
pub const MEMBERS_LISTED: usize = 100;
/// The characters its names take up at most, so its answer is never cut (the model sees 6000 of
/// a tool's output).
const LISTED_CHARS: usize = 4500;

/// What the model is told when Discord keeps the member list from the bot.
pub fn members_intent(bot: &str) -> String {
    format!(
        "Discord does not let {bot} list the server's members: the bot needs the Server Members intent. An admin turns on Server Members Intent for the bot in the Discord Developer Portal (Bot, Privileged Gateway Intents); then ask again. lookup_member still works for one member."
    )
}

/// A member `role_members` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub id: String,
    /// Their nickname, else display name, else username.
    pub name: String,
    pub username: String,
    pub bot: bool,
}

impl Holder {
    /// A member of Discord's member list, when they have the role (`role_id`; the server's own
    /// id is @everyone, which everyone has).
    pub fn of(member: &Value, role_id: &str, guild_id: &str) -> Option<Self> {
        let id = member["user"]["id"].as_str().filter(|id| !id.is_empty())?;
        let has_role = role_id == guild_id
            || member["roles"]
                .as_array()
                .is_some_and(|roles| roles.iter().any(|role| role.as_str() == Some(role_id)));
        if !has_role {
            return None;
        }

        Some(Self {
            id: id.to_string(),
            name: text::name(&member_name(member), 32),
            username: text::name(member["user"]["username"].as_str().unwrap_or_default(), 32),
            bot: member["user"]["bot"].as_bool() == Some(true),
        })
    }

    /// "Ann (ann) <@id>"; "ann <@id>" when both names are the same; " (bot)" after a bot.
    pub fn line(&self) -> String {
        let same = self.username.is_empty() || self.name.eq_ignore_ascii_case(&self.username);
        let names = if same {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.username)
        };

        format!(
            "{names} <@{}>{}",
            self.id,
            if self.bot { " (bot)" } else { "" }
        )
    }
}

/// The members to name, sorted by name (case does not count; then username and id): at most
/// `max` of them, within `max_chars` together. Answers their lines and how many are left out.
pub fn named(mut holders: Vec<Holder>, max: usize, max_chars: usize) -> (Vec<String>, usize) {
    holders.sort_by_cached_key(|holder| {
        (
            holder.name.to_lowercase(),
            holder.username.to_lowercase(),
            holder.id.parse::<u64>().unwrap_or(u64::MAX),
        )
    });

    let mut lines: Vec<String> = Vec::new();
    let mut chars = 0;
    for holder in &holders {
        let line = holder.line();
        // the quotes and the comma around it in the answer's JSON
        let length = line.chars().count() + 3;
        if lines.len() >= max || chars + length > max_chars {
            break;
        }
        chars += length;
        lines.push(line);
    }
    let left_out = holders.len() - lines.len();

    (lines, left_out)
}

/// What `role_members` found.
struct Scan {
    holders: Vec<Holder>,
    /// Members looked through.
    scanned: usize,
    /// The server has more members than were looked through.
    more: bool,
}

/// The server's members who have the role (`role_id`; the server's id: everyone), through the
/// member list page by page, up to [`MEMBERS_SCANNED`] members.
async fn scan(context: &Context, role_id: &str) -> Result<Scan, DiscordError> {
    let mut holders = Vec::new();
    let mut scanned = 0;
    let mut after: Option<String> = None;

    loop {
        let page = context
            .rest
            .members_page(&context.guild_id, after.as_deref(), MEMBERS_PAGE as u32)
            .await?;
        scanned += page.len();
        holders.extend(
            page.iter()
                .filter_map(|member| Holder::of(member, role_id, &context.guild_id)),
        );

        let last = page
            .last()
            .and_then(|member| member["user"]["id"].as_str())
            .map(str::to_string);
        // the page was full: there can be more after its last member
        let full = page.len() >= MEMBERS_PAGE && last.is_some();
        if !full || scanned >= MEMBERS_SCANNED {
            return Ok(Scan {
                holders,
                scanned,
                more: full,
            });
        }
        after = last;
    }
}

pub struct RoleMembers;

#[async_trait::async_trait]
impl Tool for RoleMembers {
    fn name(&self) -> &'static str {
        "role_members"
    }

    fn description(&self, _context: &Context) -> String {
        format!(
            "Who has a role: how many members, and up to {MEMBERS_LISTED} of them by name (display name, username, mention), sorted by name. Looks through up to {MEMBERS_SCANNED} members; needs the bot's Server Members intent."
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

        let found = match scan(context, id).await {
            Ok(found) => found,
            Err(err) if err.needs_members_intent() => {
                return Err(ToolError::Invalid(members_intent(
                    &context.config.settings.name,
                )));
            }
            Err(err) => return Err(err.into()),
        };
        let count = found.holders.len();
        let (listed, left_out) = named(found.holders, MEMBERS_LISTED, LISTED_CHARS);

        Ok(ToolResult::json(&without_nulls(json!({
            "role": if everyone { "@everyone".to_string() } else { format!("<@&{id}>") },
            "name": role["name"],
            "members": if found.more { json!(format!("at least {count}")) } else { json!(count) },
            "listed_by_name": listed,
            "not_listed": (left_out > 0).then(|| format!("{left_out} more")),
            "note": found.more.then(|| format!(
                "Only the first {} members (the oldest Discord accounts) were looked through; the server has more, so more may have the role.",
                found.scanned
            )),
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

    const GUILD: &str = "100000000000000001";
    const ROLE: &str = "100000000000000008";

    fn member(id: &str, username: &str, nick: Option<&str>, roles: &[&str]) -> Value {
        json!({
            "user": { "id": id, "username": username, "global_name": null },
            "nick": nick,
            "roles": roles,
        })
    }

    fn holder(id: &str, name: &str, username: &str) -> Holder {
        Holder {
            id: id.into(),
            name: name.into(),
            username: username.into(),
            bot: false,
        }
    }

    #[test]
    fn holders_are_the_members_with_the_role() {
        let ann = member("100000000000000006", "ann", Some("Ann\n<3"), &[ROLE]);
        assert_eq!(
            Holder::of(&ann, ROLE, GUILD),
            Some(Holder {
                id: "100000000000000006".into(),
                name: "Ann ‹3".into(),
                username: "ann".into(),
                bot: false,
            })
        );

        let bob = member("100000000000000007", "bob", None, &["100000000000000009"]);
        assert_eq!(Holder::of(&bob, ROLE, GUILD), None);
        // everyone has @everyone (Discord leaves it out of the roles)
        assert!(Holder::of(&bob, GUILD, GUILD).is_some());
        // no user, no holder
        assert_eq!(Holder::of(&json!({"roles": [ROLE]}), ROLE, GUILD), None);

        let mut bot = member("100000000000000010", "mee6", None, &[ROLE]);
        bot["user"]["bot"] = json!(true);
        assert!(Holder::of(&bot, ROLE, GUILD).unwrap().bot);
    }

    #[test]
    fn holders_read_as_one_line_each() {
        assert_eq!(
            holder("100000000000000006", "Ann", "ann_99").line(),
            "Ann (ann_99) <@100000000000000006>"
        );
        // the same name twice is said once
        assert_eq!(
            holder("100000000000000006", "Ann", "ann").line(),
            "Ann <@100000000000000006>"
        );
        let mut bot = holder("100000000000000010", "mee6", "mee6");
        bot.bot = true;
        assert_eq!(bot.line(), "mee6 <@100000000000000010> (bot)");
    }

    #[test]
    fn holders_are_sorted_by_name_and_capped() {
        let holders = vec![
            holder("100000000000000003", "carl", "carl"),
            holder("100000000000000001", "Ann", "ann"),
            holder("100000000000000004", "bob", "zed"),
            holder("100000000000000002", "Bob", "bob"),
        ];

        let (lines, left_out) = named(holders.clone(), 100, 4500);
        assert_eq!(
            lines,
            vec![
                "Ann <@100000000000000001>",
                "Bob <@100000000000000002>",
                "bob (zed) <@100000000000000004>",
                "carl <@100000000000000003>",
            ]
        );
        assert_eq!(left_out, 0);

        // at most `max`
        let (lines, left_out) = named(holders.clone(), 2, 4500);
        assert_eq!(lines.len(), 2);
        assert_eq!(left_out, 2);

        // and within the characters: "Ann <@…>" is 25, plus 3 for its JSON
        let (lines, left_out) = named(holders, 100, 56);
        assert_eq!(
            lines,
            vec!["Ann <@100000000000000001>", "Bob <@100000000000000002>"]
        );
        assert_eq!(left_out, 2);

        assert_eq!(named(Vec::new(), 100, 4500), (Vec::new(), 0));
    }

    #[test]
    fn the_intent_is_explained() {
        let said = members_intent("rux");
        assert!(said.starts_with("Discord does not let rux list the server's members"));
        assert!(said.contains("Server Members Intent"));
        assert!(said.contains("Developer Portal"));
    }

    #[test]
    fn a_full_answer_fits_what_the_model_sees() {
        let holders: Vec<Holder> = (0..150)
            .map(|n| {
                holder(
                    &format!("1000000000000001{n:02}"),
                    &"N".repeat(32),
                    &"u".repeat(32),
                )
            })
            .collect();
        let (lines, left_out) = named(holders, MEMBERS_LISTED, LISTED_CHARS);
        assert_eq!(lines.len() + left_out, 150);
        assert!(lines.len() < MEMBERS_LISTED);
        let answer = serde_json::to_string(&lines).unwrap();
        assert!(
            answer.chars().count() <= LISTED_CHARS + 2,
            "{}",
            answer.len()
        );
    }

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
