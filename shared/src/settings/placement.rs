// plugged.host: the placement settings section: when a loaded node stops taking new servers (docs/features/placement.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The automatic deployment pause: a node over the line takes no new servers until its average
/// over the last minutes is under it again.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsPlacement {
    /// The panel pauses and reopens nodes by itself.
    pub pause_enabled: bool,
    /// Memory or CPU use in percent above which a node is paused.
    pub pause_percent: i32,
    /// Points the line is lowered by on a small node.
    pub small_offset: i32,
    /// A node with less memory (GiB) or fewer threads than this is small.
    pub small_memory_gb: i32,
    pub small_threads: i32,
    /// Samples in a row without an answer after which a node is paused.
    pub unreachable_after: i32,
}

impl Default for AppSettingsPlacement {
    fn default() -> Self {
        Self {
            pause_enabled: true,
            pause_percent: 85,
            small_offset: 10,
            small_memory_gb: 34,
            small_threads: 9,
            unreachable_after: 3,
        }
    }
}

/// The placement tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadPlacement {
    #[garde(skip)]
    pause_enabled: Option<bool>,
    #[garde(inner(range(min = 10, max = 100)))]
    pause_percent: Option<i32>,
    #[garde(inner(range(min = 0, max = 50)))]
    small_offset: Option<i32>,
    #[garde(inner(range(min = 0, max = 4096)))]
    small_memory_gb: Option<i32>,
    #[garde(inner(range(min = 0, max = 1024)))]
    small_threads: Option<i32>,
    #[garde(inner(range(min = 1, max = 60)))]
    unreachable_after: Option<i32>,
}

impl PayloadPlacement {
    pub fn apply(self, settings: &mut AppSettingsPlacement) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            pause_enabled,
            pause_percent,
            small_offset,
            small_memory_gb,
            small_threads,
            unreachable_after
        );
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsPlacement {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("pause_enabled", self.pause_enabled.to_compact_string())
            .write_raw_setting("pause_percent", self.pause_percent.to_compact_string())
            .write_raw_setting("small_offset", self.small_offset.to_compact_string())
            .write_raw_setting("small_memory_gb", self.small_memory_gb.to_compact_string())
            .write_raw_setting("small_threads", self.small_threads.to_compact_string())
            .write_raw_setting(
                "unreachable_after",
                self.unreachable_after.to_compact_string(),
            ))
    }
}

pub struct AppSettingsPlacementDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsPlacementDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsPlacement::default();

        macro_rules! parsed {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsPlacement {
            pause_enabled: parsed!(pause_enabled),
            pause_percent: parsed!(pause_percent),
            small_offset: parsed!(small_offset),
            small_memory_gb: parsed!(small_memory_gb),
            small_threads: parsed!(small_threads),
            unreachable_after: parsed!(unreachable_after),
        }))
    }
}
