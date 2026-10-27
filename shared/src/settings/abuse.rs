// plugged.host: abuse protection settings (Settings › Security › Abuse, shared/src/abuse.rs).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::prelude::StringExt;
use compact_str::ToCompactString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AbuseMode {
    /// A critical finding suspends the server.
    Suspend,
    /// Findings are recorded and alerted, nothing is suspended.
    #[default]
    Report,
    /// Nothing is checked.
    Off,
}

impl AbuseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Report => "report",
            Self::Off => "off",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "suspend" => Some(Self::Suspend),
            "report" => Some(Self::Report),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// Abuse protection. It starts in `report`, so a fresh install shows what it would do before it
/// suspends anyone.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsAbuse {
    pub mode: AbuseMode,
    /// A Discord webhook for suspensions, deletions and new critical findings. Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub alert_webhook: Option<compact_str::CompactString>,
    /// Percent of its CPU limit a server must hold for [`Self::cpu_minutes`] to get a full check.
    pub cpu_percent: i32,
    pub cpu_minutes: i32,
    /// Hours after an abuse suspension until the server is deleted; 0 never deletes.
    pub delete_after_hours: i32,
    /// The panel's own account standing, without noprofit.space (docs/features/standing.md):
    /// the suspensions from which an account is limited, very limited and suspended, and for how
    /// many days after its latest suspension a limit keeps it from creating servers.
    pub standing_limited_at: i32,
    pub standing_limited_days: i32,
    pub standing_very_limited_at: i32,
    pub standing_very_limited_days: i32,
    pub standing_suspended_at: i32,
}

impl AppSettingsAbuse {
    /// The standing rules these settings describe.
    pub fn standing_rules(&self) -> crate::standing::Rules {
        crate::standing::Rules {
            limited_at: self.standing_limited_at as i64,
            limited_days: self.standing_limited_days as i64,
            very_limited_at: self.standing_very_limited_at as i64,
            very_limited_days: self.standing_very_limited_days as i64,
            suspended_at: self.standing_suspended_at as i64,
        }
    }
}

impl Default for AppSettingsAbuse {
    fn default() -> Self {
        Self {
            mode: AbuseMode::Report,
            alert_webhook: None,
            cpu_percent: 90,
            cpu_minutes: 20,
            delete_after_hours: 24,
            standing_limited_at: 2,
            standing_limited_days: 7,
            standing_very_limited_at: 3,
            standing_very_limited_days: 30,
            standing_suspended_at: 4,
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsAbuse {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let serializer = serializer
            .write_raw_setting("mode", self.mode.as_str())
            .write_raw_setting("cpu_percent", self.cpu_percent.to_compact_string())
            .write_raw_setting("cpu_minutes", self.cpu_minutes.to_compact_string())
            .write_raw_setting(
                "delete_after_hours",
                self.delete_after_hours.to_compact_string(),
            )
            .write_raw_setting(
                "standing_limited_at",
                self.standing_limited_at.to_compact_string(),
            )
            .write_raw_setting(
                "standing_limited_days",
                self.standing_limited_days.to_compact_string(),
            )
            .write_raw_setting(
                "standing_very_limited_at",
                self.standing_very_limited_at.to_compact_string(),
            )
            .write_raw_setting(
                "standing_very_limited_days",
                self.standing_very_limited_days.to_compact_string(),
            )
            .write_raw_setting(
                "standing_suspended_at",
                self.standing_suspended_at.to_compact_string(),
            );

        Ok(match &self.alert_webhook {
            Some(webhook) => {
                serializer
                    .write_raw_encrypted_setting("alert_webhook", webhook.clone())
                    .await?
            }
            None => serializer.write_raw_setting("alert_webhook", ""),
        })
    }
}

pub struct AppSettingsAbuseDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsAbuseDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsAbuse::default();

        let alert_webhook = match deserializer.take_raw_setting("alert_webhook") {
            Some(webhook) if webhook.is_empty() => None,
            Some(webhook) => Some(deserializer.database.decrypt_base64(webhook).await?),
            None => None,
        };

        Ok(Box::new(AppSettingsAbuse {
            mode: deserializer
                .take_raw_setting("mode")
                .and_then(|s| AbuseMode::parse(&s))
                .unwrap_or(defaults.mode),
            alert_webhook: alert_webhook.and_then(|s| s.into_optional()),
            cpu_percent: deserializer
                .take_raw_setting("cpu_percent")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.cpu_percent),
            cpu_minutes: deserializer
                .take_raw_setting("cpu_minutes")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.cpu_minutes),
            delete_after_hours: deserializer
                .take_raw_setting("delete_after_hours")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.delete_after_hours),
            standing_limited_at: deserializer
                .take_raw_setting("standing_limited_at")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.standing_limited_at),
            standing_limited_days: deserializer
                .take_raw_setting("standing_limited_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.standing_limited_days),
            standing_very_limited_at: deserializer
                .take_raw_setting("standing_very_limited_at")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.standing_very_limited_at),
            standing_very_limited_days: deserializer
                .take_raw_setting("standing_very_limited_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.standing_very_limited_days),
            standing_suspended_at: deserializer
                .take_raw_setting("standing_suspended_at")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.standing_suspended_at),
        }))
    }
}
