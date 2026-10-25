// plugged.host: the Discord settings section (docs/features/discord.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::allowance::Resources;
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The Discord server, the rewards for inviting and boosting, the Discord tier and the bots
/// (shared/src/discord).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsDiscord {
    /// Players see the Discord rewards page, and the rest of this section applies.
    pub enabled: bool,
    /// The Discord server (guild) id the panel checks membership in.
    pub guild_id: compact_str::CompactString,
    /// The bot that reads the server's members and invites (Server Members Intent on). Stored
    /// encrypted, shown as a placeholder.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub bot_token: Option<compact_str::CompactString>,
    /// The invite players are sent to; empty hides every invite link and button.
    pub invite_url: compact_str::CompactString,
    /// The OAuth provider (Admin › OAuth Providers) that signs in with Discord: a player's
    /// Discord account is the one linked through it. Unused while noprofit.space is connected.
    pub oauth_provider_uuid: Option<uuid::Uuid>,
    /// What being in the server adds to the allowance without noprofit.space: CPU in % (100 =
    /// one thread), MiB. Only added where the allowance has a limit.
    pub tier_cpu: i32,
    pub tier_memory: i32,
    pub tier_disk: i32,
    /// Credits the inviter gets for each member who joins through their invite and stays.
    pub credits_per_invite: i32,
    /// Discord accounts younger than this when they join pay nothing (alts).
    pub min_account_age_days: i32,
    /// Hours an invite reward is held; a member who leaves before then pays nothing.
    pub invite_maturity_hours: i32,
    /// A paid invite whose member leaves later is taken back (what the balance cannot cover
    /// comes off the inviter's next invite rewards).
    pub invite_clawback: bool,
    /// Credits for boosting the server once, and in all for boosting it twice.
    pub boost_one_credits: i32,
    pub boost_two_credits: i32,
    /// A role that marks two boosts (Discord does not say how often a member boosts); the
    /// `/setboost` command gives and takes it. Empty: two boosts only through `/setboost`.
    pub boost_two_role_id: compact_str::CompactString,
    /// A server's owner needs a linked Discord account for subdomains.
    pub subdomains_require_link: bool,
    /// A server's owner needs to be in the Discord server for subdomains.
    pub subdomains_require_member: bool,
    /// The invite and boost bot (`/invites`, `/topinvites`, `/setboost`, joins and leaves), on
    /// the bot token above.
    pub rewards_bot_enabled: bool,
    /// The assistant bot, on its own token: neo on Discord (`crate::neo::discord`).
    pub assistant_bot_enabled: bool,
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub assistant_bot_token: Option<compact_str::CompactString>,
    /// The moderation bot, on its own token: rux (`crate::rux`).
    pub moderation_bot_enabled: bool,
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub moderation_bot_token: Option<compact_str::CompactString>,
}

impl Default for AppSettingsDiscord {
    fn default() -> Self {
        Self {
            enabled: false,
            guild_id: compact_str::CompactString::default(),
            bot_token: None,
            invite_url: compact_str::CompactString::default(),
            oauth_provider_uuid: None,
            tier_cpu: 0,
            tier_memory: 0,
            tier_disk: 0,
            credits_per_invite: 50,
            min_account_age_days: 30,
            invite_maturity_hours: 6,
            invite_clawback: true,
            boost_one_credits: 500,
            boost_two_credits: 1250,
            boost_two_role_id: compact_str::CompactString::default(),
            subdomains_require_link: false,
            subdomains_require_member: false,
            rewards_bot_enabled: false,
            assistant_bot_enabled: false,
            assistant_bot_token: None,
            moderation_bot_enabled: false,
            moderation_bot_token: None,
        }
    }
}

impl AppSettingsDiscord {
    /// Whether the section is on and names a server.
    pub fn active(&self) -> bool {
        self.enabled && !self.guild_id.is_empty()
    }

    /// Whether the panel can ask Discord about the server's members.
    pub fn can_check(&self) -> bool {
        self.active() && self.bot_token.is_some()
    }

    /// Whether the panel links, checks and pays itself: no noprofit.space, a bot token and the
    /// Discord OAuth provider.
    pub fn standalone(&self, noprofit_connected: bool) -> bool {
        !noprofit_connected && self.can_check() && self.oauth_provider_uuid.is_some()
    }

    /// What being in the server adds to the allowance.
    pub fn tier(&self) -> Resources {
        Resources::new(
            self.tier_cpu.max(0) as i64,
            self.tier_memory.max(0) as i64,
            self.tier_disk.max(0) as i64,
        )
    }

    /// Credits reaching boost `level` (1 or 2) pays: the first level its amount, the second
    /// what is left of the two-boost total.
    pub fn boost_credits(&self, level: i16) -> i64 {
        let one = self.boost_one_credits.max(0) as i64;
        match level {
            1 => one,
            2 => (self.boost_two_credits.max(0) as i64 - one).max(0),
            _ => 0,
        }
    }
}

/// The Discord tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadDiscord {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(length(chars, max = 32)))]
    guild_id: Option<compact_str::CompactString>,
    /// An empty string or the placeholder keeps the stored token, `null` removes it.
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    bot_token: Option<Option<compact_str::CompactString>>,
    #[garde(inner(length(chars, max = 255)))]
    invite_url: Option<compact_str::CompactString>,
    #[garde(skip)]
    #[serde(default, with = "::serde_with::rust::double_option")]
    oauth_provider_uuid: Option<Option<uuid::Uuid>>,
    #[garde(inner(range(min = 0, max = 3200)))]
    tier_cpu: Option<i32>,
    #[garde(inner(range(min = 0, max = 262144)))]
    tier_memory: Option<i32>,
    #[garde(inner(range(min = 0, max = 1048576)))]
    tier_disk: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    credits_per_invite: Option<i32>,
    #[garde(inner(range(min = 0, max = 3650)))]
    min_account_age_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 720)))]
    invite_maturity_hours: Option<i32>,
    #[garde(skip)]
    invite_clawback: Option<bool>,
    #[garde(inner(range(min = 0, max = 10000000)))]
    boost_one_credits: Option<i32>,
    #[garde(inner(range(min = 0, max = 10000000)))]
    boost_two_credits: Option<i32>,
    #[garde(inner(length(chars, max = 32)))]
    boost_two_role_id: Option<compact_str::CompactString>,
    #[garde(skip)]
    subdomains_require_link: Option<bool>,
    #[garde(skip)]
    subdomains_require_member: Option<bool>,
    #[garde(skip)]
    rewards_bot_enabled: Option<bool>,
    #[garde(skip)]
    assistant_bot_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    assistant_bot_token: Option<Option<compact_str::CompactString>>,
    #[garde(skip)]
    moderation_bot_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    moderation_bot_token: Option<Option<compact_str::CompactString>>,
}

/// Why a Discord payload was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PayloadError {
    GuildId,
    RoleId,
    InviteUrl,
}

impl PayloadError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::GuildId => "the server id must be a Discord id (digits only)",
            Self::RoleId => "the role id must be a Discord id (digits only)",
            Self::InviteUrl => "the invite link must start with https://",
        }
    }
}

/// Whether `id` is empty or looks like a Discord id (a snowflake: 15 to 20 digits).
pub fn snowflake_or_empty(id: &str) -> bool {
    id.is_empty() || ((15..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit()))
}

/// A stored secret the payload changes: an empty value or the placeholder keeps it, `null`
/// removes it.
fn set_secret(
    field: &mut Option<compact_str::CompactString>,
    value: Option<Option<compact_str::CompactString>>,
) {
    match value {
        Some(Some(token))
            if token.trim().is_empty() || token == crate::censor::CENSORED_PLACEHOLDER => {}
        Some(token) => *field = token.map(|token| token.trim().into()),
        None => {}
    }
}

impl PayloadDiscord {
    pub fn apply(self, settings: &mut AppSettingsDiscord) -> Result<(), PayloadError> {
        let guild_id = self.guild_id.as_ref().map(|id| id.trim().to_string());
        if let Some(id) = &guild_id
            && !snowflake_or_empty(id)
        {
            return Err(PayloadError::GuildId);
        }
        let role_id = self
            .boost_two_role_id
            .as_ref()
            .map(|id| id.trim().to_string());
        if let Some(id) = &role_id
            && !snowflake_or_empty(id)
        {
            return Err(PayloadError::RoleId);
        }
        let invite_url = self.invite_url.as_ref().map(|url| url.trim().to_string());
        if let Some(url) = &invite_url
            && !url.is_empty()
            && (!url.starts_with("https://") || url.contains(char::is_whitespace))
        {
            return Err(PayloadError::InviteUrl);
        }

        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            enabled,
            tier_cpu,
            tier_memory,
            tier_disk,
            credits_per_invite,
            min_account_age_days,
            invite_maturity_hours,
            invite_clawback,
            boost_one_credits,
            boost_two_credits,
            subdomains_require_link,
            subdomains_require_member,
            rewards_bot_enabled,
            assistant_bot_enabled,
            moderation_bot_enabled
        );

        if let Some(id) = guild_id {
            settings.guild_id = id.into();
        }
        if let Some(id) = role_id {
            settings.boost_two_role_id = id.into();
        }
        if let Some(url) = invite_url {
            settings.invite_url = url.into();
        }
        if let Some(uuid) = self.oauth_provider_uuid {
            settings.oauth_provider_uuid = uuid;
        }
        set_secret(&mut settings.bot_token, self.bot_token);
        set_secret(&mut settings.assistant_bot_token, self.assistant_bot_token);
        set_secret(
            &mut settings.moderation_bot_token,
            self.moderation_bot_token,
        );

        Ok(())
    }
}

const SECRETS: [&str; 3] = ["bot_token", "assistant_bot_token", "moderation_bot_token"];

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsDiscord {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let mut serializer = serializer;
        for (key, value) in SECRETS.into_iter().zip([
            &self.bot_token,
            &self.assistant_bot_token,
            &self.moderation_bot_token,
        ]) {
            serializer = match value {
                Some(value) => {
                    serializer
                        .write_raw_encrypted_setting(key, value.clone())
                        .await?
                }
                None => serializer.write_raw_setting(key, ""),
            };
        }

        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("guild_id", &*self.guild_id)
            .write_raw_setting("invite_url", &*self.invite_url)
            .write_raw_setting(
                "oauth_provider_uuid",
                self.oauth_provider_uuid
                    .map(|uuid| uuid.to_compact_string())
                    .unwrap_or_default(),
            )
            .write_raw_setting("tier_cpu", self.tier_cpu.to_compact_string())
            .write_raw_setting("tier_memory", self.tier_memory.to_compact_string())
            .write_raw_setting("tier_disk", self.tier_disk.to_compact_string())
            .write_raw_setting(
                "credits_per_invite",
                self.credits_per_invite.to_compact_string(),
            )
            .write_raw_setting(
                "min_account_age_days",
                self.min_account_age_days.to_compact_string(),
            )
            .write_raw_setting(
                "invite_maturity_hours",
                self.invite_maturity_hours.to_compact_string(),
            )
            .write_raw_setting("invite_clawback", self.invite_clawback.to_compact_string())
            .write_raw_setting(
                "boost_one_credits",
                self.boost_one_credits.to_compact_string(),
            )
            .write_raw_setting(
                "boost_two_credits",
                self.boost_two_credits.to_compact_string(),
            )
            .write_raw_setting("boost_two_role_id", &*self.boost_two_role_id)
            .write_raw_setting(
                "subdomains_require_link",
                self.subdomains_require_link.to_compact_string(),
            )
            .write_raw_setting(
                "subdomains_require_member",
                self.subdomains_require_member.to_compact_string(),
            )
            .write_raw_setting(
                "rewards_bot_enabled",
                self.rewards_bot_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "assistant_bot_enabled",
                self.assistant_bot_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "moderation_bot_enabled",
                self.moderation_bot_enabled.to_compact_string(),
            ))
    }
}

pub struct AppSettingsDiscordDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsDiscordDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsDiscord::default();

        let mut secrets = Vec::with_capacity(SECRETS.len());
        for key in SECRETS {
            secrets.push(match deserializer.take_raw_setting(key) {
                Some(value) if !value.is_empty() => {
                    Some(deserializer.database.decrypt_base64(value).await?)
                }
                _ => None,
            });
        }
        let mut secrets = secrets.into_iter();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }
        macro_rules! text {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .unwrap_or_default()
            };
        }

        Ok(Box::new(AppSettingsDiscord {
            bot_token: secrets.next().flatten(),
            assistant_bot_token: secrets.next().flatten(),
            moderation_bot_token: secrets.next().flatten(),
            enabled: read!(enabled),
            guild_id: text!(guild_id),
            invite_url: text!(invite_url),
            oauth_provider_uuid: deserializer
                .take_raw_setting("oauth_provider_uuid")
                .filter(|value| !value.is_empty())
                .and_then(|uuid| uuid.parse().ok()),
            tier_cpu: read!(tier_cpu),
            tier_memory: read!(tier_memory),
            tier_disk: read!(tier_disk),
            credits_per_invite: read!(credits_per_invite),
            min_account_age_days: read!(min_account_age_days),
            invite_maturity_hours: read!(invite_maturity_hours),
            invite_clawback: read!(invite_clawback),
            boost_one_credits: read!(boost_one_credits),
            boost_two_credits: read!(boost_two_credits),
            boost_two_role_id: text!(boost_two_role_id),
            subdomains_require_link: read!(subdomains_require_link),
            subdomains_require_member: read!(subdomains_require_member),
            rewards_bot_enabled: read!(rewards_bot_enabled),
            assistant_bot_enabled: read!(assistant_bot_enabled),
            moderation_bot_enabled: read!(moderation_bot_enabled),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> PayloadDiscord {
        PayloadDiscord {
            enabled: None,
            guild_id: None,
            bot_token: None,
            invite_url: None,
            oauth_provider_uuid: None,
            tier_cpu: None,
            tier_memory: None,
            tier_disk: None,
            credits_per_invite: None,
            min_account_age_days: None,
            invite_maturity_hours: None,
            invite_clawback: None,
            boost_one_credits: None,
            boost_two_credits: None,
            boost_two_role_id: None,
            subdomains_require_link: None,
            subdomains_require_member: None,
            rewards_bot_enabled: None,
            assistant_bot_enabled: None,
            assistant_bot_token: None,
            moderation_bot_enabled: None,
            moderation_bot_token: None,
        }
    }

    #[test]
    fn the_second_boost_pays_what_is_left_of_the_total() {
        let settings = AppSettingsDiscord::default();

        assert_eq!(settings.boost_credits(1), 500);
        assert_eq!(settings.boost_credits(2), 750);
        assert_eq!(settings.boost_credits(3), 0);

        let settings = AppSettingsDiscord {
            boost_one_credits: 900,
            boost_two_credits: 500,
            ..Default::default()
        };
        assert_eq!(settings.boost_credits(2), 0, "never negative");
    }

    #[test]
    fn ids_are_digits() {
        assert!(snowflake_or_empty(""));
        assert!(snowflake_or_empty("123456789012345678"));
        assert!(!snowflake_or_empty("12345"));
        assert!(!snowflake_or_empty("12345678901234567a"));
    }

    #[test]
    fn tokens_are_kept_unless_replaced_or_removed() {
        let mut settings = AppSettingsDiscord {
            bot_token: Some("stored".into()),
            assistant_bot_token: Some("stored".into()),
            moderation_bot_token: Some("stored".into()),
            ..Default::default()
        };

        let mut change = payload();
        change.bot_token = Some(Some("".into()));
        change.assistant_bot_token = Some(Some(crate::censor::CENSORED_PLACEHOLDER.into()));
        change.moderation_bot_token = Some(None);
        change.apply(&mut settings).unwrap();

        assert_eq!(settings.bot_token.as_deref(), Some("stored"));
        assert_eq!(settings.assistant_bot_token.as_deref(), Some("stored"));
        assert_eq!(settings.moderation_bot_token, None);

        let mut change = payload();
        change.bot_token = Some(Some(" new ".into()));
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.bot_token.as_deref(), Some("new"));
    }

    #[test]
    fn a_payload_with_a_bad_value_changes_nothing() {
        let mut settings = AppSettingsDiscord::default();
        let mut change = payload();
        change.enabled = Some(true);
        change.invite_url = Some("http://example.com/invite".into());

        assert_eq!(change.apply(&mut settings), Err(PayloadError::InviteUrl));
        assert!(!settings.enabled);

        let mut change = payload();
        change.guild_id = Some("not-an-id".into());
        assert_eq!(change.apply(&mut settings), Err(PayloadError::GuildId));
    }
}
