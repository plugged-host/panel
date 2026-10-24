// plugged.host: the neo settings section (Admin › Settings › neo, docs/features/neo.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Provider slots a panel can fill.
pub const MAX_PROVIDERS: u8 = 6;

/// The API a provider slot speaks.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NeoDriver {
    /// OpenAI-compatible `chat/completions` (OpenAI, Groq, OpenRouter, most gateways).
    #[default]
    Openai,
    /// Anthropic's Messages API.
    Anthropic,
    /// Google AI Studio's `generateContent` (Gemini and Gemma models).
    Gemini,
}

impl NeoDriver {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
}

/// Who may pick a model in the chat.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NeoAccess {
    #[default]
    Everyone,
    /// The first membership (and the second).
    Plus,
    /// The second membership only.
    Plusplus,
}

impl NeoAccess {
    fn rank(self) -> u8 {
        match self {
            Self::Everyone => 0,
            Self::Plus => 1,
            Self::Plusplus => 2,
        }
    }

    /// Whether an account on `plan` may pick a model open to `self`.
    pub fn allows(self, plan: NeoPlan) -> bool {
        plan.rank() >= self.rank()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Everyone => "everyone",
            Self::Plus => "plus",
            Self::Plusplus => "plusplus",
        }
    }
}

/// An account's membership as neo sees it (noprofit.space's `plan`).
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NeoPlan {
    #[default]
    Free,
    Plus,
    Plusplus,
}

impl NeoPlan {
    fn rank(self) -> u8 {
        match self {
            Self::Free => 0,
            Self::Plus => 1,
            Self::Plusplus => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Plus => "plus",
            Self::Plusplus => "plusplus",
        }
    }
}

/// One model people can pick in the chat, on one provider: its endpoint, key and model. Used
/// while the panel runs neo itself (noprofit.space keeps its own).
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct NeoProvider {
    /// 1 to [`MAX_PROVIDERS`]; the order models are listed and failed over in.
    pub slot: u8,
    pub enabled: bool,
    /// The model's name in the chat's picker.
    pub label: compact_str::CompactString,
    /// One line under it in the picker.
    #[serde(default)]
    pub description: compact_str::CompactString,
    pub driver: NeoDriver,
    /// The API's base URL (`https://api.openai.com/v1`); public https only.
    pub endpoint: compact_str::CompactString,
    /// Stored encrypted, shown as a placeholder.
    #[serde(default, serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub api_key: Option<compact_str::CompactString>,
    pub model: compact_str::CompactString,
    pub max_tokens: i32,
    /// Seconds one call may take.
    pub timeout: i32,
    /// Tokens a day this slot may use in all (0: no cap).
    #[serde(default)]
    pub daily_token_cap: i64,
    /// Extra request headers (not the key or content headers). The API shows the values of
    /// [secret-looking](header_is_secret) ones as [`HEADER_MASK`]; the database keeps them.
    #[serde(default, serialize_with = "serialize_headers")]
    pub extra_headers: BTreeMap<String, String>,
    /// Extra request fields (not the ones the driver sets).
    #[serde(default)]
    #[schema(value_type = Object)]
    pub extra_body: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub access: NeoAccess,
    /// How many times its tokens count in the windows (1 to 10).
    #[serde(default = "one")]
    pub multiplier: i32,
    /// Whether it sees images; unset: Gemini and Anthropic yes, OpenAI-style no.
    #[serde(default)]
    pub vision: Option<bool>,
}

fn one() -> i32 {
    1
}

/// What the value of a secret-looking extra header is shown as. Sent back unchanged, it keeps the
/// stored value.
pub const HEADER_MASK: &str = "••••";

/// Whether an extra header carries a secret, by its name (`cf-aig-authorization`, `X-Api-Token`).
pub fn header_is_secret(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    ["key", "token", "auth", "secret", "cookie", "session"]
        .iter()
        .any(|word| name.contains(word))
}

fn serialize_headers<S: serde::Serializer>(
    headers: &BTreeMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map(headers.iter().map(|(name, value)| {
        let shown = if header_is_secret(name) {
            HEADER_MASK
        } else {
            value.as_str()
        };
        (name, shown)
    }))
}

impl NeoProvider {
    /// On, with an endpoint, a key and a model.
    pub fn usable(&self) -> bool {
        self.enabled
            && !self.endpoint.is_empty()
            && self.api_key.as_deref().is_some_and(|key| !key.is_empty())
            && !self.model.is_empty()
    }

    pub fn sees_images(&self) -> bool {
        self.vision.unwrap_or(matches!(
            self.driver,
            NeoDriver::Gemini | NeoDriver::Anthropic
        ))
    }

    pub fn multiplier(&self) -> i64 {
        self.multiplier.clamp(1, 10) as i64
    }
}

/// neo, the assistant in the chat (and on Discord): whether it is on, what one message may cost,
/// and, while noprofit.space does not run it, its providers and usage windows.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsNeo {
    /// neo's master switch. Off: the chat is gone and every entry point refuses.
    pub enabled: bool,
    /// Model calls and tool runs one message may take before neo has to answer.
    pub max_steps: i32,
    /// The context (in tokens, estimated) sent with each model call.
    pub context_tokens: i32,
    /// Messages one person may send a minute.
    pub messages_per_minute: i32,
    /// Tool runs one person's neo may make a minute.
    pub tools_per_minute: i32,
    /// Deleting and other hard-to-undo actions always ask, even with approvals off.
    pub always_confirm_destructive: bool,
    /// Chats without activity for this many days are deleted.
    pub retention_days: i32,
    /// Where neo sends people for human help (the Discord invite, a help page); empty: no link.
    pub support_url: compact_str::CompactString,
    /// The addresses neo gives for legal complaints and for privacy requests; empty: none.
    pub legal_email: compact_str::CompactString,
    pub privacy_email: compact_str::CompactString,
    /// The AI providers named in neo's notice ("OpenAI and Anthropic"); empty: a general sentence.
    pub provider_disclosure: compact_str::CompactString,
    /// neo usage of the first membership, in % of the free limits, and what the second adds on
    /// top of it (125 × 125 = 156 %).
    pub plus_percent: i32,
    pub plusplus_percent: i32,

    /// Off: neo's own turns are refused while noprofit.space does not run neo.
    pub gateway_enabled: bool,
    /// The 5-hour and weekly token windows (input and output tokens).
    pub short_hours: i32,
    pub short_tokens: i64,
    pub long_days: i32,
    pub long_tokens: i64,
    /// A Discord account nobody linked gets windows of its own at this share of the limits.
    pub discord_unlinked_percent: i32,
    /// Seconds a provider that failed without saying how long is skipped.
    pub default_cooldown: i32,
    /// Every provider down for longer than this (seconds) is an outage, not a wait.
    pub max_busy_wait: i32,
    /// Seconds one turn may take across all providers.
    pub deadline: i32,
    /// Model calls in flight at once, panel-wide.
    pub max_concurrent: i32,
    /// Ask each provider once (kept 6 hours) whether the tools of a request reach the model; one
    /// that drops them gets the tools described in the prompt instead.
    pub tools_probe: bool,
    pub providers: Vec<NeoProvider>,
}

impl Default for AppSettingsNeo {
    fn default() -> Self {
        Self {
            enabled: false,
            max_steps: 25,
            context_tokens: 24000,
            messages_per_minute: 10,
            tools_per_minute: 60,
            always_confirm_destructive: true,
            retention_days: 30,
            support_url: compact_str::CompactString::default(),
            legal_email: compact_str::CompactString::default(),
            privacy_email: compact_str::CompactString::default(),
            provider_disclosure: compact_str::CompactString::default(),
            plus_percent: 125,
            plusplus_percent: 125,
            gateway_enabled: true,
            short_hours: 5,
            short_tokens: 3_000_000,
            long_days: 7,
            long_tokens: 15_000_000,
            discord_unlinked_percent: 50,
            default_cooldown: 60,
            max_busy_wait: 60,
            deadline: 80,
            max_concurrent: 4,
            tools_probe: true,
            providers: Vec::new(),
        }
    }
}

impl AppSettingsNeo {
    /// The slots as the database keeps them: without their keys (stored encrypted apart) and with
    /// the extra headers' real values (the API masks the secret ones).
    fn stored_providers(&self) -> Result<String, serde_json::Error> {
        let providers = self
            .providers
            .iter()
            .map(|p| {
                let mut value = serde_json::to_value(NeoProvider {
                    api_key: None,
                    ..p.clone()
                })?;
                value["extra_headers"] = serde_json::to_value(&p.extra_headers)?;
                Ok(value)
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?;

        serde_json::to_string(&providers)
    }

    /// The usable providers, in slot order.
    pub fn usable_providers(&self) -> impl Iterator<Item = &NeoProvider> {
        self.providers.iter().filter(|provider| provider.usable())
    }

    /// The share of the free limits an account on `plan` gets, in %.
    pub fn usage_percent(&self, plan: NeoPlan) -> i64 {
        let plus = self.plus_percent.max(100) as i64;
        let plusplus = self.plusplus_percent.max(100) as i64;

        match plan {
            NeoPlan::Free => 100,
            NeoPlan::Plus => plus.min(1000),
            NeoPlan::Plusplus => (plus * plusplus / 100).min(1000),
        }
    }
}

/// One provider slot in the neo tab: replaces the slot, except that an empty key or the
/// placeholder keeps the stored one.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadNeoProvider {
    #[garde(range(min = 1, max = MAX_PROVIDERS))]
    slot: u8,
    #[garde(skip)]
    enabled: bool,
    #[garde(length(chars, max = 64))]
    label: compact_str::CompactString,
    #[garde(length(chars, max = 255))]
    #[serde(default)]
    description: compact_str::CompactString,
    #[garde(skip)]
    driver: NeoDriver,
    #[garde(length(chars, max = 255))]
    endpoint: compact_str::CompactString,
    #[garde(inner(length(chars, max = 1024)))]
    #[serde(default)]
    api_key: Option<compact_str::CompactString>,
    #[garde(length(chars, max = 128))]
    model: compact_str::CompactString,
    #[garde(range(min = 256, max = 128000))]
    max_tokens: i32,
    #[garde(range(min = 5, max = 300))]
    timeout: i32,
    #[garde(range(min = 0))]
    #[serde(default)]
    daily_token_cap: i64,
    #[garde(length(max = 16))]
    #[serde(default)]
    extra_headers: BTreeMap<String, String>,
    #[garde(skip)]
    #[serde(default)]
    #[schema(value_type = Object)]
    extra_body: serde_json::Map<String, serde_json::Value>,
    #[garde(skip)]
    #[serde(default)]
    access: NeoAccess,
    #[garde(range(min = 1, max = 10))]
    #[serde(default = "one")]
    multiplier: i32,
    #[garde(skip)]
    #[serde(default)]
    vision: Option<bool>,
}

/// The neo tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadNeo {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 100)))]
    max_steps: Option<i32>,
    #[garde(inner(range(min = 4000, max = 400000)))]
    context_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 600)))]
    messages_per_minute: Option<i32>,
    #[garde(inner(range(min = 1, max = 600)))]
    tools_per_minute: Option<i32>,
    #[garde(skip)]
    always_confirm_destructive: Option<bool>,
    #[garde(inner(range(min = 1, max = 3650)))]
    retention_days: Option<i32>,
    #[garde(inner(length(chars, max = 255)))]
    support_url: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 255)))]
    legal_email: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 255)))]
    privacy_email: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 255)))]
    provider_disclosure: Option<compact_str::CompactString>,
    #[garde(inner(range(min = 100, max = 1000)))]
    plus_percent: Option<i32>,
    #[garde(inner(range(min = 100, max = 1000)))]
    plusplus_percent: Option<i32>,
    #[garde(skip)]
    gateway_enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 168)))]
    short_hours: Option<i32>,
    #[garde(inner(range(min = 0)))]
    short_tokens: Option<i64>,
    #[garde(inner(range(min = 1, max = 90)))]
    long_days: Option<i32>,
    #[garde(inner(range(min = 0)))]
    long_tokens: Option<i64>,
    #[garde(inner(range(min = 0, max = 100)))]
    discord_unlinked_percent: Option<i32>,
    #[garde(inner(range(min = 1, max = 3600)))]
    default_cooldown: Option<i32>,
    #[garde(inner(range(min = 1, max = 600)))]
    max_busy_wait: Option<i32>,
    #[garde(inner(range(min = 10, max = 300)))]
    deadline: Option<i32>,
    #[garde(inner(range(min = 1, max = 64)))]
    max_concurrent: Option<i32>,
    #[garde(skip)]
    tools_probe: Option<bool>,
    #[garde(length(max = MAX_PROVIDERS as usize), dive)]
    providers: Option<Vec<PayloadNeoProvider>>,
}

/// Why a neo payload was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PayloadError {
    SupportUrl,
    Endpoint(u8),
    DuplicateSlot(u8),
    Header(u8),
    HeaderInvalid(u8),
    HeaderMask(u8),
    Body(u8),
}

impl PayloadError {
    pub fn message(&self) -> String {
        match self {
            Self::SupportUrl => "the support link must start with https:// or http://".into(),
            Self::Endpoint(slot) => format!(
                "model {slot}: the endpoint must be a public https URL (no user, password or fragment)"
            ),
            Self::DuplicateSlot(slot) => format!("model {slot} is listed twice"),
            Self::Header(slot) => format!(
                "model {slot}: extra headers may not set the key, the content type or the host"
            ),
            Self::HeaderInvalid(slot) => format!(
                "model {slot}: extra header names are letters, digits and dashes, their values plain text on one line, {MAX_HEADERS_BYTES} bytes in all"
            ),
            Self::HeaderMask(slot) => format!(
                "model {slot}: {HEADER_MASK} keeps a stored header value, and this header has none; type its value"
            ),
            Self::Body(slot) => format!(
                "model {slot}: extra request fields may not set the fields the panel sets (model, messages, tools…), {MAX_BODY_BYTES} bytes and {MAX_BODY_DEPTH} levels at most"
            ),
        }
    }
}

/// The most a slot's extra headers may take, as JSON.
pub const MAX_HEADERS_BYTES: usize = 2048;
/// The most a slot's extra request fields may take, as JSON, and how deeply they may nest.
pub const MAX_BODY_BYTES: usize = 4096;
pub const MAX_BODY_DEPTH: usize = 4;

/// Request fields a slot's extra fields may not set (compared in lower case): the ones the
/// drivers set. Gemini's `generationConfig` is not among them, its call merges it.
pub const RESERVED_BODY_FIELDS: &[&str] = &[
    "model",
    "messages",
    "system",
    "tools",
    "tool_choice",
    "stream",
    "max_tokens",
    "max_completion_tokens",
    "n",
    "thinking",
    "contents",
    "systeminstruction",
    "system_instruction",
    "toolconfig",
    "tool_config",
];

/// Whether an extra header can be sent as it is: a name of letters, digits and dashes (64 at
/// most) and a value of visible ASCII and spaces (1024 at most), so no request fails on it.
pub fn header_allowed(name: &str, value: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && value.len() <= 1024
        && value
            .bytes()
            .all(|b| b == b'\t' || (0x20..0x7f).contains(&b))
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => 1 + map.values().map(json_depth).max().unwrap_or(0),
        serde_json::Value::Array(items) => 1 + items.iter().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Whether a slot's extra request fields leave the driver's own alone and stay small.
pub fn body_allowed(body: &serde_json::Map<String, serde_json::Value>) -> bool {
    !body
        .keys()
        .any(|key| RESERVED_BODY_FIELDS.contains(&key.to_ascii_lowercase().as_str()))
        && serde_json::to_string(body).is_ok_and(|json| json.len() <= MAX_BODY_BYTES)
        && 1 + body.values().map(json_depth).max().unwrap_or(0) <= MAX_BODY_DEPTH
}

/// Headers a slot's extra headers may not set.
pub const RESERVED_HEADERS: &[&str] = &[
    "authorization",
    "x-api-key",
    "x-goog-api-key",
    "content-type",
    "content-length",
    "host",
    "accept",
];

/// Whether a provider endpoint is acceptable: https, a host, no credentials or fragment, not
/// `localhost` or a `.localhost` / `.internal` name. Addresses are checked again when called
/// (the outbound resolver refuses internal ones).
pub fn endpoint_allowed(endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();

    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && !host.is_empty()
        && host != "localhost"
        && !host.ends_with(".localhost")
        && !host.ends_with(".internal")
}

impl PayloadNeo {
    pub fn apply(self, settings: &mut AppSettingsNeo) -> Result<(), PayloadError> {
        if let Some(url) = &self.support_url {
            let url = url.trim();
            if !url.is_empty() && !url.starts_with("https://") && !url.starts_with("http://") {
                return Err(PayloadError::SupportUrl);
            }
        }

        let providers = match self.providers {
            Some(payload) => {
                let mut providers: Vec<NeoProvider> = Vec::with_capacity(payload.len());
                for provider in payload {
                    if providers.iter().any(|p| p.slot == provider.slot) {
                        return Err(PayloadError::DuplicateSlot(provider.slot));
                    }
                    let endpoint = provider.endpoint.trim();
                    if !endpoint.is_empty() && !endpoint_allowed(endpoint) {
                        return Err(PayloadError::Endpoint(provider.slot));
                    }
                    if provider.extra_headers.keys().any(|name| {
                        RESERVED_HEADERS.contains(&name.trim().to_ascii_lowercase().as_str())
                    }) {
                        return Err(PayloadError::Header(provider.slot));
                    }
                    if !body_allowed(&provider.extra_body) {
                        return Err(PayloadError::Body(provider.slot));
                    }

                    let stored_slot = settings.providers.iter().find(|p| p.slot == provider.slot);
                    let mut extra_headers = BTreeMap::new();
                    for (name, value) in provider.extra_headers {
                        let name = name.trim().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        // the mask the API shows a secret value as keeps that value
                        let value = value.trim();
                        let value = if value == HEADER_MASK {
                            match stored_slot.and_then(|p| {
                                p.extra_headers
                                    .iter()
                                    .find(|(stored, _)| stored.eq_ignore_ascii_case(&name))
                            }) {
                                Some((_, stored)) => stored.clone(),
                                None => return Err(PayloadError::HeaderMask(provider.slot)),
                            }
                        } else {
                            value.to_string()
                        };
                        if !header_allowed(&name, &value) {
                            return Err(PayloadError::HeaderInvalid(provider.slot));
                        }
                        extra_headers.insert(name, value);
                    }
                    if !serde_json::to_string(&extra_headers)
                        .is_ok_and(|json| json.len() <= MAX_HEADERS_BYTES)
                    {
                        return Err(PayloadError::HeaderInvalid(provider.slot));
                    }

                    let stored = stored_slot.and_then(|p| p.api_key.clone());
                    let api_key = match provider.api_key {
                        Some(key)
                            if key.trim().is_empty()
                                || key == crate::censor::CENSORED_PLACEHOLDER =>
                        {
                            stored
                        }
                        Some(key) => Some(key.trim().into()),
                        None => None,
                    };

                    providers.push(NeoProvider {
                        slot: provider.slot,
                        enabled: provider.enabled,
                        label: provider.label.trim().into(),
                        description: provider.description.trim().into(),
                        driver: provider.driver,
                        endpoint: endpoint.trim_end_matches('/').into(),
                        api_key,
                        model: provider.model.trim().into(),
                        max_tokens: provider.max_tokens,
                        timeout: provider.timeout,
                        daily_token_cap: provider.daily_token_cap,
                        extra_headers,
                        extra_body: provider.extra_body,
                        access: provider.access,
                        multiplier: provider.multiplier,
                        vision: provider.vision,
                    });
                }
                providers.sort_by_key(|p| p.slot);
                Some(providers)
            }
            None => None,
        };

        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            enabled,
            max_steps,
            context_tokens,
            messages_per_minute,
            tools_per_minute,
            always_confirm_destructive,
            retention_days,
            plus_percent,
            plusplus_percent,
            gateway_enabled,
            short_hours,
            short_tokens,
            long_days,
            long_tokens,
            discord_unlinked_percent,
            default_cooldown,
            max_busy_wait,
            deadline,
            max_concurrent,
            tools_probe
        );
        macro_rules! text {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value.trim().into();
                })*
            };
        }
        text!(support_url, legal_email, privacy_email, provider_disclosure);

        if let Some(providers) = providers {
            settings.providers = providers;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsNeo {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        // the keys apart, encrypted; the slots without them
        let keys: BTreeMap<String, &str> = self
            .providers
            .iter()
            .filter_map(|p| p.api_key.as_deref().map(|key| (p.slot.to_string(), key)))
            .collect();
        let providers = self.stored_providers()?;

        let serializer = if keys.is_empty() {
            serializer.write_raw_setting("provider_keys", "")
        } else {
            serializer
                .write_raw_encrypted_setting("provider_keys", serde_json::to_string(&keys)?)
                .await?
        };

        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("max_steps", self.max_steps.to_compact_string())
            .write_raw_setting("context_tokens", self.context_tokens.to_compact_string())
            .write_raw_setting(
                "messages_per_minute",
                self.messages_per_minute.to_compact_string(),
            )
            .write_raw_setting(
                "tools_per_minute",
                self.tools_per_minute.to_compact_string(),
            )
            .write_raw_setting(
                "always_confirm_destructive",
                self.always_confirm_destructive.to_compact_string(),
            )
            .write_raw_setting("retention_days", self.retention_days.to_compact_string())
            .write_raw_setting("support_url", &*self.support_url)
            .write_raw_setting("legal_email", &*self.legal_email)
            .write_raw_setting("privacy_email", &*self.privacy_email)
            .write_raw_setting("provider_disclosure", &*self.provider_disclosure)
            .write_raw_setting("plus_percent", self.plus_percent.to_compact_string())
            .write_raw_setting(
                "plusplus_percent",
                self.plusplus_percent.to_compact_string(),
            )
            .write_raw_setting("gateway_enabled", self.gateway_enabled.to_compact_string())
            .write_raw_setting("short_hours", self.short_hours.to_compact_string())
            .write_raw_setting("short_tokens", self.short_tokens.to_compact_string())
            .write_raw_setting("long_days", self.long_days.to_compact_string())
            .write_raw_setting("long_tokens", self.long_tokens.to_compact_string())
            .write_raw_setting(
                "discord_unlinked_percent",
                self.discord_unlinked_percent.to_compact_string(),
            )
            .write_raw_setting(
                "default_cooldown",
                self.default_cooldown.to_compact_string(),
            )
            .write_raw_setting("max_busy_wait", self.max_busy_wait.to_compact_string())
            .write_raw_setting("deadline", self.deadline.to_compact_string())
            .write_raw_setting("max_concurrent", self.max_concurrent.to_compact_string())
            .write_raw_setting("tools_probe", self.tools_probe.to_compact_string())
            .write_raw_setting("providers", providers))
    }
}

pub struct AppSettingsNeoDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsNeoDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsNeo::default();

        let keys: BTreeMap<String, String> = match deserializer.take_raw_setting("provider_keys") {
            Some(value) if !value.is_empty() => {
                serde_json::from_str(&deserializer.database.decrypt_base64(value).await?)
                    .unwrap_or_default()
            }
            _ => BTreeMap::new(),
        };

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

        let mut providers: Vec<NeoProvider> = deserializer
            .take_raw_setting("providers")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for provider in &mut providers {
            provider.api_key = keys
                .get(&provider.slot.to_string())
                .map(|key| key.as_str().into());
        }

        Ok(Box::new(AppSettingsNeo {
            enabled: read!(enabled),
            max_steps: read!(max_steps),
            context_tokens: read!(context_tokens),
            messages_per_minute: read!(messages_per_minute),
            tools_per_minute: read!(tools_per_minute),
            always_confirm_destructive: read!(always_confirm_destructive),
            retention_days: read!(retention_days),
            support_url: text!(support_url),
            legal_email: text!(legal_email),
            privacy_email: text!(privacy_email),
            provider_disclosure: text!(provider_disclosure),
            plus_percent: read!(plus_percent),
            plusplus_percent: read!(plusplus_percent),
            gateway_enabled: read!(gateway_enabled),
            short_hours: read!(short_hours),
            short_tokens: read!(short_tokens),
            long_days: read!(long_days),
            long_tokens: read!(long_tokens),
            discord_unlinked_percent: read!(discord_unlinked_percent),
            default_cooldown: read!(default_cooldown),
            max_busy_wait: read!(max_busy_wait),
            deadline: read!(deadline),
            max_concurrent: read!(max_concurrent),
            tools_probe: read!(tools_probe),
            providers,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(slot: u8, key: Option<&str>) -> PayloadNeoProvider {
        PayloadNeoProvider {
            slot,
            enabled: true,
            label: "Model".into(),
            description: "".into(),
            driver: NeoDriver::Openai,
            endpoint: "https://api.example.com/v1/".into(),
            api_key: key.map(Into::into),
            model: "model-1".into(),
            max_tokens: 4096,
            timeout: 30,
            daily_token_cap: 0,
            extra_headers: BTreeMap::new(),
            extra_body: serde_json::Map::new(),
            access: NeoAccess::Everyone,
            multiplier: 1,
            vision: None,
        }
    }

    fn payload(providers: Vec<PayloadNeoProvider>) -> PayloadNeo {
        PayloadNeo {
            enabled: None,
            max_steps: None,
            context_tokens: None,
            messages_per_minute: None,
            tools_per_minute: None,
            always_confirm_destructive: None,
            retention_days: None,
            support_url: None,
            legal_email: None,
            privacy_email: None,
            provider_disclosure: None,
            plus_percent: None,
            plusplus_percent: None,
            gateway_enabled: None,
            short_hours: None,
            short_tokens: None,
            long_days: None,
            long_tokens: None,
            discord_unlinked_percent: None,
            default_cooldown: None,
            max_busy_wait: None,
            deadline: None,
            max_concurrent: None,
            tools_probe: None,
            providers: Some(providers),
        }
    }

    #[test]
    fn endpoints_are_public_https() {
        assert!(endpoint_allowed("https://api.example.com/v1"));
        assert!(!endpoint_allowed("http://api.example.com/v1"));
        assert!(!endpoint_allowed("https://localhost:8080"));
        assert!(!endpoint_allowed("https://ai.localhost"));
        assert!(!endpoint_allowed("https://user:pass@api.example.com"));
        assert!(!endpoint_allowed("https://api.example.com/#x"));
        assert!(!endpoint_allowed("not a url"));
    }

    #[test]
    fn a_saved_key_survives_the_placeholder_and_an_empty_field() {
        let mut settings = AppSettingsNeo::default();
        payload(vec![
            provider(1, Some("secret-1")),
            provider(2, Some("secret-2")),
        ])
        .apply(&mut settings)
        .unwrap();
        assert_eq!(settings.providers[0].endpoint, "https://api.example.com/v1");

        payload(vec![
            provider(2, Some(crate::censor::CENSORED_PLACEHOLDER)),
            provider(1, Some("")),
        ])
        .apply(&mut settings)
        .unwrap();
        assert_eq!(settings.providers[0].api_key.as_deref(), Some("secret-1"));
        assert_eq!(settings.providers[1].api_key.as_deref(), Some("secret-2"));

        payload(vec![provider(1, None)])
            .apply(&mut settings)
            .unwrap();
        assert_eq!(settings.providers.len(), 1);
        assert_eq!(settings.providers[0].api_key, None);
        assert!(!settings.providers[0].usable());
    }

    #[test]
    fn bad_slots_change_nothing() {
        let mut settings = AppSettingsNeo::default();
        let mut bad = provider(1, Some("key"));
        bad.endpoint = "http://10.0.0.1/v1".into();
        assert_eq!(
            payload(vec![bad]).apply(&mut settings),
            Err(PayloadError::Endpoint(1))
        );

        let mut header = provider(2, Some("key"));
        header
            .extra_headers
            .insert("Authorization".into(), "x".into());
        assert_eq!(
            payload(vec![header]).apply(&mut settings),
            Err(PayloadError::Header(2))
        );
        assert_eq!(
            payload(vec![provider(3, None), provider(3, None)]).apply(&mut settings),
            Err(PayloadError::DuplicateSlot(3))
        );
        assert!(settings.providers.is_empty());
    }

    #[test]
    fn extra_headers_and_fields_are_checked() {
        assert!(header_allowed("X-Title", "My panel"));
        assert!(header_allowed("HTTP-Referer", "https://panel.example.com"));
        assert!(!header_allowed("Bad Header", "x"));
        assert!(!header_allowed("X-Title", "two\r\nlines"));
        assert!(!header_allowed("X-Title", HEADER_MASK));
        assert!(!header_allowed("", "x"));
        assert!(header_is_secret("cf-aig-authorization"));
        assert!(header_is_secret("X-Session-Id"));
        assert!(!header_is_secret("HTTP-Referer"));

        let body = |value: serde_json::Value| value.as_object().unwrap().clone();
        assert!(body_allowed(&body(serde_json::json!({
            "reasoning_format": "hidden",
            "generationConfig": {"thinkingConfig": {"thinkingBudget": 0}},
        }))));
        assert!(body_allowed(&body(
            serde_json::json!({"a": {"b": {"c": [1]}}})
        )));
        assert!(!body_allowed(&body(
            serde_json::json!({"a": {"b": {"c": {"d": [1]}}}})
        )));
        assert!(!body_allowed(&body(serde_json::json!({"Messages": []}))));
        assert!(!body_allowed(&body(
            serde_json::json!({"systemInstruction": "x"})
        )));
        assert!(!body_allowed(&body(
            serde_json::json!({"note": "x".repeat(MAX_BODY_BYTES)})
        )));

        let mut settings = AppSettingsNeo::default();
        let mut bad = provider(1, Some("key"));
        bad.extra_headers.insert("X Title".into(), "x".into());
        assert_eq!(
            payload(vec![bad]).apply(&mut settings),
            Err(PayloadError::HeaderInvalid(1))
        );
        let mut reserved = provider(1, Some("key"));
        reserved
            .extra_body
            .insert("tools".into(), serde_json::json!([]));
        assert_eq!(
            payload(vec![reserved]).apply(&mut settings),
            Err(PayloadError::Body(1))
        );
        assert!(settings.providers.is_empty());
    }

    #[test]
    fn secret_headers_are_masked_and_kept() {
        let mut settings = AppSettingsNeo::default();
        let mut first = provider(1, Some("key"));
        first
            .extra_headers
            .insert("cf-aig-authorization".into(), "Bearer abc".into());
        first.extra_headers.insert("X-Title".into(), "Panel".into());
        payload(vec![first]).apply(&mut settings).unwrap();

        let shown = serde_json::to_value(&settings.providers[0]).unwrap();
        assert_eq!(shown["extra_headers"]["cf-aig-authorization"], HEADER_MASK);
        assert_eq!(shown["extra_headers"]["X-Title"], "Panel");
        let stored: serde_json::Value =
            serde_json::from_str(&settings.stored_providers().unwrap()).unwrap();
        assert_eq!(
            stored[0]["extra_headers"]["cf-aig-authorization"],
            "Bearer abc"
        );
        assert!(stored[0]["api_key"].is_null());

        // sent back as the API shows it, the value stays
        let mut again = provider(1, Some(""));
        again
            .extra_headers
            .insert("CF-AIG-Authorization".into(), HEADER_MASK.into());
        payload(vec![again]).apply(&mut settings).unwrap();
        assert_eq!(
            settings.providers[0].extra_headers["CF-AIG-Authorization"],
            "Bearer abc"
        );
        assert_eq!(settings.providers[0].api_key.as_deref(), Some("key"));

        let mut new = provider(2, None);
        new.extra_headers
            .insert("x-api-token".into(), HEADER_MASK.into());
        assert_eq!(
            payload(vec![new]).apply(&mut settings),
            Err(PayloadError::HeaderMask(2))
        );
    }

    #[test]
    fn memberships_raise_the_limits() {
        let settings = AppSettingsNeo::default();
        assert_eq!(settings.usage_percent(NeoPlan::Free), 100);
        assert_eq!(settings.usage_percent(NeoPlan::Plus), 125);
        assert_eq!(settings.usage_percent(NeoPlan::Plusplus), 156);
        assert!(NeoAccess::Plus.allows(NeoPlan::Plusplus));
        assert!(!NeoAccess::Plusplus.allows(NeoPlan::Plus));
        assert!(NeoAccess::Everyone.allows(NeoPlan::Free));
    }
}
