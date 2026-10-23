// plugged.host: the recovery settings section: deleted servers' copies (docs/features/backups-page.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What survives a server: how long a deleted server's system backups are kept for its owner.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsRecovery {
    /// Days a deleted server's newest system backup stays downloadable from the owner's Backups
    /// page before it is removed; 0 removes the system backups with the server.
    pub deleted_hold_days: i32,
}

impl Default for AppSettingsRecovery {
    fn default() -> Self {
        Self {
            deleted_hold_days: 7,
        }
    }
}

/// The recovery tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadRecovery {
    #[garde(inner(range(min = 0, max = 90)))]
    deleted_hold_days: Option<i32>,
}

impl PayloadRecovery {
    pub fn apply(self, settings: &mut AppSettingsRecovery) {
        if let Some(deleted_hold_days) = self.deleted_hold_days {
            settings.deleted_hold_days = deleted_hold_days;
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsRecovery {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer.write_raw_setting(
            "deleted_hold_days",
            self.deleted_hold_days.to_compact_string(),
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
        }))
    }
}
