// plugged.host: the server upgrades settings section (docs/features/server-upgrades.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What users buy with tokens for one server (shared/src/server_upgrades.rs): a keep-awake pass,
/// a boost, extra ports, databases and backup slots, a short subdomain name and a card colour.
/// Each has its switch and its price; nothing is sold while tokens are off.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsUpgrades {
    /// Limbo neither stops, parks nor deletes the server as idle while it runs (only offered
    /// while limbo is on).
    pub keep_awake_enabled: bool,
    pub keep_awake_tokens: i32,
    pub keep_awake_days: i32,
    /// More memory (MiB) and CPU (%) for some hours, on top of the paid limits and outside the
    /// allowance; sold while the server's node has room for it.
    pub boost_enabled: bool,
    pub boost_tokens: i32,
    pub boost_hours: i32,
    pub boost_memory: i32,
    pub boost_cpu: i32,
    /// One more port, database or backup slot for some days, at most `*_max` at once.
    pub extra_allocation_enabled: bool,
    pub extra_allocation_tokens: i32,
    pub extra_allocation_days: i32,
    pub extra_allocation_max: i32,
    pub extra_database_enabled: bool,
    pub extra_database_tokens: i32,
    pub extra_database_days: i32,
    pub extra_database_max: i32,
    pub extra_backup_enabled: bool,
    pub extra_backup_tokens: i32,
    pub extra_backup_days: i32,
    pub extra_backup_max: i32,
    /// The deposit a subdomain of 3 characters, and of 2, takes at least (refunded like any
    /// subdomain deposit).
    pub short_subdomain_enabled: bool,
    pub short_subdomain_three: i32,
    pub short_subdomain_two: i32,
    /// A colour on the server's card in the server list, bought once per server.
    pub card_color_enabled: bool,
    pub card_color_tokens: i32,
}

impl Default for AppSettingsUpgrades {
    fn default() -> Self {
        Self {
            keep_awake_enabled: true,
            keep_awake_tokens: 3,
            keep_awake_days: 7,
            boost_enabled: true,
            boost_tokens: 3,
            boost_hours: 24,
            boost_memory: 2048,
            boost_cpu: 100,
            extra_allocation_enabled: true,
            extra_allocation_tokens: 2,
            extra_allocation_days: 30,
            extra_allocation_max: 3,
            extra_database_enabled: true,
            extra_database_tokens: 2,
            extra_database_days: 30,
            extra_database_max: 3,
            extra_backup_enabled: true,
            extra_backup_tokens: 2,
            extra_backup_days: 30,
            extra_backup_max: 3,
            short_subdomain_enabled: true,
            short_subdomain_three: 15,
            short_subdomain_two: 30,
            card_color_enabled: true,
            card_color_tokens: 2,
        }
    }
}

/// The server upgrades tab of Admin › Settings › Economy: every field optional, `null` leaves it
/// alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadUpgrades {
    #[garde(skip)]
    keep_awake_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    keep_awake_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    keep_awake_days: Option<i32>,
    #[garde(skip)]
    boost_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    boost_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 720)))]
    boost_hours: Option<i32>,
    #[garde(inner(range(min = 0, max = 1048576)))]
    boost_memory: Option<i32>,
    #[garde(inner(range(min = 0, max = 6400)))]
    boost_cpu: Option<i32>,
    #[garde(skip)]
    extra_allocation_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    extra_allocation_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    extra_allocation_days: Option<i32>,
    #[garde(inner(range(min = 1, max = 50)))]
    extra_allocation_max: Option<i32>,
    #[garde(skip)]
    extra_database_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    extra_database_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    extra_database_days: Option<i32>,
    #[garde(inner(range(min = 1, max = 50)))]
    extra_database_max: Option<i32>,
    #[garde(skip)]
    extra_backup_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    extra_backup_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    extra_backup_days: Option<i32>,
    #[garde(inner(range(min = 1, max = 50)))]
    extra_backup_max: Option<i32>,
    #[garde(skip)]
    short_subdomain_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    short_subdomain_three: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    short_subdomain_two: Option<i32>,
    #[garde(skip)]
    card_color_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100000)))]
    card_color_tokens: Option<i32>,
}

impl PayloadUpgrades {
    pub fn apply(self, settings: &mut AppSettingsUpgrades) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }

        set!(
            keep_awake_enabled,
            keep_awake_tokens,
            keep_awake_days,
            boost_enabled,
            boost_tokens,
            boost_hours,
            boost_memory,
            boost_cpu,
            extra_allocation_enabled,
            extra_allocation_tokens,
            extra_allocation_days,
            extra_allocation_max,
            extra_database_enabled,
            extra_database_tokens,
            extra_database_days,
            extra_database_max,
            extra_backup_enabled,
            extra_backup_tokens,
            extra_backup_days,
            extra_backup_max,
            short_subdomain_enabled,
            short_subdomain_three,
            short_subdomain_two,
            card_color_enabled,
            card_color_tokens
        );
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsUpgrades {
    async fn serialize(
        &self,
        mut serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        for (key, value) in [
            (
                "keep_awake_enabled",
                self.keep_awake_enabled.to_compact_string(),
            ),
            (
                "keep_awake_tokens",
                self.keep_awake_tokens.to_compact_string(),
            ),
            ("keep_awake_days", self.keep_awake_days.to_compact_string()),
            ("boost_enabled", self.boost_enabled.to_compact_string()),
            ("boost_tokens", self.boost_tokens.to_compact_string()),
            ("boost_hours", self.boost_hours.to_compact_string()),
            ("boost_memory", self.boost_memory.to_compact_string()),
            ("boost_cpu", self.boost_cpu.to_compact_string()),
            (
                "extra_allocation_enabled",
                self.extra_allocation_enabled.to_compact_string(),
            ),
            (
                "extra_allocation_tokens",
                self.extra_allocation_tokens.to_compact_string(),
            ),
            (
                "extra_allocation_days",
                self.extra_allocation_days.to_compact_string(),
            ),
            (
                "extra_allocation_max",
                self.extra_allocation_max.to_compact_string(),
            ),
            (
                "extra_database_enabled",
                self.extra_database_enabled.to_compact_string(),
            ),
            (
                "extra_database_tokens",
                self.extra_database_tokens.to_compact_string(),
            ),
            (
                "extra_database_days",
                self.extra_database_days.to_compact_string(),
            ),
            (
                "extra_database_max",
                self.extra_database_max.to_compact_string(),
            ),
            (
                "extra_backup_enabled",
                self.extra_backup_enabled.to_compact_string(),
            ),
            (
                "extra_backup_tokens",
                self.extra_backup_tokens.to_compact_string(),
            ),
            (
                "extra_backup_days",
                self.extra_backup_days.to_compact_string(),
            ),
            (
                "extra_backup_max",
                self.extra_backup_max.to_compact_string(),
            ),
            (
                "short_subdomain_enabled",
                self.short_subdomain_enabled.to_compact_string(),
            ),
            (
                "short_subdomain_three",
                self.short_subdomain_three.to_compact_string(),
            ),
            (
                "short_subdomain_two",
                self.short_subdomain_two.to_compact_string(),
            ),
            (
                "card_color_enabled",
                self.card_color_enabled.to_compact_string(),
            ),
            (
                "card_color_tokens",
                self.card_color_tokens.to_compact_string(),
            ),
        ] {
            serializer = serializer.write_raw_setting(key, value);
        }

        Ok(serializer)
    }
}

pub struct AppSettingsUpgradesDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsUpgradesDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsUpgrades::default();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsUpgrades {
            keep_awake_enabled: read!(keep_awake_enabled),
            keep_awake_tokens: read!(keep_awake_tokens),
            keep_awake_days: read!(keep_awake_days),
            boost_enabled: read!(boost_enabled),
            boost_tokens: read!(boost_tokens),
            boost_hours: read!(boost_hours),
            boost_memory: read!(boost_memory),
            boost_cpu: read!(boost_cpu),
            extra_allocation_enabled: read!(extra_allocation_enabled),
            extra_allocation_tokens: read!(extra_allocation_tokens),
            extra_allocation_days: read!(extra_allocation_days),
            extra_allocation_max: read!(extra_allocation_max),
            extra_database_enabled: read!(extra_database_enabled),
            extra_database_tokens: read!(extra_database_tokens),
            extra_database_days: read!(extra_database_days),
            extra_database_max: read!(extra_database_max),
            extra_backup_enabled: read!(extra_backup_enabled),
            extra_backup_tokens: read!(extra_backup_tokens),
            extra_backup_days: read!(extra_backup_days),
            extra_backup_max: read!(extra_backup_max),
            short_subdomain_enabled: read!(short_subdomain_enabled),
            short_subdomain_three: read!(short_subdomain_three),
            short_subdomain_two: read!(short_subdomain_two),
            card_color_enabled: read!(card_color_enabled),
            card_color_tokens: read!(card_color_tokens),
        }))
    }
}
