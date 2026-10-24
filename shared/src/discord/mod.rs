//! plugged.host: Discord (docs/features/discord.md).
//!
//! A player's Discord account is the one linked through the OAuth provider Admin › Settings ›
//! Accounts › Discord names, or, with noprofit.space connected, the one on their noprofit.space
//! account (its snapshot). Without noprofit.space the panel checks membership of the Discord
//! server itself ([`members`]), adds the Discord tier to the allowance of members, pays every boost
//! level once and pays invites ([`invites`]); with noprofit.space connected noprofit.space does
//! the membership, the bonus and the boosts, and the panel only shows them.
//!
//! The gateway ([`gateway`]) keeps the bots connected on the primary instance only; the invite and
//! boost bot ([`rewards_bot`]) records who joined through whose invite and answers `/invites`,
//! `/topinvites` and `/setboost`.

pub mod gateway;
pub mod invites;
pub mod members;
pub mod rest;
pub mod rewards;
pub mod rewards_bot;

use crate::{allowance::Resources, settings::discord::AppSettingsDiscord};

/// Ledger types (credits).
pub const TYPE_INVITE: &str = "discord_invite";
pub const TYPE_INVITE_CLAWBACK: &str = "discord_invite_clawback";
pub const TYPE_INVITE_DEBT: &str = "discord_invite_debt";
pub const TYPE_BOOST: &str = "discord_boost";
pub const LEDGER_TYPES: [&str; 4] = [
    TYPE_INVITE,
    TYPE_INVITE_CLAWBACK,
    TYPE_INVITE_DEBT,
    TYPE_BOOST,
];

/// The allowance cut a member who left (or unlinked) leaves (shared/src/allowance_cuts.rs).
pub const REASON_DISCORD: &str = "discord";

/// Discord ids count milliseconds from the start of 2015.
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

/// Whether `id` looks like a Discord id.
pub fn is_snowflake(id: &str) -> bool {
    !id.is_empty() && crate::settings::discord::snowflake_or_empty(id)
}

/// When a Discord account (or anything else with an id) was made.
pub fn created_at(id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let id: u64 = id.parse().ok()?;

    chrono::DateTime::from_timestamp_millis(((id >> 22) as i64).checked_add(DISCORD_EPOCH_MS)?)
}

/// Whether a member's permissions (Discord's decimal bit field) include Administrator (0x8).
/// Only the low four bits matter and 10000 is a multiple of 16, so the last four digits decide,
/// whatever the field's length.
pub fn is_administrator(permissions: Option<&str>) -> bool {
    let Some(permissions) =
        permissions.filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
    else {
        return false;
    };
    let tail = &permissions[permissions.len().saturating_sub(4)..];

    tail.parse::<u32>().is_ok_and(|bits| bits & 0x8 == 0x8)
}

/// An avatar's address: noprofit.space gives a whole URL, Discord a hash.
pub fn avatar_url(discord_id: &str, avatar: Option<&str>) -> Option<String> {
    let avatar = avatar.filter(|avatar| !avatar.is_empty())?;
    if avatar.starts_with("https://") {
        return Some(avatar.to_string());
    }
    if !avatar
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }

    Some(format!(
        "https://cdn.discordapp.com/avatars/{discord_id}/{avatar}.png?size=128"
    ))
}

/// The settings everything here works from.
#[derive(Clone)]
pub struct Context {
    pub settings: AppSettingsDiscord,
    pub connected: bool,
    pub credits_enabled: bool,
    /// noprofit.space's accounts site, while it is connected.
    pub accounts_url: Option<String>,
    pub app_name: String,
}

impl Context {
    pub async fn load(state: &crate::State) -> Result<Self, anyhow::Error> {
        let settings = state.settings.get().await?;
        let noprofit = settings.noprofit.config();

        Ok(Self {
            settings: settings.discord.clone(),
            connected: noprofit.is_some(),
            credits_enabled: settings.economy.credits_enabled,
            accounts_url: noprofit.map(|config| config.accounts_url),
            app_name: settings.app.name.to_string(),
        })
    }

    /// A REST client, when the server and the bot token are set.
    pub fn rest(&self, state: &crate::State) -> Option<rest::Client> {
        if !self.settings.can_check() {
            return None;
        }

        Some(rest::Client::new(
            &state.client,
            self.settings.bot_token.as_deref()?,
        ))
    }

    /// The panel links, checks and pays itself (no noprofit.space).
    pub fn standalone(&self) -> bool {
        self.settings.standalone(self.connected)
    }

    /// The panel pays invite and boost rewards: standalone, with credits on.
    pub fn pays(&self) -> bool {
        self.standalone() && self.credits_enabled
    }
}

/// The Discord account linked to a panel account.
pub async fn linked_discord_id(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
) -> Result<Option<String>, sqlx::Error> {
    if context.connected {
        let link = crate::noprofit::Link::by_user_uuid(&state.database, user_uuid).await?;

        return Ok(link
            .and_then(|link| link.discord)
            .and_then(|discord| discord.get("id")?.as_str().map(str::to_string))
            .filter(|id| is_snowflake(id)));
    }

    let Some(provider) = context.settings.oauth_provider_uuid else {
        return Ok(None);
    };

    sqlx::query_scalar(
        "SELECT identifier FROM user_oauth_links WHERE user_uuid = $1 AND oauth_provider_uuid = $2",
    )
    .bind(user_uuid)
    .bind(provider)
    .fetch_optional(state.database.read())
    .await
}

/// The panel account a Discord account is linked to.
pub async fn user_for_discord_id(
    state: &crate::State,
    context: &Context,
    discord_id: &str,
) -> Result<Option<uuid::Uuid>, sqlx::Error> {
    if context.connected {
        return sqlx::query_scalar(
            "SELECT user_uuid FROM user_noprofit_accounts
            WHERE discord->>'id' = $1 AND deleted IS NULL
            ORDER BY synced DESC LIMIT 1",
        )
        .bind(discord_id)
        .fetch_optional(state.database.read())
        .await;
    }

    let Some(provider) = context.settings.oauth_provider_uuid else {
        return Ok(None);
    };

    sqlx::query_scalar(
        "SELECT user_uuid FROM user_oauth_links WHERE oauth_provider_uuid = $1 AND identifier = $2",
    )
    .bind(provider)
    .bind(discord_id)
    .fetch_optional(state.database.read())
    .await
}

/// What the Discord tier adds to an account's allowance: the tier while its linked Discord
/// account is in the server (standalone only; with noprofit.space it is in the plan).
pub async fn tier(state: &crate::State, user_uuid: uuid::Uuid) -> Result<Resources, anyhow::Error> {
    let (settings, connected) = {
        let settings = state.settings.get().await?;
        (settings.discord.clone(), settings.noprofit.connected())
    };
    let tier = settings.tier();
    if !settings.standalone(connected) || tier == Resources::default() {
        return Ok(Resources::default());
    }

    let member: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM user_oauth_links
            JOIN discord_members ON discord_members.discord_id = user_oauth_links.identifier
            WHERE user_oauth_links.user_uuid = $1 AND user_oauth_links.oauth_provider_uuid = $2
                AND discord_members.in_guild
        )",
    )
    .bind(user_uuid)
    .bind(settings.oauth_provider_uuid)
    .fetch_one(state.database.read())
    .await?;

    Ok(if member { tier } else { Resources::default() })
}

/// What the Discord settings ask of a server's owner before it gets a subdomain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Open,
    /// The owner has no linked Discord account.
    NeedsLink,
    /// The owner's Discord account is not in the server.
    NeedsMember,
}

/// Whether the server's owner may get a subdomain. `live` asks Discord when the owner is not
/// known as a member yet (creating one); the page only reads what was recorded, and shows an
/// account that was never checked as open, so creating asks.
pub async fn subdomain_gate(
    state: &crate::State,
    owner_uuid: uuid::Uuid,
    live: bool,
) -> Result<Gate, anyhow::Error> {
    let context = Context::load(state).await?;
    let settings = &context.settings;
    if !settings.active()
        || (!settings.subdomains_require_link && !settings.subdomains_require_member)
    {
        return Ok(Gate::Open);
    }

    let Some(discord_id) = linked_discord_id(state, &context, owner_uuid).await? else {
        return Ok(Gate::NeedsLink);
    };
    // without a bot token nobody can say who is in the server
    if !settings.subdomains_require_member || !settings.can_check() {
        return Ok(Gate::Open);
    }

    let recorded: Option<bool> = sqlx::query_scalar(
        "SELECT in_guild FROM discord_members WHERE discord_id = $1 AND checked IS NOT NULL",
    )
    .bind(&discord_id)
    .fetch_optional(state.database.read())
    .await?;
    match recorded {
        Some(true) => return Ok(Gate::Open),
        // the page: an account never checked is asked about when it creates one
        None if !live => return Ok(Gate::Open),
        Some(false) if !live => return Ok(Gate::NeedsMember),
        _ => {}
    }

    let Some(rest) = context.rest(state) else {
        return Ok(Gate::Open);
    };
    match rest.member(&settings.guild_id, &discord_id).await {
        rest::Lookup::Found(member) => {
            members::record(
                state,
                &context,
                &discord_id,
                Some(owner_uuid),
                Some(&member),
            )
            .await?;
            Ok(Gate::Open)
        }
        rest::Lookup::Absent => {
            members::record(state, &context, &discord_id, Some(owner_uuid), None).await?;
            Ok(Gate::NeedsMember)
        }
        rest::Lookup::Unknown => Ok(Gate::NeedsMember),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_carry_their_creation_time() {
        // 175928847299117063 is Discord's documented example: 2016-04-30 11:18:25.796 UTC
        let created = created_at("175928847299117063").unwrap();
        assert_eq!(created.timestamp_millis(), 1_462_015_105_796);
        assert!(created_at("not a number").is_none());
    }

    #[test]
    fn administrator_is_bit_three() {
        assert!(is_administrator(Some("8")));
        assert!(is_administrator(Some("2147483656")));
        // wider than any integer type, still decided by the last digits
        assert!(is_administrator(Some(
            "99999999999999999999999999999999999999999999990008"
        )));
        assert!(!is_administrator(Some("2147483648")));
        assert!(!is_administrator(Some("")));
        assert!(!is_administrator(Some("-8")));
        assert!(!is_administrator(None));
    }

    #[test]
    fn avatars() {
        assert_eq!(
            avatar_url("1", Some("abc123")).as_deref(),
            Some("https://cdn.discordapp.com/avatars/1/abc123.png?size=128")
        );
        assert_eq!(
            avatar_url("1", Some("https://example.com/a.png")).as_deref(),
            Some("https://example.com/a.png")
        );
        assert_eq!(avatar_url("1", Some("../x")), None);
        assert_eq!(avatar_url("1", None), None);
    }
}
