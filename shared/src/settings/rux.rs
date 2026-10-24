// plugged.host: the rux settings section (Admin › Settings › rux, docs/features/rux.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::settings::discord::snowflake_or_empty;
use compact_str::{CompactString, ToCompactString};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Servers rux works on besides the main one.
pub const MAX_SERVERS: usize = 9;
/// The languages rux posts in.
pub const LOCALES: [&str; 2] = ["en", "de"];

/// One more Discord server rux works on, besides the main one.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct RuxServer {
    pub id: CompactString,
    /// Whose server it is, as rux's prompts name it; empty: the panel's name.
    pub name: CompactString,
    /// What that host is, as in "<name> is <about>"; empty: a general sentence.
    pub about: CompactString,
    pub giveaway_channel_id: CompactString,
    pub mod_log_channel_id: CompactString,
    /// Sites the message checks treat as the host's own on this server (with their subdomains).
    pub trusted_domains: Vec<CompactString>,
}

/// rux, the moderation bot on the Discord section's moderation bot token: the servers it works
/// on, what it may give away, the warning ladder, spam and checks. While noprofit.space keeps
/// rux's settings, those apply and these only show (shared/src/rux/config.rs).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsRux {
    /// The bot's name: what members call it by ("hey rux") and what it calls itself.
    pub name: CompactString,
    /// The main Discord server; empty: the Discord section's server. DMs act on it until someone
    /// picks another.
    pub guild_id: CompactString,
    /// The main server's default giveaway channel and its mod log.
    pub giveaway_channel_id: CompactString,
    pub mod_log_channel_id: CompactString,
    /// The host's own sites on the main server.
    pub trusted_domains: Vec<CompactString>,
    /// The other servers.
    pub servers: Vec<RuxServer>,
    /// Every time rux reads or writes is in this zone.
    pub timezone: CompactString,
    /// The language rux posts in (`en`, `de`).
    pub locale: CompactString,
    pub giveaway_max_credits: i64,
    pub giveaway_max_winners: i32,
    pub giveaway_max_days: i32,
    /// Discord ids of the people who may have rux give credits; empty turns it off.
    pub credit_granters: Vec<CompactString>,
    pub credit_max_per_grant: i64,
    /// From this many warnings each new one also times the member out (0: never), for this long.
    pub warn_timeout_at: i32,
    pub warn_timeout: CompactString,
    /// From this many warnings each new one bans the member (0: never), for this long.
    pub warn_ban_at: i32,
    pub warn_ban: CompactString,
    /// Seconds between two actions of one standing rule on the same member.
    pub rule_cooldown_seconds: i32,
    /// Watched channels also enforce the basic guidelines (scams, slurs, harassment, ads).
    pub watch_guidelines: bool,
    /// Spam caught without AI, in every channel: flooding, repeats, mass mentions, emoji walls.
    pub spam_enabled: bool,
    pub spam_flood_messages: i32,
    pub spam_flood_seconds: i32,
    pub spam_duplicates: i32,
    pub spam_duplicate_seconds: i32,
    pub spam_mentions: i32,
    pub spam_emoji: i32,
    /// Orders one person may give a minute.
    pub messages_per_minute: i32,
    /// Model calls one order may take.
    pub max_steps: i32,
    /// How far back an order remembers the conversation.
    pub history_minutes: i32,
    /// The secret noprofit.space's rux routes are signed with; empty: the noprofit.space API
    /// secret. Stored encrypted, shown as a placeholder.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub noprofit_secret: Option<CompactString>,
}

impl Default for AppSettingsRux {
    fn default() -> Self {
        Self {
            name: "rux".into(),
            guild_id: CompactString::default(),
            giveaway_channel_id: CompactString::default(),
            mod_log_channel_id: CompactString::default(),
            trusted_domains: Vec::new(),
            servers: Vec::new(),
            timezone: "UTC".into(),
            locale: "en".into(),
            giveaway_max_credits: 10_000,
            giveaway_max_winners: 25,
            giveaway_max_days: 60,
            credit_granters: Vec::new(),
            credit_max_per_grant: 10_000,
            warn_timeout_at: 3,
            warn_timeout: "24h".into(),
            warn_ban_at: 5,
            warn_ban: "24h".into(),
            rule_cooldown_seconds: 300,
            watch_guidelines: true,
            spam_enabled: true,
            spam_flood_messages: 7,
            spam_flood_seconds: 10,
            spam_duplicates: 3,
            spam_duplicate_seconds: 120,
            spam_mentions: 6,
            spam_emoji: 25,
            messages_per_minute: 6,
            max_steps: 8,
            history_minutes: 30,
            noprofit_secret: None,
        }
    }
}

// by hand, so the secret never reaches a log
impl std::fmt::Debug for AppSettingsRux {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSettingsRux")
            .field("name", &self.name)
            .field("guild_id", &self.guild_id)
            .field("servers", &self.servers)
            .field("timezone", &self.timezone)
            .field("locale", &self.locale)
            .field(
                "noprofit_secret",
                &self.noprofit_secret.as_ref().map(|_| "(set)"),
            )
            .finish_non_exhaustive()
    }
}

/// Whether `domain` looks like a host name with a dot (`example.com`), lowercase or not.
pub fn domain_valid(domain: &str) -> bool {
    let Some((head, tld)) = domain.rsplit_once('.') else {
        return false;
    };

    domain.len() <= 253
        && !head.is_empty()
        && tld.len() >= 2
        && tld.bytes().all(|b| b.is_ascii_alphabetic())
        && head
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && !head.starts_with(['.', '-'])
        && !head.contains("..")
}

/// A list of domains as people type them: trimmed, lowercased, `https://` and paths dropped,
/// empty entries and repeats left out.
pub fn clean_domains(domains: &[CompactString]) -> Vec<CompactString> {
    let mut out: Vec<CompactString> = Vec::new();
    for domain in domains {
        let domain = domain.trim().to_ascii_lowercase();
        let domain = domain
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default()
            .trim_start_matches("www.")
            .trim_end_matches('.');
        if !domain.is_empty() && !out.iter().any(|d| d == domain) {
            out.push(domain.into());
        }
    }

    out
}

/// Whether `zone` is a time zone name rux can work in (`Europe/Paris`, `UTC`).
pub fn timezone_valid(zone: &str) -> bool {
    zone.parse::<chrono_tz::Tz>().is_ok()
}

/// One more server in the rux tab.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadRuxServer {
    #[garde(length(chars, max = 20))]
    id: CompactString,
    #[garde(length(chars, max = 64))]
    #[serde(default)]
    name: CompactString,
    #[garde(length(chars, max = 255))]
    #[serde(default)]
    about: CompactString,
    #[garde(length(chars, max = 20))]
    #[serde(default)]
    giveaway_channel_id: CompactString,
    #[garde(length(chars, max = 20))]
    #[serde(default)]
    mod_log_channel_id: CompactString,
    #[garde(length(max = 20), inner(length(chars, max = 253)))]
    #[serde(default)]
    trusted_domains: Vec<CompactString>,
}

/// The rux tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadRux {
    #[garde(inner(length(chars, min = 1, max = 32)))]
    name: Option<CompactString>,
    #[garde(inner(length(chars, max = 20)))]
    guild_id: Option<CompactString>,
    #[garde(inner(length(chars, max = 20)))]
    giveaway_channel_id: Option<CompactString>,
    #[garde(inner(length(chars, max = 20)))]
    mod_log_channel_id: Option<CompactString>,
    #[garde(inner(length(max = 20), inner(length(chars, max = 253))))]
    trusted_domains: Option<Vec<CompactString>>,
    #[garde(length(max = MAX_SERVERS), dive)]
    servers: Option<Vec<PayloadRuxServer>>,
    #[garde(inner(length(chars, max = 64)))]
    timezone: Option<CompactString>,
    #[garde(inner(length(chars, max = 8)))]
    locale: Option<CompactString>,
    #[garde(inner(range(min = 1, max = 1_000_000)))]
    giveaway_max_credits: Option<i64>,
    #[garde(inner(range(min = 1, max = 500)))]
    giveaway_max_winners: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    giveaway_max_days: Option<i32>,
    #[garde(inner(length(max = 20), inner(length(chars, max = 20))))]
    credit_granters: Option<Vec<CompactString>>,
    #[garde(inner(range(min = 1, max = 1_000_000)))]
    credit_max_per_grant: Option<i64>,
    #[garde(inner(range(min = 0, max = 100)))]
    warn_timeout_at: Option<i32>,
    #[garde(inner(length(chars, max = 16)))]
    warn_timeout: Option<CompactString>,
    #[garde(inner(range(min = 0, max = 100)))]
    warn_ban_at: Option<i32>,
    #[garde(inner(length(chars, max = 16)))]
    warn_ban: Option<CompactString>,
    #[garde(inner(range(min = 0, max = 86400)))]
    rule_cooldown_seconds: Option<i32>,
    #[garde(skip)]
    watch_guidelines: Option<bool>,
    #[garde(skip)]
    spam_enabled: Option<bool>,
    #[garde(inner(range(min = 2, max = 100)))]
    spam_flood_messages: Option<i32>,
    #[garde(inner(range(min = 1, max = 600)))]
    spam_flood_seconds: Option<i32>,
    #[garde(inner(range(min = 2, max = 100)))]
    spam_duplicates: Option<i32>,
    #[garde(inner(range(min = 1, max = 3600)))]
    spam_duplicate_seconds: Option<i32>,
    #[garde(inner(range(min = 2, max = 100)))]
    spam_mentions: Option<i32>,
    #[garde(inner(range(min = 2, max = 500)))]
    spam_emoji: Option<i32>,
    #[garde(inner(range(min = 1, max = 120)))]
    messages_per_minute: Option<i32>,
    #[garde(inner(range(min = 2, max = 30)))]
    max_steps: Option<i32>,
    #[garde(inner(range(min = 1, max = 1440)))]
    history_minutes: Option<i32>,
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    noprofit_secret: Option<Option<CompactString>>,
}

/// Why a rux payload was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PayloadError {
    Name,
    Id(&'static str),
    ServerId(usize),
    ServerChannel(usize),
    DuplicateServer(usize),
    Domain(String),
    Timezone,
    Locale,
    Duration(&'static str),
    Granter,
    Secret,
}

impl PayloadError {
    pub fn message(&self) -> String {
        match self {
            Self::Name => "the name may only hold letters, digits, dashes and underscores".into(),
            Self::Id(field) => format!("{field} must be a Discord id (digits only)"),
            Self::ServerId(index) => format!("server {}: the id must be a Discord id", index + 1),
            Self::ServerChannel(index) => {
                format!("server {}: channel ids must be Discord ids", index + 1)
            }
            Self::DuplicateServer(index) => format!(
                "server {} is listed twice (or is the main server)",
                index + 1
            ),
            Self::Domain(domain) => format!("\"{domain}\" is not a domain like example.com"),
            Self::Timezone => "unknown time zone (use a name like Europe/Paris)".into(),
            Self::Locale => format!("the language must be one of {}", LOCALES.join(", ")),
            Self::Duration(field) => {
                format!("{field} must be a duration like 24h or 7d (at most 28 days for timeouts)")
            }
            Self::Granter => "the credit granters must be Discord user ids".into(),
            Self::Secret => format!(
                "the noprofit.space secret must be at least {MIN_SECRET_CHARS} characters, like noprofit.space's own"
            ),
        }
    }
}

/// Seconds Discord lets a timeout last.
const MAX_TIMEOUT_SECONDS: i64 = 28 * 86400;
/// noprofit.space refuses shorter secrets (and rux does not sign with them).
pub const MIN_SECRET_CHARS: usize = 32;

fn snowflake(
    field: &'static str,
    value: Option<CompactString>,
) -> Result<Option<CompactString>, PayloadError> {
    match value {
        Some(value) => {
            let value = value.trim();
            if !snowflake_or_empty(value) {
                return Err(PayloadError::Id(field));
            }
            Ok(Some(value.into()))
        }
        None => Ok(None),
    }
}

fn domains(value: Option<Vec<CompactString>>) -> Result<Option<Vec<CompactString>>, PayloadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let cleaned = clean_domains(&value);
    if let Some(bad) = cleaned.iter().find(|domain| !domain_valid(domain)) {
        return Err(PayloadError::Domain(bad.to_string()));
    }

    Ok(Some(cleaned))
}

impl PayloadRux {
    pub fn apply(self, settings: &mut AppSettingsRux) -> Result<(), PayloadError> {
        let name = match self.name {
            Some(name) => {
                let name = name.trim();
                if name.is_empty()
                    || !name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Err(PayloadError::Name);
                }
                Some(CompactString::from(name))
            }
            None => None,
        };
        let guild_id = snowflake("the main server id", self.guild_id)?;
        let giveaway_channel_id = snowflake("the giveaway channel id", self.giveaway_channel_id)?;
        let mod_log_channel_id = snowflake("the mod log channel id", self.mod_log_channel_id)?;
        let trusted_domains = domains(self.trusted_domains)?;

        let main = guild_id
            .clone()
            .unwrap_or_else(|| settings.guild_id.clone());
        let servers = match self.servers {
            Some(payload) => {
                let mut servers: Vec<RuxServer> = Vec::with_capacity(payload.len());
                for (index, server) in payload.into_iter().enumerate() {
                    let id = server.id.trim();
                    if id.is_empty() || !snowflake_or_empty(id) {
                        return Err(PayloadError::ServerId(index));
                    }
                    if id == main || servers.iter().any(|s| s.id == id) {
                        return Err(PayloadError::DuplicateServer(index));
                    }
                    let giveaway = server.giveaway_channel_id.trim();
                    let mod_log = server.mod_log_channel_id.trim();
                    if !snowflake_or_empty(giveaway) || !snowflake_or_empty(mod_log) {
                        return Err(PayloadError::ServerChannel(index));
                    }

                    servers.push(RuxServer {
                        id: id.into(),
                        name: server.name.trim().into(),
                        about: server.about.trim().into(),
                        giveaway_channel_id: giveaway.into(),
                        mod_log_channel_id: mod_log.into(),
                        trusted_domains: domains(Some(server.trusted_domains))?.unwrap_or_default(),
                    });
                }
                Some(servers)
            }
            None => None,
        };

        let timezone = match self.timezone {
            Some(zone) => {
                let zone = zone.trim();
                if !timezone_valid(zone) {
                    return Err(PayloadError::Timezone);
                }
                Some(CompactString::from(zone))
            }
            None => None,
        };
        let locale = match self.locale {
            Some(locale) => {
                let locale = locale.trim().to_ascii_lowercase();
                if !LOCALES.contains(&locale.as_str()) {
                    return Err(PayloadError::Locale);
                }
                Some(CompactString::from(locale))
            }
            None => None,
        };
        let duration =
            |field: &'static str, value: Option<CompactString>, max: Option<i64>| match value {
                Some(value) => {
                    let value = value.trim().to_ascii_lowercase();
                    match crate::rux::time::duration_seconds(&value) {
                        Some(seconds) if seconds > 0 && max.is_none_or(|max| seconds <= max) => {
                            Ok(Some(CompactString::from(value)))
                        }
                        _ => Err(PayloadError::Duration(field)),
                    }
                }
                None => Ok(None),
            };
        let warn_timeout = duration(
            "the timeout length",
            self.warn_timeout,
            Some(MAX_TIMEOUT_SECONDS),
        )?;
        let warn_ban = duration("the ban length", self.warn_ban, None)?;
        let credit_granters = match self.credit_granters {
            Some(granters) => {
                let mut out: Vec<CompactString> = Vec::new();
                for granter in granters {
                    let granter = granter.trim();
                    if granter.is_empty() {
                        continue;
                    }
                    if !snowflake_or_empty(granter) {
                        return Err(PayloadError::Granter);
                    }
                    if !out.iter().any(|g| g == granter) {
                        out.push(granter.into());
                    }
                }
                Some(out)
            }
            None => None,
        };

        let noprofit_secret = match self.noprofit_secret {
            Some(Some(secret))
                if secret.trim().is_empty() || secret == crate::censor::CENSORED_PLACEHOLDER =>
            {
                None
            }
            Some(Some(secret)) if secret.trim().chars().count() < MIN_SECRET_CHARS => {
                return Err(PayloadError::Secret);
            }
            Some(secret) => Some(secret.map(|secret| CompactString::from(secret.trim()))),
            None => None,
        };

        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            giveaway_max_credits,
            giveaway_max_winners,
            giveaway_max_days,
            credit_max_per_grant,
            warn_timeout_at,
            warn_ban_at,
            rule_cooldown_seconds,
            watch_guidelines,
            spam_enabled,
            spam_flood_messages,
            spam_flood_seconds,
            spam_duplicates,
            spam_duplicate_seconds,
            spam_mentions,
            spam_emoji,
            messages_per_minute,
            max_steps,
            history_minutes
        );
        macro_rules! set_checked {
            ($($field:ident),*) => {
                $(if let Some(value) = $field {
                    settings.$field = value;
                })*
            };
        }
        set_checked!(
            name,
            guild_id,
            giveaway_channel_id,
            mod_log_channel_id,
            trusted_domains,
            servers,
            timezone,
            locale,
            warn_timeout,
            warn_ban,
            credit_granters
        );

        if let Some(secret) = noprofit_secret {
            settings.noprofit_secret = secret;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsRux {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let serializer = match &self.noprofit_secret {
            Some(secret) => {
                serializer
                    .write_raw_encrypted_setting("noprofit_secret", secret.clone())
                    .await?
            }
            None => serializer.write_raw_setting("noprofit_secret", ""),
        };

        Ok(serializer
            .write_raw_setting("name", &*self.name)
            .write_raw_setting("guild_id", &*self.guild_id)
            .write_raw_setting("giveaway_channel_id", &*self.giveaway_channel_id)
            .write_raw_setting("mod_log_channel_id", &*self.mod_log_channel_id)
            .write_serde_setting("trusted_domains", &self.trusted_domains)?
            .write_serde_setting("servers", &self.servers)?
            .write_raw_setting("timezone", &*self.timezone)
            .write_raw_setting("locale", &*self.locale)
            .write_raw_setting(
                "giveaway_max_credits",
                self.giveaway_max_credits.to_compact_string(),
            )
            .write_raw_setting(
                "giveaway_max_winners",
                self.giveaway_max_winners.to_compact_string(),
            )
            .write_raw_setting(
                "giveaway_max_days",
                self.giveaway_max_days.to_compact_string(),
            )
            .write_serde_setting("credit_granters", &self.credit_granters)?
            .write_raw_setting(
                "credit_max_per_grant",
                self.credit_max_per_grant.to_compact_string(),
            )
            .write_raw_setting("warn_timeout_at", self.warn_timeout_at.to_compact_string())
            .write_raw_setting("warn_timeout", &*self.warn_timeout)
            .write_raw_setting("warn_ban_at", self.warn_ban_at.to_compact_string())
            .write_raw_setting("warn_ban", &*self.warn_ban)
            .write_raw_setting(
                "rule_cooldown_seconds",
                self.rule_cooldown_seconds.to_compact_string(),
            )
            .write_raw_setting(
                "watch_guidelines",
                self.watch_guidelines.to_compact_string(),
            )
            .write_raw_setting("spam_enabled", self.spam_enabled.to_compact_string())
            .write_raw_setting(
                "spam_flood_messages",
                self.spam_flood_messages.to_compact_string(),
            )
            .write_raw_setting(
                "spam_flood_seconds",
                self.spam_flood_seconds.to_compact_string(),
            )
            .write_raw_setting("spam_duplicates", self.spam_duplicates.to_compact_string())
            .write_raw_setting(
                "spam_duplicate_seconds",
                self.spam_duplicate_seconds.to_compact_string(),
            )
            .write_raw_setting("spam_mentions", self.spam_mentions.to_compact_string())
            .write_raw_setting("spam_emoji", self.spam_emoji.to_compact_string())
            .write_raw_setting(
                "messages_per_minute",
                self.messages_per_minute.to_compact_string(),
            )
            .write_raw_setting("max_steps", self.max_steps.to_compact_string())
            .write_raw_setting("history_minutes", self.history_minutes.to_compact_string()))
    }
}

pub struct AppSettingsRuxDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsRuxDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsRux::default();

        let noprofit_secret = match deserializer.take_raw_setting("noprofit_secret") {
            Some(value) if !value.is_empty() => {
                Some(deserializer.database.decrypt_base64(value).await?)
            }
            _ => None,
        };

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
        macro_rules! text_or_default {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| defaults.$field.clone())
            };
        }
        macro_rules! json {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default()
            };
        }

        Ok(Box::new(AppSettingsRux {
            name: text_or_default!(name),
            guild_id: text!(guild_id),
            giveaway_channel_id: text!(giveaway_channel_id),
            mod_log_channel_id: text!(mod_log_channel_id),
            trusted_domains: json!(trusted_domains),
            servers: json!(servers),
            timezone: text_or_default!(timezone),
            locale: text_or_default!(locale),
            giveaway_max_credits: read!(giveaway_max_credits),
            giveaway_max_winners: read!(giveaway_max_winners),
            giveaway_max_days: read!(giveaway_max_days),
            credit_granters: json!(credit_granters),
            credit_max_per_grant: read!(credit_max_per_grant),
            warn_timeout_at: read!(warn_timeout_at),
            warn_timeout: text_or_default!(warn_timeout),
            warn_ban_at: read!(warn_ban_at),
            warn_ban: text_or_default!(warn_ban),
            rule_cooldown_seconds: read!(rule_cooldown_seconds),
            watch_guidelines: read!(watch_guidelines),
            spam_enabled: read!(spam_enabled),
            spam_flood_messages: read!(spam_flood_messages),
            spam_flood_seconds: read!(spam_flood_seconds),
            spam_duplicates: read!(spam_duplicates),
            spam_duplicate_seconds: read!(spam_duplicate_seconds),
            spam_mentions: read!(spam_mentions),
            spam_emoji: read!(spam_emoji),
            messages_per_minute: read!(messages_per_minute),
            max_steps: read!(max_steps),
            history_minutes: read!(history_minutes),
            noprofit_secret,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = "100000000000000001";
    const OTHER: &str = "100000000000000002";

    fn payload() -> PayloadRux {
        PayloadRux {
            name: None,
            guild_id: None,
            giveaway_channel_id: None,
            mod_log_channel_id: None,
            trusted_domains: None,
            servers: None,
            timezone: None,
            locale: None,
            giveaway_max_credits: None,
            giveaway_max_winners: None,
            giveaway_max_days: None,
            credit_granters: None,
            credit_max_per_grant: None,
            warn_timeout_at: None,
            warn_timeout: None,
            warn_ban_at: None,
            warn_ban: None,
            rule_cooldown_seconds: None,
            watch_guidelines: None,
            spam_enabled: None,
            spam_flood_messages: None,
            spam_flood_seconds: None,
            spam_duplicates: None,
            spam_duplicate_seconds: None,
            spam_mentions: None,
            spam_emoji: None,
            messages_per_minute: None,
            max_steps: None,
            history_minutes: None,
            noprofit_secret: None,
        }
    }

    fn server(id: &str) -> PayloadRuxServer {
        PayloadRuxServer {
            id: id.into(),
            name: " Example ".into(),
            about: "".into(),
            giveaway_channel_id: "".into(),
            mod_log_channel_id: "".into(),
            trusted_domains: vec!["https://Example.com/path".into()],
        }
    }

    #[test]
    fn ships_neutral_and_off_by_default() {
        let settings = AppSettingsRux::default();
        assert_eq!(settings.name, "rux");
        assert_eq!(settings.timezone, "UTC");
        assert!(settings.guild_id.is_empty());
        assert!(settings.credit_granters.is_empty());
        assert!(settings.noprofit_secret.is_none());
    }

    #[test]
    fn checks_ids_zones_and_durations() {
        let mut settings = AppSettingsRux::default();

        let mut change = payload();
        change.guild_id = Some("not an id".into());
        assert_eq!(
            change.apply(&mut settings),
            Err(PayloadError::Id("the main server id"))
        );

        let mut change = payload();
        change.timezone = Some("Mars/Olympus".into());
        assert_eq!(change.apply(&mut settings), Err(PayloadError::Timezone));

        let mut change = payload();
        change.warn_timeout = Some("30d".into());
        assert_eq!(
            change.apply(&mut settings),
            Err(PayloadError::Duration("the timeout length"))
        );

        let mut change = payload();
        change.locale = Some("fr".into());
        assert_eq!(change.apply(&mut settings), Err(PayloadError::Locale));

        let mut change = payload();
        change.name = Some("rux bot".into());
        assert_eq!(change.apply(&mut settings), Err(PayloadError::Name));

        let mut change = payload();
        change.guild_id = Some(format!(" {MAIN} ").into());
        change.timezone = Some("Europe/Paris".into());
        change.warn_ban = Some("7D".into());
        change.locale = Some("DE".into());
        change.credit_granters = Some(vec![OTHER.into(), "".into(), OTHER.into()]);
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.guild_id, MAIN);
        assert_eq!(settings.timezone, "Europe/Paris");
        assert_eq!(settings.warn_ban, "7d");
        assert_eq!(settings.locale, "de");
        assert_eq!(settings.credit_granters, vec![CompactString::from(OTHER)]);
    }

    #[test]
    fn servers_are_unique_and_not_the_main_one() {
        let mut settings = AppSettingsRux {
            guild_id: MAIN.into(),
            ..Default::default()
        };

        let mut change = payload();
        change.servers = Some(vec![server(MAIN)]);
        assert_eq!(
            change.apply(&mut settings),
            Err(PayloadError::DuplicateServer(0))
        );

        let mut change = payload();
        change.servers = Some(vec![server(OTHER), server(OTHER)]);
        assert_eq!(
            change.apply(&mut settings),
            Err(PayloadError::DuplicateServer(1))
        );

        let mut change = payload();
        change.servers = Some(vec![server(OTHER)]);
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.servers.len(), 1);
        assert_eq!(settings.servers[0].name, "Example");
        assert_eq!(
            settings.servers[0].trusted_domains,
            vec![CompactString::from("example.com")]
        );
    }

    #[test]
    fn domains_are_cleaned_and_checked() {
        assert_eq!(
            clean_domains(&["  https://www.Example.org/x ".into(), "example.org".into()]),
            vec![CompactString::from("example.org")]
        );
        assert!(domain_valid("status.example.com"));
        assert!(!domain_valid("localhost"));
        assert!(!domain_valid("exa mple.com"));
        assert!(!domain_valid(".example.com"));

        let mut settings = AppSettingsRux::default();
        let mut change = payload();
        change.trusted_domains = Some(vec!["not a domain".into()]);
        assert!(matches!(
            change.apply(&mut settings),
            Err(PayloadError::Domain(_))
        ));
    }

    #[test]
    fn the_secret_survives_the_placeholder() {
        let mut settings = AppSettingsRux {
            noprofit_secret: Some("stored".into()),
            ..Default::default()
        };

        let mut change = payload();
        change.noprofit_secret = Some(Some(crate::censor::CENSORED_PLACEHOLDER.into()));
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.noprofit_secret.as_deref(), Some("stored"));

        let mut change = payload();
        change.noprofit_secret = Some(Some("too short".into()));
        assert_eq!(change.apply(&mut settings), Err(PayloadError::Secret));
        assert_eq!(settings.noprofit_secret.as_deref(), Some("stored"));

        let mut change = payload();
        change.noprofit_secret = Some(Some(format!(" {} ", "s".repeat(32)).into()));
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.noprofit_secret, Some("s".repeat(32).into()));

        let mut change = payload();
        change.noprofit_secret = Some(None);
        change.apply(&mut settings).unwrap();
        assert_eq!(settings.noprofit_secret, None);
    }
}
