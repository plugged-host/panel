// plugged.host: the devboxes settings section (docs/features/devboxes.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Devboxes: a private Linux box next to a game server, with its files at `/server`, that VS Code
/// connects to over SSH or opens in the browser (shared/src/devboxes).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsDevboxes {
    /// Players see the VS Code page (once the egg is chosen too).
    pub enabled: bool,
    /// The devbox egg (`docs/eggs/devbox.json`, imported into any nest).
    pub egg_uuid: Option<uuid::Uuid>,
    /// The image devboxes run; empty: the egg's first image. A digest (`…@sha256:…`) pins one
    /// build. Running devboxes pick a change up at their next start.
    pub image: compact_str::CompactString,
    /// Only members (Admin › Settings › Economy › Memberships) get a devbox; off: everyone.
    pub members_only: bool,
    /// Resources of a devbox, on top of its game server's: CPU in % (100 = one thread), MiB.
    pub cpu: i32,
    pub memory: i32,
    pub disk: i32,
    /// The same for a devbox paid by someone with the second membership.
    pub premium_cpu: i32,
    pub premium_memory: i32,
    pub premium_disk: i32,
    /// Stopped after this many minutes without a connection.
    pub idle_minutes: i32,
    /// Stopped this many hours after it was started, connected or not.
    pub max_hours: i32,
    /// Devboxes one person may have running at once: their own, and the shared ones on servers
    /// they own.
    pub running_per_user: i32,
    /// A devbox nobody used for this many days is deleted with its home.
    pub cleanup_days: i32,
    /// Where devboxes reach the panel; empty: the panel's URL (Admin › Settings › Application).
    pub panel_url: compact_str::CompactString,
    /// VS Code in the browser: each devbox answers at `https://<slug>.<web_domain>` through the
    /// edge. Empty switches it off. Never the panel's domain or one its cookie reaches.
    pub web_domain: compact_str::CompactString,
    /// The edge's public IPv4, which each devbox's proxied DNS record points at.
    pub web_origin_ip: compact_str::CompactString,
    /// A Cloudflare API token with Zone › DNS › Edit on the web domain's zone; empty uses the
    /// subdomains token. Stored encrypted, shown as a placeholder.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub cloudflare_token: Option<compact_str::CompactString>,
    /// The devbox CA and the edge's client certificate (PEM), made by Create Certificates. The
    /// keys are stored encrypted and never sent to the browser.
    pub ca_cert: Option<compact_str::CompactString>,
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub ca_key: Option<compact_str::CompactString>,
    pub edge_cert: Option<compact_str::CompactString>,
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub edge_key: Option<compact_str::CompactString>,
    /// The account that owns every devbox server (made by the panel, suspended: nobody signs in
    /// as it).
    pub system_user_uuid: Option<uuid::Uuid>,
}

impl Default for AppSettingsDevboxes {
    fn default() -> Self {
        Self {
            enabled: false,
            egg_uuid: None,
            image: compact_str::CompactString::default(),
            members_only: true,
            cpu: 200,
            memory: 2048,
            disk: 5120,
            premium_cpu: 400,
            premium_memory: 4096,
            premium_disk: 10240,
            idle_minutes: 15,
            max_hours: 12,
            running_per_user: 2,
            cleanup_days: 14,
            panel_url: compact_str::CompactString::default(),
            web_domain: compact_str::CompactString::default(),
            web_origin_ip: compact_str::CompactString::default(),
            cloudflare_token: None,
            ca_cert: None,
            ca_key: None,
            edge_cert: None,
            edge_key: None,
            system_user_uuid: None,
        }
    }
}

impl AppSettingsDevboxes {
    /// Whether players are offered devboxes.
    pub fn active(&self) -> bool {
        self.enabled && self.egg_uuid.is_some()
    }

    /// Whether the browser version is set up: a domain and the certificates.
    pub fn web_ready(&self) -> bool {
        !self.web_domain.is_empty()
            && self.ca_cert.is_some()
            && self.ca_key.is_some()
            && self.edge_cert.is_some()
            && self.edge_key.is_some()
    }
}

/// The devboxes tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadDevboxes {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(skip)]
    #[serde(default, with = "::serde_with::rust::double_option")]
    egg_uuid: Option<Option<uuid::Uuid>>,
    #[garde(inner(length(chars, max = 255)))]
    image: Option<compact_str::CompactString>,
    #[garde(skip)]
    members_only: Option<bool>,
    #[garde(inner(range(min = 25, max = 3200)))]
    cpu: Option<i32>,
    #[garde(inner(range(min = 256, max = 65536)))]
    memory: Option<i32>,
    #[garde(inner(range(min = 512, max = 262144)))]
    disk: Option<i32>,
    #[garde(inner(range(min = 25, max = 3200)))]
    premium_cpu: Option<i32>,
    #[garde(inner(range(min = 256, max = 65536)))]
    premium_memory: Option<i32>,
    #[garde(inner(range(min = 512, max = 262144)))]
    premium_disk: Option<i32>,
    #[garde(inner(range(min = 1, max = 1440)))]
    idle_minutes: Option<i32>,
    #[garde(inner(range(min = 1, max = 168)))]
    max_hours: Option<i32>,
    #[garde(inner(range(min = 1, max = 20)))]
    running_per_user: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    cleanup_days: Option<i32>,
    #[garde(inner(length(chars, max = 255)))]
    panel_url: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 253)))]
    web_domain: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 45)))]
    web_origin_ip: Option<compact_str::CompactString>,
    /// An empty string or the placeholder keeps the stored token, `null` removes it.
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    cloudflare_token: Option<Option<compact_str::CompactString>>,
    /// Makes a new devbox CA and edge certificate (replacing the old ones).
    #[garde(skip)]
    create_certificates: Option<bool>,
}

/// Why a devboxes payload was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PayloadError {
    PanelUrl,
    WebDomain,
    OriginIp,
    Certificates,
}

impl PayloadError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::PanelUrl => "the panel URL must start with https:// or http://",
            Self::WebDomain => {
                "the browser domain must be a domain of its own, not the panel's or one under or above it"
            }
            Self::OriginIp => "the edge address must be an IPv4 address",
            Self::Certificates => "the certificates could not be made",
        }
    }
}

impl PayloadDevboxes {
    /// Applies the payload; `panel_host` is the panel's host name, which the browser domain may
    /// not share a registrable domain with, `app_name` labels new certificates.
    pub fn apply(
        self,
        settings: &mut AppSettingsDevboxes,
        panel_host: Option<&str>,
        app_name: &str,
    ) -> Result<(), PayloadError> {
        if let Some(url) = &self.panel_url {
            let url = url.trim();
            if !url.is_empty() && !url.starts_with("https://") && !url.starts_with("http://") {
                return Err(PayloadError::PanelUrl);
            }
        }
        let web_domain = self
            .web_domain
            .as_ref()
            .map(|domain| domain.trim().trim_matches('.').to_lowercase());
        if let Some(domain) = &web_domain
            && !domain.is_empty()
            && !web_domain_allowed(domain, panel_host)
        {
            return Err(PayloadError::WebDomain);
        }
        if let Some(ip) = &self.web_origin_ip {
            let ip = ip.trim();
            if !ip.is_empty() && ip.parse::<std::net::Ipv4Addr>().is_err() {
                return Err(PayloadError::OriginIp);
            }
        }

        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            enabled,
            members_only,
            cpu,
            memory,
            disk,
            premium_cpu,
            premium_memory,
            premium_disk,
            idle_minutes,
            max_hours,
            running_per_user,
            cleanup_days
        );

        if let Some(egg_uuid) = self.egg_uuid {
            settings.egg_uuid = egg_uuid;
        }
        if let Some(image) = self.image {
            settings.image = image.trim().into();
        }
        if let Some(url) = self.panel_url {
            settings.panel_url = url.trim().trim_end_matches('/').into();
        }
        if let Some(domain) = web_domain {
            settings.web_domain = domain.into();
        }
        if let Some(ip) = self.web_origin_ip {
            settings.web_origin_ip = ip.trim().into();
        }
        match self.cloudflare_token {
            Some(Some(token))
                if token.trim().is_empty() || token == crate::censor::CENSORED_PLACEHOLDER => {}
            Some(token) => settings.cloudflare_token = token.map(|token| token.trim().into()),
            None => {}
        }
        if self.create_certificates == Some(true) {
            let authority = crate::devboxes::pki::create_authority(app_name).map_err(|err| {
                tracing::error!("failed to make the devbox certificates: {err:#}");
                PayloadError::Certificates
            })?;
            settings.ca_cert = Some(authority.ca_cert.into());
            settings.ca_key = Some(authority.ca_key.into());
            settings.edge_cert = Some(authority.edge_cert.into());
            settings.edge_key = Some(authority.edge_key.into());
        }

        Ok(())
    }
}

/// Second-level labels under which country domains register (`example.co.uk`).
const COUNTRY_SECOND_LEVELS: &[&str] = &[
    "ac", "co", "com", "edu", "go", "gob", "gov", "ltd", "me", "mil", "ne", "net", "nom", "or",
    "org", "plc", "sch",
];

/// The registrable domain of a host name (`panel.example.com` → `example.com`,
/// `panel.example.co.uk` → `example.co.uk`); `None` for an IP address or a single label.
pub fn registrable_domain(host: &str) -> Option<String> {
    let host = host.trim().trim_matches('.').to_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
        return None;
    }

    let labels: Vec<&str> = host.split('.').collect();
    let count = labels.len();
    let keep = if count >= 3
        && labels[count - 1].len() == 2
        && COUNTRY_SECOND_LEVELS.contains(&labels[count - 2])
    {
        3
    } else {
        2
    };

    Some(labels[count.saturating_sub(keep)..].join("."))
}

/// Whether `domain` may serve devboxes in the browser: a domain name, and not the panel's
/// registrable domain or one under or above it (anything a devbox serves would get the panel's
/// cookies).
pub fn web_domain_allowed(domain: &str, panel_host: Option<&str>) -> bool {
    let domain = domain.trim().trim_matches('.').to_lowercase();
    let valid = domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
    if !valid {
        return false;
    }

    match panel_host.and_then(registrable_domain) {
        Some(panel) => {
            domain != panel
                && !domain.ends_with(&format!(".{panel}"))
                && !panel.ends_with(&format!(".{domain}"))
        }
        None => true,
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsDevboxes {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let mut serializer = serializer;
        for (key, value) in [
            ("cloudflare_token", &self.cloudflare_token),
            ("ca_key", &self.ca_key),
            ("edge_key", &self.edge_key),
        ] {
            serializer = match value {
                Some(value) => {
                    serializer
                        .write_raw_encrypted_setting(key, value.clone())
                        .await?
                }
                None => serializer.write_raw_setting(key, ""),
            };
        }

        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting(
                "egg_uuid",
                self.egg_uuid
                    .map(|uuid| uuid.to_compact_string())
                    .unwrap_or_default(),
            )
            .write_raw_setting("image", &*self.image)
            .write_raw_setting("members_only", self.members_only.to_compact_string())
            .write_raw_setting("cpu", self.cpu.to_compact_string())
            .write_raw_setting("memory", self.memory.to_compact_string())
            .write_raw_setting("disk", self.disk.to_compact_string())
            .write_raw_setting("premium_cpu", self.premium_cpu.to_compact_string())
            .write_raw_setting("premium_memory", self.premium_memory.to_compact_string())
            .write_raw_setting("premium_disk", self.premium_disk.to_compact_string())
            .write_raw_setting("idle_minutes", self.idle_minutes.to_compact_string())
            .write_raw_setting("max_hours", self.max_hours.to_compact_string())
            .write_raw_setting(
                "running_per_user",
                self.running_per_user.to_compact_string(),
            )
            .write_raw_setting("cleanup_days", self.cleanup_days.to_compact_string())
            .write_raw_setting("panel_url", &*self.panel_url)
            .write_raw_setting("web_domain", &*self.web_domain)
            .write_raw_setting("web_origin_ip", &*self.web_origin_ip)
            .write_raw_setting("ca_cert", self.ca_cert.as_deref().unwrap_or_default())
            .write_raw_setting("edge_cert", self.edge_cert.as_deref().unwrap_or_default())
            .write_raw_setting(
                "system_user_uuid",
                self.system_user_uuid
                    .map(|uuid| uuid.to_compact_string())
                    .unwrap_or_default(),
            ))
    }
}

pub struct AppSettingsDevboxesDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsDevboxesDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsDevboxes::default();

        let mut secrets = Vec::with_capacity(3);
        for key in ["cloudflare_token", "ca_key", "edge_key"] {
            secrets.push(match deserializer.take_raw_setting(key) {
                Some(value) if !value.is_empty() => {
                    Some(deserializer.database.decrypt_base64(value).await?)
                }
                _ => None,
            });
        }
        let mut secrets = secrets.into_iter();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }
        macro_rules! text {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .unwrap_or_default()
            };
        }
        macro_rules! optional {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .filter(|value| !value.is_empty())
            };
        }

        Ok(Box::new(AppSettingsDevboxes {
            enabled: read!(enabled),
            egg_uuid: optional!(egg_uuid).and_then(|uuid| uuid.parse().ok()),
            image: text!(image),
            members_only: read!(members_only),
            cpu: read!(cpu),
            memory: read!(memory),
            disk: read!(disk),
            premium_cpu: read!(premium_cpu),
            premium_memory: read!(premium_memory),
            premium_disk: read!(premium_disk),
            idle_minutes: read!(idle_minutes),
            max_hours: read!(max_hours),
            running_per_user: read!(running_per_user),
            cleanup_days: read!(cleanup_days),
            panel_url: text!(panel_url),
            web_domain: text!(web_domain),
            web_origin_ip: text!(web_origin_ip),
            cloudflare_token: secrets.next().flatten(),
            ca_cert: optional!(ca_cert),
            ca_key: secrets.next().flatten(),
            edge_cert: optional!(edge_cert),
            edge_key: secrets.next().flatten(),
            system_user_uuid: optional!(system_user_uuid).and_then(|uuid| uuid.parse().ok()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrable_domains() {
        assert_eq!(
            registrable_domain("panel.example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            registrable_domain("panel.example.co.uk").as_deref(),
            Some("example.co.uk")
        );
        assert_eq!(registrable_domain("192.0.2.10"), None);
        assert_eq!(registrable_domain("localhost"), None);
    }

    #[test]
    fn the_browser_domain_never_shares_the_panels_cookies() {
        let panel = Some("panel.example.com");

        assert!(web_domain_allowed("example.net", panel));
        assert!(!web_domain_allowed("example.com", panel));
        assert!(!web_domain_allowed("dev.example.com", panel));
        assert!(!web_domain_allowed("com", panel));
        assert!(!web_domain_allowed("exa mple.net", panel));
        assert!(web_domain_allowed("example.net", None));
    }

    #[test]
    fn a_payload_with_a_bad_value_changes_nothing() {
        let mut settings = AppSettingsDevboxes::default();
        let payload = PayloadDevboxes {
            enabled: Some(true),
            egg_uuid: None,
            image: None,
            members_only: None,
            cpu: None,
            memory: None,
            disk: None,
            premium_cpu: None,
            premium_memory: None,
            premium_disk: None,
            idle_minutes: None,
            max_hours: None,
            running_per_user: None,
            cleanup_days: None,
            panel_url: None,
            web_domain: Some("dev.example.com".into()),
            web_origin_ip: None,
            cloudflare_token: None,
            create_certificates: Some(true),
        };

        assert_eq!(
            payload.apply(&mut settings, Some("panel.example.com"), "Example"),
            Err(PayloadError::WebDomain)
        );
        assert!(!settings.enabled);
        assert!(settings.ca_cert.is_none(), "nor are certificates made");
    }
}
