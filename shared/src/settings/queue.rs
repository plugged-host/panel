// plugged.host: the start queue settings section (docs/features/start-queue.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The start queue: starts on a busy node wait their turn (shared/src/start_queue.rs).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsQueue {
    pub enabled: bool,
    /// A restart waits too (the server keeps running until it is released).
    pub include_restart: bool,
    /// Memory or CPU use in percent above which a node holds starts back; lower on a small node by
    /// the placement settings' offset.
    pub line_percent: i32,
    /// Seconds after which a start goes out however busy the node is.
    pub max_wait: i32,
    /// Seconds between two releases on one node.
    pub release_interval: i32,
    /// Minutes after a release in which the server's next start skips the queue.
    pub pass_minutes: i32,
    /// The queue page offers to move the server to a quieter node of its location.
    pub move_offer: bool,
    /// Only to accounts with a membership.
    pub move_requires_membership: bool,
    /// Only to servers whose disk limit is under this, in MiB (0: any size); never to a server with
    /// unlimited disk.
    pub move_max_disk: i32,
}

impl Default for AppSettingsQueue {
    fn default() -> Self {
        Self {
            enabled: false,
            include_restart: true,
            line_percent: 85,
            max_wait: 600,
            release_interval: 20,
            pass_minutes: 10,
            move_offer: true,
            move_requires_membership: true,
            move_max_disk: 3072,
        }
    }
}

/// The start queue tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadQueue {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(skip)]
    include_restart: Option<bool>,
    #[garde(inner(range(min = 1, max = 100)))]
    line_percent: Option<i32>,
    #[garde(inner(range(min = 30, max = 86400)))]
    max_wait: Option<i32>,
    #[garde(inner(range(min = 0, max = 3600)))]
    release_interval: Option<i32>,
    #[garde(inner(range(min = 0, max = 1440)))]
    pass_minutes: Option<i32>,
    #[garde(skip)]
    move_offer: Option<bool>,
    #[garde(skip)]
    move_requires_membership: Option<bool>,
    #[garde(inner(range(min = 0, max = 1048576)))]
    move_max_disk: Option<i32>,
}

impl PayloadQueue {
    pub fn apply(self, settings: &mut AppSettingsQueue) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }

        set!(
            enabled,
            include_restart,
            line_percent,
            max_wait,
            release_interval,
            pass_minutes,
            move_offer,
            move_requires_membership,
            move_max_disk
        );
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsQueue {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("include_restart", self.include_restart.to_compact_string())
            .write_raw_setting("line_percent", self.line_percent.to_compact_string())
            .write_raw_setting("max_wait", self.max_wait.to_compact_string())
            .write_raw_setting(
                "release_interval",
                self.release_interval.to_compact_string(),
            )
            .write_raw_setting("pass_minutes", self.pass_minutes.to_compact_string())
            .write_raw_setting("move_offer", self.move_offer.to_compact_string())
            .write_raw_setting(
                "move_requires_membership",
                self.move_requires_membership.to_compact_string(),
            )
            .write_raw_setting("move_max_disk", self.move_max_disk.to_compact_string()))
    }
}

pub struct AppSettingsQueueDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsQueueDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsQueue::default();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsQueue {
            enabled: read!(enabled),
            include_restart: read!(include_restart),
            line_percent: read!(line_percent),
            max_wait: read!(max_wait),
            release_interval: read!(release_interval),
            pass_minutes: read!(pass_minutes),
            move_offer: read!(move_offer),
            move_requires_membership: read!(move_requires_membership),
            move_max_disk: read!(move_max_disk),
        }))
    }
}
