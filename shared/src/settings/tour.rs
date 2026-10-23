// plugged.host: the welcome tour settings section (docs/features/welcome-tour.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The guide drawn over the server list and the first console a user opens.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsTour {
    /// Users who have not finished or skipped the tour see it.
    pub enabled: bool,
    /// Raising it shows the tour again to everyone who already saw it.
    pub version: i32,
}

impl Default for AppSettingsTour {
    fn default() -> Self {
        Self {
            enabled: false,
            version: 1,
        }
    }
}

/// The welcome tour tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadTour {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 1000)))]
    version: Option<i32>,
}

impl PayloadTour {
    pub fn apply(self, settings: &mut AppSettingsTour) {
        if let Some(enabled) = self.enabled {
            settings.enabled = enabled;
        }
        if let Some(version) = self.version {
            settings.version = version;
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsTour {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("version", self.version.to_compact_string()))
    }
}

pub struct AppSettingsTourDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsTourDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsTour::default();

        Ok(Box::new(AppSettingsTour {
            enabled: deserializer
                .take_raw_setting("enabled")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.enabled),
            version: deserializer
                .take_raw_setting("version")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.version),
        }))
    }
}
