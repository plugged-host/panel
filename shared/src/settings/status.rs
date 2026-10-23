// plugged.host: the status page settings section (Admin › Settings › Status, shared/src/status.rs).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::prelude::StringExt;
use compact_str::ToCompactString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The public status page. While it is off the panel neither probes nor serves `/data/*`.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsStatus {
    /// Whether the panel collects status every minute and serves `/data/*.json` and the feed.
    pub enabled: bool,
    /// The website's health URL, probed every minute; empty: the website is not listed.
    pub site_url: Option<compact_str::CompactString>,
    /// A `Host` header for the website probe (when the URL is an address).
    pub site_host: Option<compact_str::CompactString>,
    /// The panel's health URL, probed every minute; empty: the panel is not listed.
    pub panel_url: Option<compact_str::CompactString>,
    /// The public status page, linked from the feed.
    pub page_url: Option<compact_str::CompactString>,
    /// Days of daily uptime kept (and shown, up to 90).
    pub history_days: i32,
    /// CPU use in percent that, held for five checks, marks a node degraded.
    pub cpu_threshold: i32,
}

impl Default for AppSettingsStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            site_url: None,
            site_host: None,
            panel_url: None,
            page_url: None,
            history_days: 90,
            cpu_threshold: 95,
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsStatus {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("site_url", self.site_url.clone().unwrap_or_default())
            .write_raw_setting("site_host", self.site_host.clone().unwrap_or_default())
            .write_raw_setting("panel_url", self.panel_url.clone().unwrap_or_default())
            .write_raw_setting("page_url", self.page_url.clone().unwrap_or_default())
            .write_raw_setting("history_days", self.history_days.to_compact_string())
            .write_raw_setting("cpu_threshold", self.cpu_threshold.to_compact_string()))
    }
}

pub struct AppSettingsStatusDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsStatusDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsStatus::default();

        Ok(Box::new(AppSettingsStatus {
            enabled: deserializer
                .take_raw_setting("enabled")
                .map(|s| s == "true")
                .unwrap_or(defaults.enabled),
            site_url: deserializer
                .take_raw_setting("site_url")
                .and_then(|s| s.into_optional()),
            site_host: deserializer
                .take_raw_setting("site_host")
                .and_then(|s| s.into_optional()),
            panel_url: deserializer
                .take_raw_setting("panel_url")
                .and_then(|s| s.into_optional()),
            page_url: deserializer
                .take_raw_setting("page_url")
                .and_then(|s| s.into_optional()),
            history_days: deserializer
                .take_raw_setting("history_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.history_days),
            cpu_threshold: deserializer
                .take_raw_setting("cpu_threshold")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.cpu_threshold),
        }))
    }
}
