// plugged.host: the limbo settings section (docs/features/limbo.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Limbo: idle servers are stopped, then parked off their node until their owner restores them
/// (shared/src/limbo.rs).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsLimbo {
    pub enabled: bool,
    /// Minutes without activity after which a running server is stopped.
    pub stop_minutes: i32,
    /// Minutes without activity after which a stopped server is parked.
    pub park_minutes: i32,
    /// Stops and parks each started per run (once a minute).
    pub max_per_run: i32,
    /// Tokens a restore costs within `cost_window_hours` of parking, and after.
    pub cost_early: i32,
    pub cost_late: i32,
    pub cost_window_hours: i32,
    /// Servers of accounts with a membership are never stopped or parked.
    pub exempt_members: bool,
    /// Days a server may stay parked before it is deleted (0: never).
    pub delete_after_days: i32,
}

impl Default for AppSettingsLimbo {
    fn default() -> Self {
        Self {
            enabled: false,
            stop_minutes: 48 * 60,
            park_minutes: 72 * 60,
            max_per_run: 3,
            cost_early: 5,
            cost_late: 10,
            cost_window_hours: 72,
            exempt_members: true,
            delete_after_days: 0,
        }
    }
}

/// The limbo tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadLimbo {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 525_600)))]
    stop_minutes: Option<i32>,
    #[garde(inner(range(min = 1, max = 525_600)))]
    park_minutes: Option<i32>,
    #[garde(inner(range(min = 1, max = 100)))]
    max_per_run: Option<i32>,
    #[garde(inner(range(min = 0, max = 100000)))]
    cost_early: Option<i32>,
    #[garde(inner(range(min = 0, max = 100000)))]
    cost_late: Option<i32>,
    #[garde(inner(range(min = 0, max = 8760)))]
    cost_window_hours: Option<i32>,
    #[garde(skip)]
    exempt_members: Option<bool>,
    #[garde(inner(range(min = 0, max = 3650)))]
    delete_after_days: Option<i32>,
}

impl PayloadLimbo {
    pub fn apply(self, settings: &mut AppSettingsLimbo) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }

        set!(
            enabled,
            stop_minutes,
            park_minutes,
            max_per_run,
            cost_early,
            cost_late,
            cost_window_hours,
            exempt_members,
            delete_after_days
        );
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsLimbo {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("stop_minutes", self.stop_minutes.to_compact_string())
            .write_raw_setting("park_minutes", self.park_minutes.to_compact_string())
            .write_raw_setting("max_per_run", self.max_per_run.to_compact_string())
            .write_raw_setting("cost_early", self.cost_early.to_compact_string())
            .write_raw_setting("cost_late", self.cost_late.to_compact_string())
            .write_raw_setting(
                "cost_window_hours",
                self.cost_window_hours.to_compact_string(),
            )
            .write_raw_setting("exempt_members", self.exempt_members.to_compact_string())
            .write_raw_setting(
                "delete_after_days",
                self.delete_after_days.to_compact_string(),
            ))
    }
}

pub struct AppSettingsLimboDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsLimboDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsLimbo::default();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        // the windows were hours until 2026-10-24: a stored hour value still counts
        macro_rules! minutes {
            ($field:ident, $hours:ident) => {{
                let minutes = deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse::<i32>().ok());
                let hours = deserializer
                    .take_raw_setting(stringify!($hours))
                    .and_then(|s| s.parse::<i32>().ok());
                minutes
                    .or(hours.map(|hours| hours.saturating_mul(60)))
                    .unwrap_or(defaults.$field)
            }};
        }

        Ok(Box::new(AppSettingsLimbo {
            enabled: read!(enabled),
            stop_minutes: minutes!(stop_minutes, stop_hours),
            park_minutes: minutes!(park_minutes, park_hours),
            max_per_run: read!(max_per_run),
            cost_early: read!(cost_early),
            cost_late: read!(cost_late),
            cost_window_hours: read!(cost_window_hours),
            exempt_members: read!(exempt_members),
            delete_after_days: read!(delete_after_days),
        }))
    }
}
