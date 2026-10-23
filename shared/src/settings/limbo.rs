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
    /// Hours without activity after which a running server is stopped.
    pub stop_hours: i32,
    /// Hours without activity after which a stopped server is parked.
    pub park_hours: i32,
    /// Stops and parks each started per run (once a minute).
    pub max_per_run: i32,
    /// Tokens a restore costs within `cost_window_hours` of parking, and after.
    pub cost_early: i32,
    pub cost_late: i32,
    pub cost_window_hours: i32,
    /// Servers of accounts with a membership are never stopped or parked.
    pub exempt_members: bool,
}

impl Default for AppSettingsLimbo {
    fn default() -> Self {
        Self {
            enabled: false,
            stop_hours: 48,
            park_hours: 72,
            max_per_run: 3,
            cost_early: 5,
            cost_late: 10,
            cost_window_hours: 72,
            exempt_members: true,
        }
    }
}

/// The limbo tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadLimbo {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 8760)))]
    stop_hours: Option<i32>,
    #[garde(inner(range(min = 1, max = 8760)))]
    park_hours: Option<i32>,
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
            stop_hours,
            park_hours,
            max_per_run,
            cost_early,
            cost_late,
            cost_window_hours,
            exempt_members
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
            .write_raw_setting("stop_hours", self.stop_hours.to_compact_string())
            .write_raw_setting("park_hours", self.park_hours.to_compact_string())
            .write_raw_setting("max_per_run", self.max_per_run.to_compact_string())
            .write_raw_setting("cost_early", self.cost_early.to_compact_string())
            .write_raw_setting("cost_late", self.cost_late.to_compact_string())
            .write_raw_setting(
                "cost_window_hours",
                self.cost_window_hours.to_compact_string(),
            )
            .write_raw_setting("exempt_members", self.exempt_members.to_compact_string()))
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

        Ok(Box::new(AppSettingsLimbo {
            enabled: read!(enabled),
            stop_hours: read!(stop_hours),
            park_hours: read!(park_hours),
            max_per_run: read!(max_per_run),
            cost_early: read!(cost_early),
            cost_late: read!(cost_late),
            cost_window_hours: read!(cost_window_hours),
            exempt_members: read!(exempt_members),
        }))
    }
}
