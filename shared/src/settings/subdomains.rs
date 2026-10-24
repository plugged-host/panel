// plugged.host: the subdomains settings section (docs/features/subdomains.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Server subdomains under the domains of Admin › Domains, as DNS records on Cloudflare
/// (shared/src/subdomains).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsSubdomains {
    /// Players see the Subdomains page (once the token is set too).
    pub enabled: bool,
    /// A Cloudflare API token with Zone › Zone › Read and Zone › DNS › Edit on the domains.
    /// Stored encrypted, shown as a placeholder.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub cloudflare_token: Option<compact_str::CompactString>,
    /// Subdomains a server may have unless an admin gave it its own limit.
    pub default_limit: i32,
    /// Tokens a server's first, second, third and every further subdomain cost while tokens are
    /// on (Admin › Settings › Economy). Paid by the server's owner, refunded on deletion.
    pub cost_first: i32,
    pub cost_second: i32,
    pub cost_third: i32,
    pub cost_more: i32,
    /// Names nobody may take, besides the built-in ones (`www`, `panel`, `mail`, ...): comma
    /// separated, `*` matches anything (`test*`).
    pub blocked_names: compact_str::CompactString,
}

impl Default for AppSettingsSubdomains {
    fn default() -> Self {
        Self {
            enabled: false,
            cloudflare_token: None,
            default_limit: 3,
            cost_first: 1,
            cost_second: 3,
            cost_third: 8,
            cost_more: 15,
            blocked_names: compact_str::CompactString::default(),
        }
    }
}

impl AppSettingsSubdomains {
    /// Whether players are offered subdomains.
    pub fn active(&self) -> bool {
        self.enabled && self.cloudflare_token.is_some()
    }

    /// The tokens a server's subdomain costs when it already has `existing` of them.
    pub fn cost(&self, existing: i64) -> i32 {
        match existing {
            ..=0 => self.cost_first,
            1 => self.cost_second,
            2 => self.cost_third,
            _ => self.cost_more,
        }
    }
}

/// The subdomains tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadSubdomains {
    #[garde(skip)]
    enabled: Option<bool>,
    /// An empty string or the placeholder keeps the stored token, `null` removes it.
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    cloudflare_token: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0, max = 100)))]
    default_limit: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    cost_first: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    cost_second: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    cost_third: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000000)))]
    cost_more: Option<i32>,
    #[garde(inner(length(chars, max = 4096)))]
    blocked_names: Option<compact_str::CompactString>,
}

impl PayloadSubdomains {
    pub fn apply(self, settings: &mut AppSettingsSubdomains) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            enabled,
            default_limit,
            cost_first,
            cost_second,
            cost_third,
            cost_more
        );

        match self.cloudflare_token {
            Some(Some(token))
                if token.trim().is_empty() || token == crate::censor::CENSORED_PLACEHOLDER => {}
            Some(token) => settings.cloudflare_token = token.map(|token| token.trim().into()),
            None => {}
        }
        if let Some(names) = self.blocked_names {
            settings.blocked_names = names
                .split([',', '\n'])
                .map(|name| name.trim().to_lowercase())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join(",")
                .into();
        }
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsSubdomains {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let serializer = match &self.cloudflare_token {
            Some(token) => {
                serializer
                    .write_raw_encrypted_setting("cloudflare_token", token.clone())
                    .await?
            }
            None => serializer.write_raw_setting("cloudflare_token", ""),
        };

        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("default_limit", self.default_limit.to_compact_string())
            .write_raw_setting("cost_first", self.cost_first.to_compact_string())
            .write_raw_setting("cost_second", self.cost_second.to_compact_string())
            .write_raw_setting("cost_third", self.cost_third.to_compact_string())
            .write_raw_setting("cost_more", self.cost_more.to_compact_string())
            .write_raw_setting("blocked_names", &*self.blocked_names))
    }
}

pub struct AppSettingsSubdomainsDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsSubdomainsDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsSubdomains::default();
        let cloudflare_token = match deserializer.take_raw_setting("cloudflare_token") {
            Some(token) if !token.is_empty() => {
                Some(deserializer.database.decrypt_base64(token).await?)
            }
            _ => None,
        };

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsSubdomains {
            enabled: read!(enabled),
            cloudflare_token,
            default_limit: read!(default_limit),
            cost_first: read!(cost_first),
            cost_second: read!(cost_second),
            cost_third: read!(cost_third),
            cost_more: read!(cost_more),
            blocked_names: deserializer
                .take_raw_setting("blocked_names")
                .unwrap_or_default(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_go_by_the_subdomains_a_server_has() {
        let settings = AppSettingsSubdomains::default();

        assert_eq!(settings.cost(0), 1);
        assert_eq!(settings.cost(1), 3);
        assert_eq!(settings.cost(2), 8);
        assert_eq!(settings.cost(3), 15);
        assert_eq!(settings.cost(40), 15);
    }

    #[test]
    fn blocked_names_are_tidied_and_the_placeholder_keeps_the_token() {
        let mut settings = AppSettingsSubdomains {
            cloudflare_token: Some("old".into()),
            ..Default::default()
        };
        PayloadSubdomains {
            enabled: Some(true),
            cloudflare_token: Some(Some(crate::censor::CENSORED_PLACEHOLDER.into())),
            default_limit: None,
            cost_first: None,
            cost_second: None,
            cost_third: None,
            cost_more: None,
            blocked_names: Some(" Shop, ,test*\nPLAY ".into()),
        }
        .apply(&mut settings);

        assert_eq!(settings.cloudflare_token.as_deref(), Some("old"));
        assert_eq!(settings.blocked_names, "shop,test*,play");
        assert!(settings.active());
    }
}
