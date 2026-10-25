// plugged.host: the recovery settings section: deleted servers' copies (docs/features/backups-page.md)
// and node failover (docs/features/failover.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// When the panel moves a node's servers to other nodes by itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ToSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailoverMode {
    /// Never, not even by hand.
    Off,
    /// Only when an admin starts it on the node's Failover tab.
    #[default]
    Manual,
    /// By hand, and a node unreachable for `failover_after` checks is reported to the admins with
    /// what a failover would do, without restoring anything.
    Notify,
    /// Also on its own, once a node has been unreachable for `failover_after` checks.
    Auto,
}

impl FailoverMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Manual => "manual",
            Self::Notify => "notify",
            Self::Auto => "auto",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "manual" => Some(Self::Manual),
            "notify" => Some(Self::Notify),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// What survives a server or its node: how long a deleted server's system backups are kept for its
/// owner, and when a node's servers are restored on other nodes.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsRecovery {
    /// Days a deleted server's newest system backup stays downloadable from the owner's Backups
    /// page before it is removed; 0 removes the system backups with the server.
    pub deleted_hold_days: i32,
    pub failover_mode: FailoverMode,
    /// Checks in a row (one a minute) a node must miss, unreachable, before an automatic failover.
    pub failover_after: i32,
    /// Servers of one failover restored at the same time.
    pub failover_parallel: i32,
}

impl Default for AppSettingsRecovery {
    fn default() -> Self {
        Self {
            deleted_hold_days: 7,
            failover_mode: FailoverMode::Manual,
            failover_after: 10,
            failover_parallel: 3,
        }
    }
}

/// The recovery tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadRecovery {
    #[garde(inner(range(min = 0, max = 90)))]
    deleted_hold_days: Option<i32>,
    #[garde(skip)]
    failover_mode: Option<FailoverMode>,
    #[garde(inner(range(min = 3, max = 1440)))]
    failover_after: Option<i32>,
    #[garde(inner(range(min = 1, max = 20)))]
    failover_parallel: Option<i32>,
}

impl PayloadRecovery {
    pub fn apply(self, settings: &mut AppSettingsRecovery) {
        if let Some(deleted_hold_days) = self.deleted_hold_days {
            settings.deleted_hold_days = deleted_hold_days;
        }
        if let Some(failover_mode) = self.failover_mode {
            settings.failover_mode = failover_mode;
        }
        if let Some(failover_after) = self.failover_after {
            settings.failover_after = failover_after;
        }
        if let Some(failover_parallel) = self.failover_parallel {
            settings.failover_parallel = failover_parallel;
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsRecovery {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting(
                "deleted_hold_days",
                self.deleted_hold_days.to_compact_string(),
            )
            .write_raw_setting("failover_mode", self.failover_mode.as_str())
            .write_raw_setting("failover_after", self.failover_after.to_compact_string())
            .write_raw_setting(
                "failover_parallel",
                self.failover_parallel.to_compact_string(),
            ))
    }
}

pub struct AppSettingsRecoveryDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsRecoveryDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsRecovery::default();

        Ok(Box::new(AppSettingsRecovery {
            deleted_hold_days: deserializer
                .take_raw_setting("deleted_hold_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.deleted_hold_days),
            failover_mode: deserializer
                .take_raw_setting("failover_mode")
                .and_then(|s| FailoverMode::parse(&s))
                .unwrap_or(defaults.failover_mode),
            failover_after: deserializer
                .take_raw_setting("failover_after")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.failover_after),
            failover_parallel: deserializer
                .take_raw_setting("failover_parallel")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.failover_parallel),
        }))
    }
}
