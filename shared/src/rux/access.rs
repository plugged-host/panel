//! plugged.host: who rux obeys and whom it leaves alone.
//!
//!  - admin: a Discord account linked to an admin. With noprofit.space connected, a
//!    noprofit.space admin (noprofit.space says so live, or its account snapshot here when the
//!    rux routes are not set up); without it, a panel admin (`users.admin`) who linked Discord
//!    through the Discord section's OAuth provider. Everything, on every server.
//!  - moderator: a member of a role an admin made a rux moderator role. Moderation only, never on
//!    staff, and only on the server of that role.
//!  - staff: the server owner, anyone whose roles may kick, ban, time out or manage messages or
//!    the server, and every rux moderator. Standing rules and watched channels leave staff alone.
//!
//! Everything but "admin" is per server: a moderator or staff member of one server is nobody
//! special on another.

use super::{
    config::Config,
    context::Context,
    discord::{DiscordError, Rest},
    models::ModeratorRole,
    noprofit::Unavailable,
};
use serde::Serialize;
use serde_json::Value;

/// What someone may have rux do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Admin,
    Moderator,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Moderator => "moderator",
        }
    }
}

pub const ADMINISTRATOR: u64 = 1 << 3;
pub const VIEW_CHANNEL: u64 = 1 << 10;
/// Kick, ban, administrator, manage server, manage messages, time out.
pub const STAFF_PERMISSIONS: u64 =
    (1 << 1) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 13) | (1 << 40);
/// What "give them mod permissions" adds to a role on Discord: kick, ban, manage messages, time
/// out.
pub const MODERATOR_PERMISSIONS: u64 = (1 << 1) | (1 << 2) | (1 << 13) | (1 << 40);

/// Discord's permission bits, as its own settings name them.
const PERMISSION_NAMES: [(u32, &str); 51] = [
    (0, "Create Invite"),
    (1, "Kick Members"),
    (2, "Ban Members"),
    (3, "Administrator"),
    (4, "Manage Channels"),
    (5, "Manage Server"),
    (6, "Add Reactions"),
    (7, "View Audit Log"),
    (8, "Priority Speaker"),
    (9, "Video"),
    (10, "View Channels"),
    (11, "Send Messages"),
    (12, "Send Text-to-Speech Messages"),
    (13, "Manage Messages"),
    (14, "Embed Links"),
    (15, "Attach Files"),
    (16, "Read Message History"),
    (17, "Mention @everyone, @here and All Roles"),
    (18, "Use External Emoji"),
    (19, "View Server Insights"),
    (20, "Connect"),
    (21, "Speak"),
    (22, "Mute Members"),
    (23, "Deafen Members"),
    (24, "Move Members"),
    (25, "Use Voice Activity"),
    (26, "Change Nickname"),
    (27, "Manage Nicknames"),
    (28, "Manage Roles"),
    (29, "Manage Webhooks"),
    (30, "Manage Expressions"),
    (31, "Use Application Commands"),
    (32, "Request to Speak"),
    (33, "Manage Events"),
    (34, "Manage Threads"),
    (35, "Create Public Threads"),
    (36, "Create Private Threads"),
    (37, "Use External Stickers"),
    (38, "Send Messages in Threads"),
    (39, "Use Activities"),
    (40, "Timeout Members"),
    (41, "View Creator Monetization Analytics"),
    (42, "Use Soundboard"),
    (43, "Create Expressions"),
    (44, "Create Events"),
    (45, "Use External Sounds"),
    (46, "Send Voice Messages"),
    (49, "Create Polls"),
    (50, "Use External Apps"),
    (51, "Pin Messages"),
    (52, "Bypass Slowmode"),
];

/// The names of the permission bits in `bits`.
pub fn permission_names(bits: u64) -> Vec<String> {
    (0u32..63)
        .filter(|bit| bits & (1u64 << bit) != 0)
        .map(|bit| {
            PERMISSION_NAMES
                .iter()
                .find(|(b, _)| *b == bit)
                .map_or_else(
                    || format!("permission bit {bit}"),
                    |(_, name)| name.to_string(),
                )
        })
        .collect()
}

/// A permission field as Discord sends it (a decimal string, sometimes a number).
pub fn bits(value: &Value) -> u64 {
    match value {
        Value::String(text) => text.parse().unwrap_or(0),
        Value::Number(number) => number.as_u64().unwrap_or(0),
        _ => 0,
    }
}

/// Someone who wrote to rux, as the account directory knows them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Actor {
    pub discord_id: String,
    pub name: String,
    pub account_id: Option<i64>,
    pub user_uuid: Option<uuid::Uuid>,
    pub is_admin: bool,
}

fn failed(err: impl std::fmt::Display) -> Unavailable {
    Unavailable(format!("the account directory failed: {err}"))
}

/// The account linked to a Discord account; `None` when nobody linked it. [`Unavailable`] when
/// noprofit.space (or the database) cannot say.
pub async fn actor(state: &crate::State, discord_id: &str) -> Result<Option<Actor>, Unavailable> {
    if let Some(client) = super::noprofit::Client::load(state).await.map_err(failed)?
        && client.configured()
    {
        let Some(account) = client.account(discord_id).await? else {
            return Ok(None);
        };
        let user_uuid: Option<uuid::Uuid> = sqlx::query_scalar(
            "SELECT user_uuid FROM user_noprofit_accounts WHERE account_id = $1 AND deleted IS NULL",
        )
        .bind(account.account_id)
        .fetch_optional(state.database.read())
        .await
        .map_err(failed)?;

        return Ok(Some(Actor {
            discord_id: discord_id.to_string(),
            name: account.name,
            account_id: Some(account.account_id),
            user_uuid,
            is_admin: account.is_admin,
        }));
    }

    // the panel's own records: noprofit.space's snapshots while it is connected, else the
    // Discord OAuth links
    let context = crate::discord::Context::load(state).await.map_err(failed)?;
    let Some(user_uuid) = crate::discord::user_for_discord_id(state, &context, discord_id)
        .await
        .map_err(failed)?
    else {
        return Ok(None);
    };

    if context.connected {
        let Some(link) = crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
            .await
            .map_err(failed)?
        else {
            return Ok(None);
        };

        return Ok(Some(Actor {
            discord_id: discord_id.to_string(),
            name: link.username,
            account_id: Some(link.account_id),
            user_uuid: Some(user_uuid),
            is_admin: link.is_admin && link.deleted.is_none(),
        }));
    }

    // a suspended or frozen admin gives no orders
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT username, admin AND NOT suspended AND NOT frozen FROM users WHERE uuid = $1",
    )
    .bind(user_uuid)
    .fetch_optional(state.database.read())
    .await
    .map_err(failed)?;

    Ok(row.map(|(username, admin)| Actor {
        discord_id: discord_id.to_string(),
        name: username,
        account_id: None,
        user_uuid: Some(user_uuid),
        is_admin: admin,
    }))
}

/// Part of the key under which "not allowed" is remembered; bumped when moderator roles change,
/// so a new moderator is heard at once.
pub async fn generation(state: &crate::State) -> i64 {
    state
        .cache
        .get::<i64>("rux::access::generation")
        .await
        .ok()
        .flatten()
        .unwrap_or(0)
}

pub async fn bump_generation(state: &crate::State) {
    let next = generation(state).await + 1;
    if let Err(err) = state
        .cache
        .set("rux::access::generation", 365 * 86400, &next)
        .await
    {
        tracing::warn!("rux: could not bump the access generation: {err:#}");
    }
}

/// Where "not allowed" is remembered for someone, per server (`None`: in DMs), since a
/// moderator of one server is nobody on another.
pub fn denied_key(generation: i64, guild_id: Option<&str>, discord_id: &str) -> String {
    format!(
        "rux::denied::{generation}::{}::{discord_id}",
        guild_id.unwrap_or("dm")
    )
}

/// Which server someone's DM with rux acts on, when they picked one (`switch_server`).
pub fn dm_server_key(discord_id: &str) -> String {
    format!("rux::dm_server::{discord_id}")
}

/// How long a DM keeps acting on the picked server.
pub const DM_SERVER_SECONDS: u64 = 86400;

pub async fn dm_server(state: &crate::State, discord_id: &str) -> Option<String> {
    state
        .cache
        .get::<String>(&dm_server_key(discord_id))
        .await
        .ok()
        .flatten()
}

/// Remembers the server a DM acts on, for a day.
pub async fn remember_dm_server(state: &crate::State, discord_id: &str, guild_id: &str) {
    if let Err(err) = state
        .cache
        .set(
            &dm_server_key(discord_id),
            DM_SERVER_SECONDS,
            &guild_id.to_string(),
        )
        .await
    {
        tracing::warn!("rux: could not remember the DM's server: {err:#}");
    }
}

/// The rux moderator role among these roles of a server's member, if any.
pub async fn moderator_role(
    state: &crate::State,
    guild_id: &str,
    role_ids: &[String],
) -> Result<Option<ModeratorRole>, sqlx::Error> {
    ModeratorRole::among(&state.database, guild_id, role_ids).await
}

/// Whether someone is a rux moderator on this server. `false` when Discord cannot say.
pub async fn is_moderator(
    state: &crate::State,
    rest: &Rest,
    guild_id: &str,
    discord_id: &str,
) -> bool {
    let roles = match rest.member_roles(guild_id, discord_id).await {
        Ok(Some(roles)) => roles,
        Ok(None) => return false,
        Err(err) => {
            tracing::debug!("rux: member roles for {discord_id}: {err}");
            return false;
        }
    };

    match moderator_role(state, guild_id, &roles).await {
        Ok(role) => role.is_some(),
        Err(err) => {
            tracing::warn!("rux: moderator roles: {err}");
            false
        }
    }
}

/// The server's owner (kept an hour).
pub async fn owner(rest: &Rest, guild_id: &str) -> Result<String, DiscordError> {
    Ok(rest.guild(guild_id).await?["owner_id"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

/// Everything the roles of a member allow on the server: @everyone's and theirs.
pub fn base_permissions(guild_id: &str, member_roles: &[String], roles: &[Value]) -> u64 {
    roles
        .iter()
        .filter(|role| {
            role["id"]
                .as_str()
                .is_some_and(|id| id == guild_id || member_roles.iter().any(|r| r == id))
        })
        .fold(0, |bits_so_far, role| {
            bits_so_far | bits(&role["permissions"])
        })
}

/// Whether a member is staff by what Discord says: the owner, or roles that may moderate.
pub fn staff_by_permissions(
    owner_id: &str,
    guild_id: &str,
    discord_id: &str,
    member_roles: &[String],
    roles: &[Value],
) -> bool {
    discord_id == owner_id
        || base_permissions(guild_id, member_roles, roles) & STAFF_PERMISSIONS != 0
}

/// Whether someone is staff on this server. When Discord cannot say, they are: leaving someone
/// alone is the safe mistake. `member_roles`: `None` looks them up.
pub async fn is_staff(
    state: &crate::State,
    rest: &Rest,
    guild_id: &str,
    discord_id: &str,
    member_roles: Option<&[String]>,
) -> bool {
    let looked_up;
    let member_roles: &[String] = match member_roles {
        Some(roles) => roles,
        None => match rest.member_roles(guild_id, discord_id).await {
            Ok(roles) => {
                looked_up = roles.unwrap_or_default();
                &looked_up
            }
            Err(_) => return true,
        },
    };
    let (Ok(owner_id), Ok(roles)) = (
        owner(rest, guild_id).await,
        rest.roles(guild_id, false).await,
    ) else {
        return true;
    };

    if staff_by_permissions(&owner_id, guild_id, discord_id, member_roles, &roles) {
        return true;
    }

    moderator_role(state, guild_id, member_roles)
        .await
        .map_or(true, |role| role.is_some())
}

/// Whether a member can see a channel: View Channel after its overwrites (@everyone's, their
/// roles', their own). rux sees every channel, so a moderator only gets to read or delete in the
/// ones they can open themselves.
pub fn can_view_with(
    owner_id: &str,
    guild_id: &str,
    discord_id: &str,
    member_roles: &[String],
    roles: &[Value],
    channel: &Value,
) -> bool {
    if discord_id == owner_id {
        return true;
    }

    let mut permissions = base_permissions(guild_id, member_roles, roles);
    if permissions & ADMINISTRATOR != 0 {
        return true;
    }

    let overwrites: Vec<&Value> = channel["permission_overwrites"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let find = |id: &str| {
        overwrites
            .iter()
            .find(|overwrite| overwrite["id"].as_str() == Some(id))
            .copied()
    };
    let apply = |permissions: u64, overwrite: Option<&Value>| match overwrite {
        Some(overwrite) => (permissions & !bits(&overwrite["deny"])) | bits(&overwrite["allow"]),
        None => permissions,
    };

    permissions = apply(permissions, find(guild_id));

    let (mut allow, mut deny) = (0u64, 0u64);
    for role in member_roles {
        if let Some(overwrite) = find(role) {
            allow |= bits(&overwrite["allow"]);
            deny |= bits(&overwrite["deny"]);
        }
    }
    permissions = (permissions & !deny) | allow;

    // a member's own overwrite has type 1
    let own = find(discord_id).filter(|overwrite| {
        overwrite["type"].as_i64() == Some(1) || overwrite["type"].as_str() == Some("1")
    });
    permissions = apply(permissions, own);

    permissions & VIEW_CHANNEL != 0
}

/// [`can_view_with`] asking Discord; `false` when Discord cannot say. A thread goes by its
/// parent channel.
pub async fn can_view(rest: &Rest, guild_id: &str, discord_id: &str, channel: &Value) -> bool {
    let Ok(owner_id) = owner(rest, guild_id).await else {
        return false;
    };
    if discord_id == owner_id {
        return true;
    }
    let member_roles = match rest.member_roles(guild_id, discord_id).await {
        Ok(Some(roles)) => roles,
        _ => return false,
    };

    let mut channel = channel.clone();
    if channel["type"]
        .as_i64()
        .is_some_and(|kind| super::discord::THREADS.contains(&kind))
        && let Some(parent) = channel["parent_id"].as_str()
    {
        match rest.channel(parent).await {
            Ok(parent) => channel = parent,
            Err(_) => return false,
        }
    }
    let Ok(roles) = rest.roles(guild_id, false).await else {
        return false;
    };

    can_view_with(
        &owner_id,
        guild_id,
        discord_id,
        &member_roles,
        &roles,
        &channel,
    )
}

/// The server's name on Discord (for members' DMs and the servers list), the host's name when
/// Discord cannot say.
pub async fn server_name(rest: &Rest, config: &Config, guild_id: &str) -> String {
    let fallback = || {
        config
            .guild(guild_id)
            .map_or_else(|| config.app_name.clone(), |guild| guild.host_name.clone())
    };

    match rest.guild(guild_id).await {
        Ok(guild) => guild["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .map_or_else(fallback, str::to_string),
        Err(_) => fallback(),
    }
}

/// Whether a role comes with rewards, so handing it out would give more than credits: roles
/// Discord or an integration manages (the server's Booster role among them), "2x Booster"-style
/// roles (matched by name) and the Discord section's two-boost role. rux only ever gives credits.
pub fn is_reward_role(role: &Value, boost_two_role_id: &str) -> bool {
    let boost_two = boost_two_role_id.trim();

    role["managed"].as_bool() == Some(true)
        || role["tags"]
            .as_object()
            .is_some_and(|tags| tags.contains_key("premium_subscriber"))
        || (!boost_two.is_empty() && role["id"].as_str() == Some(boost_two))
        || role["name"]
            .as_str()
            .is_some_and(|name| name.to_lowercase().contains("boost"))
}

/// Why a role cannot be a moderator role (`@everyone`, or managed by Discord or a bot).
pub fn grantable(guild_id: &str, role: &Value) -> Result<(), String> {
    let name = role["name"].as_str().unwrap_or("That role");
    if role["id"].as_str() == Some(guild_id) || role["managed"].as_bool() == Some(true) {
        return Err(format!(
            "{name} cannot be a moderator role (it is @everyone, or managed by Discord or a bot)."
        ));
    }

    Ok(())
}

/// The permissions rux adds to a role made a moderator role: the moderation ones it lacks,
/// never Administrator.
pub fn permissions_to_add(current: u64) -> u64 {
    (MODERATOR_PERMISSIONS & !current) & !ADMINISTRATOR
}

/// Makes a role's members rux moderators, and with `discord_permissions` gives the role
/// Discord's own moderation permissions too. Answers the row and the permissions added.
pub async fn grant(
    context: &Context,
    role: &Value,
    discord_permissions: bool,
) -> Result<(ModeratorRole, Vec<String>), super::tools::ToolError> {
    grantable(&context.guild_id, role).map_err(super::tools::ToolError::Invalid)?;
    let role_id = role["id"].as_str().unwrap_or_default();
    let current = bits(&role["permissions"]);
    let added = if discord_permissions {
        permissions_to_add(current)
    } else {
        0
    };

    if added != 0 {
        context
            .rest
            .modify_role(
                &context.guild_id,
                role_id,
                &serde_json::json!({ "permissions": (current | added).to_string() }),
                Some(&context.audit_reason(Some("made a rux moderator role"))),
            )
            .await?;
    }

    let existing =
        ModeratorRole::by_role(&context.state.database, &context.guild_id, role_id).await?;
    let before = existing
        .as_ref()
        .map_or(0, |row| row.permissions_added.parse::<u64>().unwrap_or(0));
    let name: String = role["name"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(100)
        .collect();

    sqlx::query(
        "INSERT INTO rux_moderator_roles
            (guild_id, role_id, role_name, granted_by_discord_id, granted_by_account_id, granted_by_user_uuid, permissions_added)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (guild_id, role_id) DO UPDATE SET
            role_name = EXCLUDED.role_name,
            granted_by_discord_id = EXCLUDED.granted_by_discord_id,
            granted_by_account_id = EXCLUDED.granted_by_account_id,
            granted_by_user_uuid = EXCLUDED.granted_by_user_uuid,
            permissions_added = EXCLUDED.permissions_added,
            updated = NOW()",
    )
    .bind(&context.guild_id)
    .bind(role_id)
    .bind(&name)
    .bind(&context.actor_discord_id)
    .bind(context.actor_account_id)
    .bind(context.actor_user_uuid)
    .bind((before | added).to_string())
    .execute(context.state.database.write())
    .await?;
    bump_generation(&context.state).await;

    let row = ModeratorRole::by_role(&context.state.database, &context.guild_id, role_id)
        .await?
        .ok_or_else(|| {
            super::tools::ToolError::Failed(anyhow::anyhow!("the role was not saved"))
        })?;

    Ok((row, permission_names(added)))
}

/// Takes moderator rights away from a role, and with `discord_permissions` the Discord
/// permissions rux added to it. Answers the permissions taken away.
pub async fn revoke(
    context: &Context,
    role: &Value,
    discord_permissions: bool,
) -> Result<Vec<String>, super::tools::ToolError> {
    let role_id = role["id"].as_str().unwrap_or_default();
    let Some(row) =
        ModeratorRole::by_role(&context.state.database, &context.guild_id, role_id).await?
    else {
        return Err(super::tools::ToolError::Invalid(format!(
            "{} is not a rux moderator role.",
            role["name"].as_str().unwrap_or(role_id)
        )));
    };

    let added = row.permissions_added.parse::<u64>().unwrap_or(0);
    let current = bits(&role["permissions"]);
    let removed = if discord_permissions {
        current & added
    } else {
        0
    };
    if removed != 0 {
        context
            .rest
            .modify_role(
                &context.guild_id,
                role_id,
                &serde_json::json!({ "permissions": (current & !added).to_string() }),
                Some(&context.audit_reason(Some("no longer a rux moderator role"))),
            )
            .await?;
    }

    sqlx::query("DELETE FROM rux_moderator_roles WHERE id = $1")
        .bind(row.id)
        .execute(context.state.database.write())
        .await?;
    bump_generation(&context.state).await;

    Ok(permission_names(removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GUILD: &str = "100000000000000001";
    const OWNER: &str = "100000000000000009";
    const MEMBER: &str = "100000000000000002";
    const ROLE: &str = "100000000000000003";

    fn roles(everyone: u64, role: u64) -> Vec<Value> {
        vec![
            json!({"id": GUILD, "permissions": everyone.to_string()}),
            json!({"id": ROLE, "permissions": role.to_string()}),
        ]
    }

    #[test]
    fn staff_is_the_owner_or_moderating_roles() {
        let member_roles = vec![ROLE.to_string()];
        assert!(staff_by_permissions(OWNER, GUILD, OWNER, &[], &roles(0, 0)));
        assert!(!staff_by_permissions(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0)
        ));
        assert!(staff_by_permissions(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(0, 1 << 13)
        ));
        // a role they do not have does not count
        assert!(!staff_by_permissions(
            OWNER,
            GUILD,
            MEMBER,
            &[],
            &roles(0, 1 << 2)
        ));
        assert!(staff_by_permissions(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(0, 1 << 40)
        ));
    }

    #[test]
    fn channel_overwrites_decide_who_sees_a_channel() {
        let member_roles = vec![ROLE.to_string()];
        let open = json!({"permission_overwrites": []});
        assert!(can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0),
            &open
        ));

        let hidden = json!({"permission_overwrites": [
            {"id": GUILD, "type": 0, "allow": "0", "deny": VIEW_CHANNEL.to_string()}
        ]});
        assert!(!can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0),
            &hidden
        ));
        // the owner and administrators see everything
        assert!(can_view_with(
            OWNER,
            GUILD,
            OWNER,
            &[],
            &roles(0, 0),
            &hidden
        ));
        assert!(can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(0, ADMINISTRATOR),
            &hidden
        ));

        let for_role = json!({"permission_overwrites": [
            {"id": GUILD, "type": 0, "allow": "0", "deny": VIEW_CHANNEL.to_string()},
            {"id": ROLE, "type": 0, "allow": VIEW_CHANNEL.to_string(), "deny": "0"}
        ]});
        assert!(can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0),
            &for_role
        ));

        let member_denied = json!({"permission_overwrites": [
            {"id": ROLE, "type": 0, "allow": VIEW_CHANNEL.to_string(), "deny": "0"},
            {"id": MEMBER, "type": 1, "allow": "0", "deny": VIEW_CHANNEL.to_string()}
        ]});
        assert!(!can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0),
            &member_denied
        ));

        // an overwrite with the member's id but of a role (type 0) is not theirs
        let not_theirs = json!({"permission_overwrites": [
            {"id": MEMBER, "type": 0, "allow": "0", "deny": VIEW_CHANNEL.to_string()}
        ]});
        assert!(can_view_with(
            OWNER,
            GUILD,
            MEMBER,
            &member_roles,
            &roles(VIEW_CHANNEL, 0),
            &not_theirs
        ));
    }

    #[test]
    fn reward_roles() {
        assert!(is_reward_role(&json!({"id": "1", "managed": true}), ""));
        assert!(is_reward_role(
            &json!({"id": "1", "tags": {"premium_subscriber": null}}),
            ""
        ));
        assert!(is_reward_role(
            &json!({"id": "1", "name": "2x Booster"}),
            ""
        ));
        assert!(is_reward_role(&json!({"id": ROLE, "name": "VIP"}), ROLE));
        assert!(!is_reward_role(&json!({"id": ROLE, "name": "VIP"}), ""));
    }

    #[test]
    fn moderator_roles() {
        assert!(grantable(GUILD, &json!({"id": GUILD, "name": "@everyone"})).is_err());
        assert!(grantable(GUILD, &json!({"id": ROLE, "name": "Bot", "managed": true})).is_err());
        assert!(grantable(GUILD, &json!({"id": ROLE, "name": "Mods"})).is_ok());

        assert_eq!(permissions_to_add(0), MODERATOR_PERMISSIONS);
        assert_eq!(
            permissions_to_add(1 << 1),
            MODERATOR_PERMISSIONS & !(1 << 1)
        );
        assert_eq!(
            permission_names(MODERATOR_PERMISSIONS),
            vec![
                "Kick Members",
                "Ban Members",
                "Manage Messages",
                "Timeout Members"
            ]
        );
        assert_eq!(permission_names(1 << 47), vec!["permission bit 47"]);
        assert_eq!(bits(&json!("8")), 8);
        assert_eq!(bits(&json!(8)), 8);
        assert_eq!(bits(&json!(null)), 0);
    }

    #[test]
    fn denied_keys_are_per_server_and_generation() {
        assert_eq!(
            denied_key(3, Some(GUILD), MEMBER),
            format!("rux::denied::3::{GUILD}::{MEMBER}")
        );
        assert_eq!(
            denied_key(0, None, MEMBER),
            format!("rux::denied::0::dm::{MEMBER}")
        );
    }
}
