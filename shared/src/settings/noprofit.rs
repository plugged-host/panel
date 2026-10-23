// plugged.host: the noprofit.space settings section (Admin › Settings › Accounts › noprofit.space).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::prelude::StringExt;
use compact_str::ToCompactString;
use serde::{Deserialize, Serialize, Serializer};
use utoipa::ToSchema;

/// How noprofit.space is connected. noprofit.space is optional: while it is not
/// [connected](Self::connected), the panel signs people in on its own. The secrets are stored
/// encrypted and never leave the panel: the API shows them as an empty string when they are set.
///
/// Before these settings are saved for the first time, the `NOPROFIT_*` environment variables
/// of earlier releases fill them in.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsNoprofit {
    /// Whether noprofit.space owns sign-in.
    pub enabled: bool,
    /// noprofit.space's accounts app, without a trailing slash.
    pub accounts_url: compact_str::CompactString,
    /// The sign-in client noprofit.space knows this panel as.
    pub sso_client: compact_str::CompactString,
    /// The audience of the sign-in tokens, `{sso_client}:sso` when not set.
    pub sso_audience: Option<compact_str::CompactString>,
    /// Where noprofit.space sends the tokens, `{app url}/api/auth/noprofit/callback` when not set.
    pub sso_callback_url: Option<compact_str::CompactString>,
    /// Only noprofit.space admins may sign in.
    pub admins_only: bool,
    /// A first sign-in takes over the panel user with the same email.
    pub link_by_email: bool,

    /// The secret the sign-in tokens are signed with (at least 32 characters).
    #[serde(serialize_with = "serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub sso_secret: Option<compact_str::CompactString>,
    /// The secret of the server-to-server API (account sync, SFTP passwords), at least 32
    /// characters. Without it there is no account sync.
    #[serde(serialize_with = "serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub api_secret: Option<compact_str::CompactString>,
}

/// A set secret is shown as [`crate::censor::CENSORED_PLACEHOLDER`], an unset one as `null`.
pub(crate) fn serialize_secret<S: Serializer>(
    secret: &Option<compact_str::CompactString>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match secret {
        Some(_) => serializer.serialize_some(crate::censor::CENSORED_PLACEHOLDER),
        None => serializer.serialize_none(),
    }
}

impl AppSettingsNoprofit {
    /// The noprofit.space connection, or `None` when the panel signs people in on its own
    /// (turned off, or the accounts URL, the client or a long enough sign-in secret missing).
    pub fn config(&self) -> Option<crate::noprofit::Config> {
        crate::noprofit::Config::from_settings(self)
    }

    /// Whether noprofit.space owns sign-in. Password sign-in is then for admins only, and the
    /// panel's own OAuth providers and security keys are off.
    #[inline]
    pub fn connected(&self) -> bool {
        self.config().is_some()
    }
}

fn env_string(key: &str) -> Option<compact_str::CompactString> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_compact_string())
        .filter(|value| !value.is_empty())
}

fn env_flag(key: &str) -> Option<bool> {
    env_string(key).map(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsNoprofit {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let mut serializer = serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("accounts_url", &*self.accounts_url)
            .write_raw_setting("sso_client", &*self.sso_client)
            .write_raw_setting(
                "sso_audience",
                self.sso_audience.clone().unwrap_or_default(),
            )
            .write_raw_setting(
                "sso_callback_url",
                self.sso_callback_url.clone().unwrap_or_default(),
            )
            .write_raw_setting("admins_only", self.admins_only.to_compact_string())
            .write_raw_setting("link_by_email", self.link_by_email.to_compact_string());

        // An unset secret is stored empty, so the environment no longer fills it in.
        for (key, secret) in [
            ("sso_secret", &self.sso_secret),
            ("api_secret", &self.api_secret),
        ] {
            serializer = match secret {
                Some(secret) => {
                    serializer
                        .write_raw_encrypted_setting(key, secret.clone())
                        .await?
                }
                None => serializer.write_raw_setting(key, ""),
            };
        }

        Ok(serializer)
    }
}

pub struct AppSettingsNoprofitDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsNoprofitDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let mut secrets = Vec::with_capacity(2);
        for (key, env_key) in [
            ("sso_secret", "NOPROFIT_SSO_SECRET"),
            ("api_secret", "NOPROFIT_API_SECRET"),
        ] {
            secrets.push(match deserializer.take_raw_setting(key) {
                Some(secret) if secret.is_empty() => None,
                Some(secret) => Some(deserializer.database.decrypt_base64(secret).await?),
                None => env_string(env_key),
            });
        }
        let api_secret = secrets.pop().flatten();
        let sso_secret = secrets.pop().flatten();

        let accounts_url = deserializer
            .take_raw_setting("accounts_url")
            .or_else(|| env_string("NOPROFIT_ACCOUNTS_URL"));

        Ok(Box::new(AppSettingsNoprofit {
            enabled: deserializer
                .take_raw_setting("enabled")
                .map(|s| s == "true")
                .unwrap_or_else(|| accounts_url.is_some() && sso_secret.is_some()),
            accounts_url: accounts_url.unwrap_or_else(|| "https://accounts.noprofit.space".into()),
            sso_client: deserializer
                .take_raw_setting("sso_client")
                .or_else(|| env_string("NOPROFIT_SSO_CLIENT"))
                .unwrap_or_default(),
            sso_audience: deserializer
                .take_raw_setting("sso_audience")
                .or_else(|| env_string("NOPROFIT_SSO_AUDIENCE"))
                .and_then(|s| s.into_optional()),
            sso_callback_url: deserializer
                .take_raw_setting("sso_callback_url")
                .or_else(|| env_string("NOPROFIT_SSO_CALLBACK"))
                .and_then(|s| s.into_optional()),
            admins_only: deserializer
                .take_raw_setting("admins_only")
                .map(|s| s == "true")
                .or_else(|| env_flag("NOPROFIT_ADMINS_ONLY"))
                .unwrap_or(false),
            link_by_email: deserializer
                .take_raw_setting("link_by_email")
                .map(|s| s == "true")
                .or_else(|| env_flag("NOPROFIT_LINK_BY_EMAIL"))
                .unwrap_or(true),
            sso_secret: sso_secret.and_then(|s| s.into_optional()),
            api_secret: api_secret.and_then(|s| s.into_optional()),
        }))
    }
}
