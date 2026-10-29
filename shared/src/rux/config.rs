//! plugged.host: the settings rux runs on: the rux section (Admin › Settings › rux), or, while
//! noprofit.space keeps them, noprofit.space's laid over it ([`super::noprofit::settings`]), and
//! the servers they describe.

use crate::settings::rux::{AppSettingsRux, MAX_SERVERS, RuxServer, clean_domains};
use compact_str::CompactString;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use utoipa::ToSchema;

/// What the bot needs on a server: view channels, send messages (and in threads), embed links,
/// read history, manage messages, pin messages, manage channels, kick, ban, manage roles,
/// manage nicknames and time members out; and manage server, to read the invites when the same
/// bot also counts them.
pub const BOT_PERMISSIONS: &str = "2253174605966390";
/// What a server's host is when its settings do not say.
pub const DEFAULT_ABOUT: &str = "a free game server host";

/// One Discord server rux works on.
#[derive(Clone, Debug, Serialize, ToSchema, PartialEq, Eq)]
pub struct Guild {
    pub id: String,
    /// The main server: DMs act on it until someone picks another.
    pub primary: bool,
    /// Whose server it is, as rux's prompts name it ("the <host> Discord server").
    pub host_name: String,
    /// What that host is ("<host> is <about>").
    pub about: String,
    pub giveaway_channel_id: Option<String>,
    pub mod_log_channel_id: Option<String>,
    /// Sites the checks treat as the host's own here (each with its subdomains).
    pub trusted_domains: Vec<String>,
    /// The checks are told the panel's server locations (the panel's own community only).
    pub locations: bool,
}

impl Guild {
    /// "Example (Discord server 123…)"
    pub fn label(&self) -> String {
        format!("{} (Discord server {})", self.host_name, self.id)
    }
}

/// Where rux's settings are edited when not here.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Managed {
    /// noprofit.space's host, as the page names it.
    pub by: String,
    /// Its rux settings page.
    pub url: Option<String>,
    /// When the copy in use was fetched (unix seconds).
    pub fetched_at: i64,
    pub saved_at: Option<String>,
    pub saved_by: Option<String>,
}

/// plugged.host: the memberships' names (the economy settings'), as giveaways and `/tokens` name
/// them.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MembershipNames {
    pub plus: String,
    pub plusplus: String,
}

impl Default for MembershipNames {
    fn default() -> Self {
        Self {
            plus: "Plus".into(),
            plusplus: "Premium".into(),
        }
    }
}

impl MembershipNames {
    /// The name a giveaway's prize kind gives days of.
    pub fn of(&self, plusplus: bool) -> &str {
        if plusplus { &self.plusplus } else { &self.plus }
    }
}

/// Everything rux runs on, for one message or one tick.
#[derive(Clone, Serialize, ToSchema)]
pub struct Config {
    /// The settings in effect (the panel's, or noprofit.space's over them).
    #[schema(inline)]
    pub settings: AppSettingsRux,
    /// noprofit.space's switch while it keeps the settings; always on otherwise (the Discord
    /// section's moderation bot switch is the panel's).
    pub enabled: bool,
    /// The servers, the main one first; empty without a main server.
    pub guilds: Vec<Guild>,
    pub app_name: String,
    /// Where accounts live, as rux names it to people: noprofit.space's host while it is
    /// connected, else the panel's name.
    pub portal: String,
    /// Where someone links Discord to their account.
    pub link_url: String,
    /// The Discord section's role that marks two boosts (a role that comes with rewards).
    pub boost_two_role_id: String,
    pub managed: Option<Managed>,
    /// The bot token noprofit.space keeps with the settings (the gateway runs on the Discord
    /// section's moderation token; this is only for a lead who wants noprofit.space's).
    #[serde(skip)]
    pub managed_bot_token: Option<String>,
    /// plugged.host: the memberships' names.
    #[serde(skip)]
    pub memberships: MembershipNames,
}

// by hand, so the bot token never reaches a log
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("settings", &self.settings)
            .field("enabled", &self.enabled)
            .field("guilds", &self.guilds)
            .field("app_name", &self.app_name)
            .field("managed", &self.managed)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn primary(&self) -> Option<&Guild> {
        self.guilds
            .iter()
            .find(|guild| guild.primary)
            .or(self.guilds.first())
    }

    /// The server with this id, when rux works on it.
    pub fn guild(&self, id: &str) -> Option<&Guild> {
        if id.is_empty() {
            return None;
        }

        self.guilds.iter().find(|guild| guild.id == id)
    }

    /// Whether rux works on this server.
    pub fn serves(&self, id: Option<&str>) -> bool {
        id.is_some_and(|id| self.guild(id).is_some())
    }

    /// A stored row's server: its own, or the main one for rows without one.
    pub fn guild_for(&self, id: Option<&str>) -> Option<&Guild> {
        match id {
            Some(id) if !id.is_empty() => self.guild(id),
            _ => self.primary(),
        }
    }

    pub fn tz(&self) -> chrono_tz::Tz {
        self.settings.timezone.parse().unwrap_or(chrono_tz::UTC)
    }

    pub fn now(&self) -> chrono::DateTime<chrono_tz::Tz> {
        chrono::Utc::now().with_timezone(&self.tz())
    }

    pub fn locale(&self) -> &str {
        &self.settings.locale
    }

    /// A line rux posts on Discord, in its language, `:portal` and `:name` filled in.
    pub fn say(&self, key: &str, replace: &[(&str, &str)]) -> String {
        let mut all: Vec<(&str, &str)> = replace.to_vec();
        all.push(("portal", &self.portal));
        all.push(("name", &self.settings.name));

        super::say::say(self.locale(), key, &all)
    }

    /// Whether `discord_id` may have credits given (the settings' credit granters).
    pub fn grants_credits(&self, discord_id: &str) -> bool {
        self.settings
            .credit_granters
            .iter()
            .any(|granter| granter == discord_id)
    }

    /// The link that adds the bot to a server with the permissions it needs.
    pub fn invite_url(application_id: &str, guild_id: Option<&str>) -> String {
        let mut url = format!(
            "https://discord.com/oauth2/authorize?client_id={application_id}&scope=bot%20applications.commands&permissions={BOT_PERMISSIONS}"
        );
        if let Some(guild_id) = guild_id.filter(|id| crate::discord::is_snowflake(id)) {
            url.push_str(&format!("&guild_id={guild_id}&disable_guild_select=true"));
        }

        url
    }
}

/// The bot's application (its user id), which is the first part of its token, base64 encoded;
/// `None` when the token does not look like one.
pub fn application_id_of(token: &str) -> Option<String> {
    use base64::Engine;

    let first = token
        .trim()
        .split('.')
        .next()?
        .replace('-', "+")
        .replace('_', "/");
    let decoded = crate::database::BASE64_ENGINE
        .decode(first.trim_end_matches('='))
        .ok()?;
    let id = String::from_utf8(decoded).ok()?;

    crate::discord::is_snowflake(&id).then_some(id)
}

fn snowflake(id: &str) -> Option<String> {
    let id = id.trim();

    crate::discord::is_snowflake(id).then(|| id.to_string())
}

/// One line of free text, at most `max` characters, or `None` when empty.
fn line(text: &str, max: usize) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    (!text.is_empty()).then(|| text.chars().take(max).collect())
}

/// The servers `settings` describe: the main one (its own id, else the Discord section's),
/// named after the panel and told the panel's locations, then the others in order. A server
/// without a valid id, or one listed already, is left out; so is everything without a main one.
/// The main server's trusted domains count on every server.
pub fn guilds(settings: &AppSettingsRux, discord_guild_id: &str, app_name: &str) -> Vec<Guild> {
    let main = if settings.guild_id.trim().is_empty() {
        discord_guild_id
    } else {
        &settings.guild_id
    };
    let Some(main) = snowflake(main) else {
        return Vec::new();
    };
    let shared: Vec<String> = clean_domains(&settings.trusted_domains)
        .into_iter()
        .map(String::from)
        .collect();

    let mut guilds = vec![Guild {
        id: main,
        primary: true,
        host_name: app_name.to_string(),
        about: DEFAULT_ABOUT.to_string(),
        giveaway_channel_id: snowflake(&settings.giveaway_channel_id),
        mod_log_channel_id: snowflake(&settings.mod_log_channel_id),
        trusted_domains: shared.clone(),
        locations: true,
    }];

    for server in settings.servers.iter().take(MAX_SERVERS) {
        let Some(id) = snowflake(&server.id) else {
            continue;
        };
        if guilds.iter().any(|guild| guild.id == id) {
            continue;
        }

        let mut trusted_domains = shared.clone();
        for domain in clean_domains(&server.trusted_domains) {
            if !trusted_domains.iter().any(|d| d == domain.as_str()) {
                trusted_domains.push(domain.into());
            }
        }

        guilds.push(Guild {
            id,
            primary: false,
            host_name: line(&server.name, 60).unwrap_or_else(|| app_name.to_string()),
            about: line(&server.about, 200).unwrap_or_else(|| DEFAULT_ABOUT.to_string()),
            giveaway_channel_id: snowflake(&server.giveaway_channel_id),
            mod_log_channel_id: snowflake(&server.mod_log_channel_id),
            trusted_domains,
            locations: false,
        });
    }

    guilds
}

/// What noprofit.space keeps of rux's settings and where each goes in the panel's section.
const BOOLEANS: [(&str, &str); 2] = [
    ("watch.guidelines", "watch_guidelines"),
    ("spam.enabled", "spam_enabled"),
];
const STRINGS: [(&str, &str); 7] = [
    ("guild_id", "guild_id"),
    ("giveaway_channel_id", "giveaway_channel_id"),
    ("mod_log_channel_id", "mod_log_channel_id"),
    ("timezone", "timezone"),
    ("locale", "locale"),
    ("warnings.timeout", "warn_timeout"),
    ("warnings.ban", "warn_ban"),
];
const INTEGERS: [(&str, &str); 16] = [
    ("giveaway.max_credits", "giveaway_max_credits"),
    ("giveaway.max_winners", "giveaway_max_winners"),
    ("giveaway.max_days", "giveaway_max_days"),
    ("credits.max_per_grant", "credit_max_per_grant"),
    ("warnings.timeout_at", "warn_timeout_at"),
    ("warnings.ban_at", "warn_ban_at"),
    ("rules.cooldown_seconds", "rule_cooldown_seconds"),
    ("spam.flood_messages", "spam_flood_messages"),
    ("spam.flood_seconds", "spam_flood_seconds"),
    ("spam.duplicates", "spam_duplicates"),
    ("spam.duplicate_seconds", "spam_duplicate_seconds"),
    ("spam.mentions", "spam_mentions"),
    ("spam.emoji", "spam_emoji"),
    ("messages_per_minute", "messages_per_minute"),
    ("agent.max_steps", "max_steps"),
    ("agent.history_minutes", "history_minutes"),
];
const LISTS: [(&str, &str); 2] = [
    ("trusted_domains", "trusted_domains"),
    ("credits.granters", "credit_granters"),
];

fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(value, |value, key| &value[key])
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) if number.is_i64() || number.is_u64() => Some(number.to_string()),
        _ => None,
    }
}

fn integer_of(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => {
            text.parse().ok()
        }
        _ => None,
    }
}

fn strings_of(value: &Value) -> Option<Vec<CompactString>> {
    let list = value.as_array()?;

    Some(
        list.iter()
            .filter_map(text_of)
            .filter(|text| !text.is_empty())
            .map(CompactString::from)
            .collect(),
    )
}

/// The panel's rux settings with noprofit.space's laid over them: every key noprofit.space
/// keeps replaces the panel's (lists whole, its servers all of the panel's), every other key
/// stays. Values of the wrong type are skipped; numbers are clamped into what the panel's own
/// settings page would take. Returns noprofit.space's `enabled`.
pub fn overlay(settings: &mut AppSettingsRux, remote: &Value) -> bool {
    // the panel's settings as JSON, so every key goes in by name
    let mut panel = serde_json::to_value(&*settings).unwrap_or(Value::Null);
    let Some(object) = panel.as_object_mut() else {
        return false;
    };

    for (from, to) in BOOLEANS {
        if let Some(value) = at(remote, from).as_bool() {
            object.insert(to.into(), Value::Bool(value));
        }
    }
    for (from, to) in STRINGS {
        if let Some(value) = text_of(at(remote, from)) {
            object.insert(to.into(), Value::String(value));
        }
    }
    for (from, to) in INTEGERS {
        if let Some(value) = integer_of(at(remote, from)) {
            object.insert(to.into(), Value::from(value));
        }
    }
    for (from, to) in LISTS {
        if let Some(value) = strings_of(at(remote, from)) {
            object.insert(to.into(), serde_json::to_value(value).unwrap_or_default());
        }
    }
    if let Some(servers) = remote["servers"].as_array() {
        let servers: Vec<RuxServer> = servers
            .iter()
            .filter(|server| server.is_object())
            .take(MAX_SERVERS)
            .map(|server| RuxServer {
                id: text_of(&server["id"]).unwrap_or_default().into(),
                name: text_of(&server["name"]).unwrap_or_default().into(),
                about: text_of(&server["about"]).unwrap_or_default().into(),
                giveaway_channel_id: text_of(&server["giveaway_channel_id"])
                    .unwrap_or_default()
                    .into(),
                mod_log_channel_id: text_of(&server["mod_log_channel_id"])
                    .unwrap_or_default()
                    .into(),
                trusted_domains: strings_of(&server["trusted_domains"]).unwrap_or_default(),
            })
            .collect();
        object.insert(
            "servers".into(),
            serde_json::to_value(servers).unwrap_or_default(),
        );
    }

    // the secret is not part of the JSON (it serializes as a placeholder); keep the real one
    let secret = settings.noprofit_secret.take();
    if let Ok(mut merged) = serde_json::from_value::<AppSettingsRuxJson>(panel) {
        merged.clamp();
        *settings = merged.into_settings(secret);
    } else {
        settings.noprofit_secret = secret;
    }

    remote["enabled"].as_bool().unwrap_or(false)
}

/// [`AppSettingsRux`] as JSON, read back after the overlay.
#[derive(serde::Deserialize)]
struct AppSettingsRuxJson {
    name: CompactString,
    guild_id: CompactString,
    giveaway_channel_id: CompactString,
    mod_log_channel_id: CompactString,
    trusted_domains: Vec<CompactString>,
    servers: Vec<RuxServer>,
    timezone: CompactString,
    locale: CompactString,
    giveaway_max_credits: i64,
    giveaway_max_winners: i32,
    giveaway_max_days: i32,
    // plugged.host: what noprofit.space does not keep stays the panel's
    giveaway_max_tokens: i64,
    giveaway_max_membership_days: i32,
    tokens_command: bool,
    announce_channel_id: CompactString,
    token_bonus_posts: bool,
    credit_granters: Vec<CompactString>,
    credit_max_per_grant: i64,
    warn_timeout_at: i32,
    warn_timeout: CompactString,
    warn_ban_at: i32,
    warn_ban: CompactString,
    rule_cooldown_seconds: i32,
    watch_guidelines: bool,
    spam_enabled: bool,
    spam_flood_messages: i32,
    spam_flood_seconds: i32,
    spam_duplicates: i32,
    spam_duplicate_seconds: i32,
    spam_mentions: i32,
    spam_emoji: i32,
    messages_per_minute: i32,
    max_steps: i32,
    history_minutes: i32,
}

impl AppSettingsRuxJson {
    fn clamp(&mut self) {
        let defaults = AppSettingsRux::default();
        self.giveaway_max_credits = self.giveaway_max_credits.clamp(1, 1_000_000);
        self.giveaway_max_winners = self.giveaway_max_winners.clamp(1, 500);
        self.giveaway_max_days = self.giveaway_max_days.clamp(1, 365);
        self.giveaway_max_tokens = self.giveaway_max_tokens.clamp(1, 100_000);
        self.giveaway_max_membership_days = self.giveaway_max_membership_days.clamp(1, 365);
        self.credit_max_per_grant = self.credit_max_per_grant.clamp(1, 1_000_000);
        self.warn_timeout_at = self.warn_timeout_at.clamp(0, 100);
        self.warn_ban_at = self.warn_ban_at.clamp(0, 100);
        self.rule_cooldown_seconds = self.rule_cooldown_seconds.clamp(0, 86400);
        self.spam_flood_messages = self.spam_flood_messages.clamp(2, 100);
        self.spam_flood_seconds = self.spam_flood_seconds.clamp(1, 600);
        self.spam_duplicates = self.spam_duplicates.clamp(2, 100);
        self.spam_duplicate_seconds = self.spam_duplicate_seconds.clamp(1, 3600);
        self.spam_mentions = self.spam_mentions.clamp(2, 100);
        self.spam_emoji = self.spam_emoji.clamp(2, 500);
        self.messages_per_minute = self.messages_per_minute.clamp(1, 120);
        self.max_steps = self.max_steps.clamp(2, 30);
        self.history_minutes = self.history_minutes.clamp(1, 1440);
        if !crate::settings::rux::timezone_valid(&self.timezone) {
            self.timezone = defaults.timezone.clone();
        }
        let locale = self.locale.to_ascii_lowercase();
        self.locale = if crate::settings::rux::LOCALES.contains(&locale.as_str()) {
            locale.into()
        } else {
            defaults.locale.clone()
        };
        for (value, default) in [
            (&mut self.warn_timeout, &defaults.warn_timeout),
            (&mut self.warn_ban, &defaults.warn_ban),
        ] {
            let lower = value.to_ascii_lowercase();
            *value = if super::time::duration_seconds(&lower).is_some_and(|seconds| seconds > 0) {
                lower.into()
            } else {
                default.clone()
            };
        }
        self.trusted_domains = clean_domains(&self.trusted_domains);
    }

    fn into_settings(self, noprofit_secret: Option<CompactString>) -> AppSettingsRux {
        AppSettingsRux {
            name: self.name,
            guild_id: self.guild_id,
            giveaway_channel_id: self.giveaway_channel_id,
            mod_log_channel_id: self.mod_log_channel_id,
            trusted_domains: self.trusted_domains,
            servers: self.servers,
            timezone: self.timezone,
            locale: self.locale,
            giveaway_max_credits: self.giveaway_max_credits,
            giveaway_max_winners: self.giveaway_max_winners,
            giveaway_max_days: self.giveaway_max_days,
            giveaway_max_tokens: self.giveaway_max_tokens,
            giveaway_max_membership_days: self.giveaway_max_membership_days,
            tokens_command: self.tokens_command,
            announce_channel_id: self.announce_channel_id,
            token_bonus_posts: self.token_bonus_posts,
            credit_granters: self.credit_granters,
            credit_max_per_grant: self.credit_max_per_grant,
            warn_timeout_at: self.warn_timeout_at,
            warn_timeout: self.warn_timeout,
            warn_ban_at: self.warn_ban_at,
            warn_ban: self.warn_ban,
            rule_cooldown_seconds: self.rule_cooldown_seconds,
            watch_guidelines: self.watch_guidelines,
            spam_enabled: self.spam_enabled,
            spam_flood_messages: self.spam_flood_messages,
            spam_flood_seconds: self.spam_flood_seconds,
            spam_duplicates: self.spam_duplicates,
            spam_duplicate_seconds: self.spam_duplicate_seconds,
            spam_mentions: self.spam_mentions,
            spam_emoji: self.spam_emoji,
            messages_per_minute: self.messages_per_minute,
            max_steps: self.max_steps,
            history_minutes: self.history_minutes,
            noprofit_secret,
        }
    }
}

/// noprofit.space's host as people know it (`accounts.example.org` → `example.org`).
pub fn portal_name(accounts_url: &str) -> Option<String> {
    let host = reqwest::Url::parse(accounts_url)
        .ok()?
        .host_str()?
        .to_string();

    Some(host.strip_prefix("accounts.").unwrap_or(&host).to_string())
}

/// The settings rux runs on now.
pub async fn load(state: &crate::State) -> Result<Arc<Config>, anyhow::Error> {
    let (
        mut settings,
        discord_guild_id,
        boost_two_role_id,
        app_name,
        app_url,
        noprofit,
        memberships,
    ) = {
        let all = state.settings.get().await?;
        (
            all.rux.clone(),
            all.discord.guild_id.to_string(),
            all.discord.boost_two_role_id.to_string(),
            all.app.name.to_string(),
            all.app.url.trim_end_matches('/').to_string(),
            all.noprofit.config(),
            // plugged.host: the memberships' names, for giveaways and /tokens
            MembershipNames {
                plus: all.economy.plus_name.to_string(),
                plusplus: all.economy.plusplus_name.to_string(),
            },
        )
    };

    let (portal, link_url) = match &noprofit {
        Some(config) => (
            portal_name(&config.accounts_url).unwrap_or_else(|| app_name.clone()),
            config.accounts_url.clone(),
        ),
        None => (app_name.clone(), app_url),
    };

    let mut enabled = true;
    let mut managed = None;
    let mut managed_bot_token = None;
    if let Some(config) = &noprofit
        && let Some(copy) =
            super::noprofit::settings(state, config, settings.noprofit_secret.as_deref()).await
        && copy.configured
        && let Some(remote) = &copy.settings
    {
        enabled = overlay(&mut settings, remote);
        managed_bot_token = remote["bot_token"]
            .as_str()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        managed = Some(Managed {
            by: portal.clone(),
            url: copy.admin_url.clone(),
            fetched_at: copy.fetched_at,
            saved_at: copy.saved_at.clone(),
            saved_by: copy.saved_by.clone(),
        });
    }

    Ok(Arc::new(Config {
        guilds: guilds(&settings, &discord_guild_id, &app_name),
        settings,
        enabled,
        app_name,
        portal,
        link_url,
        boost_two_role_id,
        managed,
        managed_bot_token,
        memberships,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MAIN: &str = "100000000000000001";
    const OTHER: &str = "100000000000000002";
    const THIRD: &str = "100000000000000003";

    #[test]
    fn no_main_server_no_servers() {
        let settings = AppSettingsRux::default();
        assert!(guilds(&settings, "", "Example").is_empty());
        assert!(guilds(&settings, "not an id", "Example").is_empty());
    }

    #[test]
    fn the_main_server_falls_back_to_the_discord_section() {
        let mut settings = AppSettingsRux {
            trusted_domains: vec!["Example.com".into()],
            ..Default::default()
        };
        settings.servers = vec![
            RuxServer {
                id: OTHER.into(),
                name: "  Other   host ".into(),
                trusted_domains: vec!["other.org".into(), "example.com".into()],
                ..Default::default()
            },
            RuxServer {
                id: MAIN.into(),
                ..Default::default()
            },
            RuxServer {
                id: "bad".into(),
                ..Default::default()
            },
        ];

        let list = guilds(&settings, MAIN, "Example");
        assert_eq!(list.len(), 2);
        assert!(list[0].primary && list[0].locations);
        assert_eq!(list[0].id, MAIN);
        assert_eq!(list[0].host_name, "Example");
        assert_eq!(list[1].host_name, "Other host");
        assert_eq!(list[1].about, DEFAULT_ABOUT);
        assert_eq!(list[1].trusted_domains, vec!["example.com", "other.org"]);
        assert_eq!(
            list[1].label(),
            format!("Other host (Discord server {OTHER})")
        );

        settings.guild_id = THIRD.into();
        assert_eq!(guilds(&settings, MAIN, "Example")[0].id, THIRD);
    }

    #[test]
    fn noprofit_keys_replace_the_panels() {
        let mut settings = AppSettingsRux {
            name: "rux".into(),
            noprofit_secret: Some("kept".into()),
            ..Default::default()
        };
        let remote = json!({
            "enabled": true,
            "guild_id": MAIN,
            "timezone": "Europe/Berlin",
            "locale": "DE",
            "giveaway": {"max_credits": 500, "max_winners": "3"},
            "credits": {"granters": [OTHER, 7], "max_per_grant": 99999999},
            "warnings": {"timeout_at": 2, "timeout": "1H", "ban": "nonsense"},
            "spam": {"enabled": false},
            "agent": {"max_steps": "x"},
            "servers": [{"id": OTHER, "name": "Other", "trusted_domains": ["other.org"]}, "junk"],
            "bot_token": "token"
        });

        assert!(overlay(&mut settings, &remote));
        assert_eq!(settings.guild_id, MAIN);
        assert_eq!(settings.timezone, "Europe/Berlin");
        assert_eq!(settings.locale, "de");
        assert_eq!(settings.giveaway_max_credits, 500);
        assert_eq!(settings.giveaway_max_winners, 3);
        assert_eq!(
            settings.credit_granters,
            vec![CompactString::from(OTHER), CompactString::from("7")]
        );
        assert_eq!(settings.credit_max_per_grant, 1_000_000);
        assert_eq!(settings.warn_timeout_at, 2);
        assert_eq!(settings.warn_timeout, "1h");
        assert_eq!(settings.warn_ban, "24h");
        assert!(!settings.spam_enabled);
        assert_eq!(settings.max_steps, 8);
        assert_eq!(settings.servers.len(), 1);
        assert_eq!(settings.servers[0].id, OTHER);
        assert_eq!(settings.name, "rux");
        assert_eq!(settings.noprofit_secret.as_deref(), Some("kept"));

        let mut settings = AppSettingsRux::default();
        assert!(!overlay(&mut settings, &json!({"guild_id": MAIN})));
    }

    #[test]
    fn portal_is_the_accounts_host_without_its_prefix() {
        assert_eq!(
            portal_name("https://accounts.example.org").as_deref(),
            Some("example.org")
        );
        assert_eq!(
            portal_name("https://id.example.org/").as_deref(),
            Some("id.example.org")
        );
        assert_eq!(portal_name("nope"), None);
    }

    #[test]
    fn the_application_is_in_the_token() {
        use base64::Engine;

        let first = base64::engine::general_purpose::STANDARD_NO_PAD.encode(MAIN);
        assert_eq!(
            application_id_of(&format!("{first}.abc.def")).as_deref(),
            Some(MAIN)
        );
        // newer applications have 19 digits: the token leaves out base64's padding
        let newer = "1000000000000000001";
        for first in [
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(newer),
            base64::engine::general_purpose::STANDARD.encode(newer),
        ] {
            assert_eq!(
                application_id_of(&format!("{first}.abc.def")).as_deref(),
                Some(newer)
            );
        }
        assert_eq!(application_id_of("not-a-token"), None);
        assert_eq!(application_id_of(""), None);
    }

    #[test]
    fn invite_links_preselect_a_valid_server() {
        let url = Config::invite_url("123", Some(MAIN));
        assert!(url.contains(&format!("&guild_id={MAIN}&disable_guild_select=true")));
        assert!(!Config::invite_url("123", Some("x")).contains("guild_id"));
    }
}
