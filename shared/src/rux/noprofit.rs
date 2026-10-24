//! plugged.host: noprofit.space's rux routes (`/api/v1/internal/rux/*`), while noprofit.space is
//! connected: who a Discord account is (and whether they are an admin there), rux's settings as
//! noprofit.space keeps them, and, for the tools, giveaway payouts and credit grants.
//!
//! The routes are signed like the panel's other noprofit.space calls (`X-NP-*`), with the rux
//! section's own secret when it is set (noprofit.space checks these routes against its sync
//! secret), else with the noprofit.space API secret.
//!
//! The settings are read for every message the bot sees, so a fetch is reused for a minute: in
//! memory, and through an encrypted copy in the cache that every instance shares. The copy is also
//! what rux keeps running on while noprofit.space cannot be reached; a failed fetch is not tried
//! again for 30 seconds. With no copy at all, the panel's own settings apply.

use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::LazyLock, time::Duration};

pub const BASE: &str = "/api/v1/internal/rux";
pub const SETTINGS_PATH: &str = "/api/v1/internal/rux/settings";
/// Seconds a fetched copy of the settings is used before it is fetched again.
pub const REFRESH: i64 = 60;
/// Seconds before a failed fetch is tried again.
pub const RETRY_AFTER: i64 = 30;
const COPY_KEY: &str = "rux::noprofit::settings";
const COPY_TTL: u64 = 30 * 86400;

/// noprofit.space could not answer (not configured, unreachable, or an error), with a message
/// the model and the logs can use.
#[derive(Debug, Clone)]
pub struct Unavailable(pub String);

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unavailable {}

/// The secret these routes are signed with, when one is usable.
fn own_secret(secret: Option<&str>) -> Option<&str> {
    secret
        .map(str::trim)
        .filter(|secret| secret.chars().count() >= crate::settings::rux::MIN_SECRET_CHARS)
}

/// Whether the panel can call noprofit.space's rux routes at all (no network involved).
pub fn configured(config: &crate::noprofit::Config, secret: Option<&str>) -> bool {
    own_secret(secret).is_some() || config.sync_enabled()
}

fn headers(
    config: &crate::noprofit::Config,
    secret: Option<&str>,
    method: &str,
    path: &str,
    body: &[u8],
) -> Option<[(&'static str, String); 3]> {
    let Some(secret) = own_secret(secret) else {
        return config.sign_request(method, path, body);
    };
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 32);

    Some([
        ("X-NP-Timestamp", timestamp.clone()),
        ("X-NP-Nonce", nonce.clone()),
        (
            "X-NP-Signature",
            crate::noprofit::request_signature(secret, method, path, &timestamp, &nonce, body),
        ),
    ])
}

/// One signed call: the status and the answer's JSON (`null` when it is not JSON).
async fn call(
    http: &reqwest::Client,
    config: &crate::noprofit::Config,
    secret: Option<&str>,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
    timeout: u64,
) -> Result<(reqwest::StatusCode, Value), Unavailable> {
    let raw = match body {
        Some(body) => serde_json::to_vec(body).map_err(|err| Unavailable(err.to_string()))?,
        None => Vec::new(),
    };
    let Some(headers) = headers(config, secret, method.as_str(), path, &raw) else {
        return Err(Unavailable(
            "noprofit.space is not configured for rux on this panel (no rux or API secret)".into(),
        ));
    };

    let mut request = http
        .request(method, format!("{}{path}", config.accounts_url))
        .timeout(Duration::from_secs(timeout))
        .header("Accept", "application/json");
    if body.is_some() {
        request = request.header("Content-Type", "application/json").body(raw);
    }
    for (name, value) in headers {
        request = request.header(name, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| Unavailable(format!("noprofit.space could not be reached: {err}")))?;
    let status = response.status();
    let json = response.json::<Value>().await.unwrap_or(Value::Null);

    Ok((status, json))
}

/// A noprofit.space account linked to a Discord account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteAccount {
    pub account_id: i64,
    pub name: String,
    pub is_admin: bool,
}

/// The linked accounts in an `/accounts` answer, by Discord id.
pub fn parse_accounts(answer: &Value) -> HashMap<String, RemoteAccount> {
    let mut found = HashMap::new();

    for row in answer["accounts"].as_array().into_iter().flatten() {
        let discord_id = match &row["discord_id"] {
            Value::String(id) => id.clone(),
            Value::Number(id) => id.to_string(),
            _ => continue,
        };
        let Some(account_id) = row["account_id"]
            .as_i64()
            .or_else(|| row["account_id"].as_str().and_then(|id| id.parse().ok()))
        else {
            continue;
        };

        found.insert(
            discord_id,
            RemoteAccount {
                account_id,
                name: row["name"].as_str().unwrap_or_default().to_string(),
                is_admin: row["is_admin"].as_bool() == Some(true),
            },
        );
    }

    found
}

/// The rux routes, with the settings that sign them.
pub struct Client {
    http: reqwest::Client,
    config: crate::noprofit::Config,
    secret: Option<String>,
}

impl Client {
    /// The routes as the settings describe them; `None` while noprofit.space is not connected.
    pub async fn load(state: &crate::State) -> Result<Option<Self>, anyhow::Error> {
        let settings = state.settings.get().await?;
        let Some(config) = settings.noprofit.config() else {
            return Ok(None);
        };

        Ok(Some(Self {
            http: state.client.clone(),
            config,
            secret: settings.rux.noprofit_secret.as_ref().map(|s| s.to_string()),
        }))
    }

    pub fn configured(&self) -> bool {
        configured(&self.config, self.secret.as_deref())
    }

    pub fn accounts_url(&self) -> &str {
        &self.config.accounts_url
    }

    /// A signed POST to `{BASE}{path}`; an answer that is not a success is [`Unavailable`] with
    /// noprofit.space's `error` (or `message`) in brackets.
    pub async fn post(&self, path: &str, body: &Value, timeout: u64) -> Result<Value, Unavailable> {
        let (status, json) = call(
            &self.http,
            &self.config,
            self.secret.as_deref(),
            reqwest::Method::POST,
            &format!("{BASE}{path}"),
            Some(body),
            timeout,
        )
        .await?;

        if !status.is_success() {
            let error = json["error"]
                .as_str()
                .or(json["message"].as_str())
                .unwrap_or_default();
            return Err(Unavailable(if error.is_empty() {
                format!("noprofit.space answered HTTP {}.", status.as_u16())
            } else {
                format!(
                    "noprofit.space answered HTTP {} ({error}).",
                    status.as_u16()
                )
            }));
        }

        Ok(json)
    }

    /// The linked ones among these Discord ids, by Discord id.
    pub async fn accounts(
        &self,
        discord_ids: &[String],
    ) -> Result<HashMap<String, RemoteAccount>, Unavailable> {
        let mut found = HashMap::new();
        for chunk in discord_ids.chunks(1000) {
            let answer = self
                .post("/accounts", &serde_json::json!({ "discord_ids": chunk }), 8)
                .await?;
            found.extend(parse_accounts(&answer));
        }

        Ok(found)
    }

    /// The account linked to one Discord id; `None` when there is none.
    pub async fn account(&self, discord_id: &str) -> Result<Option<RemoteAccount>, Unavailable> {
        Ok(self
            .accounts(&[discord_id.to_string()])
            .await?
            .remove(discord_id))
    }
}

/// rux's settings as noprofit.space answered them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsCopy {
    /// When it was fetched (unix seconds).
    pub fetched_at: i64,
    /// noprofit.space keeps rux's settings (they were saved there once).
    pub configured: bool,
    pub settings: Option<Value>,
    pub admin_url: Option<String>,
    pub saved_at: Option<String>,
    pub saved_by: Option<String>,
}

/// A `GET …/rux/settings` answer as a copy, or why it is not taken. A configured answer without
/// the settings rux needs (`enabled`, `guild_id`) is refused, so the last copy stays.
pub fn parse_settings(answer: &Value, fetched_at: i64) -> Result<SettingsCopy, String> {
    let Some(configured) = answer["configured"].as_bool() else {
        return Err(
            "noprofit.space answered without saying whether it keeps rux's settings".into(),
        );
    };
    let settings = &answer["settings"];
    if configured
        && (!settings.is_object()
            || !settings["enabled"].is_boolean()
            || !settings["guild_id"].is_string())
    {
        return Err("noprofit.space answered without rux's settings".into());
    }
    let text = |value: &Value| {
        value
            .as_str()
            .filter(|text| !text.is_empty())
            .map(|text| text.chars().take(512).collect::<String>())
    };

    Ok(SettingsCopy {
        fetched_at,
        configured,
        settings: configured.then(|| settings.clone()),
        admin_url: text(&answer["admin_url"]).filter(|url| url.starts_with("https://")),
        saved_at: text(&answer["saved_at"]),
        saved_by: text(&answer["saved_by"]),
    })
}

struct Memory {
    /// Which connection the copy is from (its URL and secret), so a change starts over.
    source: String,
    copy: Option<SettingsCopy>,
    retry_at: i64,
}

static MEMORY: LazyLock<parking_lot::Mutex<Memory>> = LazyLock::new(|| {
    parking_lot::Mutex::new(Memory {
        source: String::new(),
        copy: None,
        retry_at: 0,
    })
});
static FETCHING: LazyLock<std::sync::Arc<tokio::sync::Mutex<()>>> = LazyLock::new(Default::default);

async fn load_copy(state: &crate::State, source: &str) -> Option<SettingsCopy> {
    let stored: String = state.cache.get(COPY_KEY).await.ok().flatten()?;
    let json = state.database.decrypt_base64(stored).await.ok()?;
    let (stored_source, copy): (String, SettingsCopy) = serde_json::from_str(&json).ok()?;

    (stored_source == source).then_some(copy)
}

async fn store_copy(state: &crate::State, source: &str, copy: &SettingsCopy) {
    let Ok(json) = serde_json::to_string(&(source, copy)) else {
        return;
    };
    match state.database.encrypt_base64(json).await {
        Ok(encrypted) => {
            if let Err(err) = state
                .cache
                .set(COPY_KEY, COPY_TTL, &encrypted.to_string())
                .await
            {
                tracing::warn!("rux: could not keep noprofit.space's settings: {err:#}");
            }
        }
        Err(err) => tracing::warn!("rux: could not encrypt noprofit.space's settings: {err:#}"),
    }
}

async fn fetch(
    state: &crate::State,
    config: &crate::noprofit::Config,
    secret: Option<&str>,
) -> Result<SettingsCopy, String> {
    let (status, json) = call(
        &state.client,
        config,
        secret,
        reqwest::Method::GET,
        SETTINGS_PATH,
        None,
        5,
    )
    .await
    .map_err(|err| err.0)?;
    if !status.is_success() {
        return Err(format!("noprofit.space answered HTTP {}", status.as_u16()));
    }

    parse_settings(&json, chrono::Utc::now().timestamp())
}

/// Where a copy comes from: noprofit.space's URL and which secret signs (a change starts over).
fn source_of(config: &crate::noprofit::Config, secret: Option<&str>) -> String {
    format!(
        "{}|{}",
        config.accounts_url,
        own_secret(secret).map_or("api", |_| "rux")
    )
}

/// Fetches the settings now and keeps them; on failure, waits [`RETRY_AFTER`] before the next
/// try and answers the copy there is.
async fn refresh(
    state: &crate::State,
    config: &crate::noprofit::Config,
    secret: Option<&str>,
    source: &str,
) -> Option<SettingsCopy> {
    let now = chrono::Utc::now().timestamp();
    match fetch(state, config, secret).await {
        Ok(copy) => {
            store_copy(state, source, &copy).await;
            let mut memory = MEMORY.lock();
            if memory.source == source {
                memory.copy = Some(copy.clone());
                memory.retry_at = 0;
            }
            Some(copy)
        }
        Err(reason) => {
            let current = {
                let mut memory = MEMORY.lock();
                if memory.source != source {
                    return None;
                }
                memory.retry_at = now + RETRY_AFTER;
                memory.copy.clone()
            };
            if state
                .cache
                .get::<bool>("rux::noprofit::settings_warned")
                .await
                .ok()
                .flatten()
                .is_none()
            {
                let _ = state
                    .cache
                    .set("rux::noprofit::settings_warned", 600, &true)
                    .await;
                tracing::warn!(
                    last_fetched_at = current.as_ref().map(|copy| copy.fetched_at),
                    "rux: could not fetch rux's settings from noprofit.space ({reason}); using the last ones fetched"
                );
            }
            current
        }
    }
}

/// [`refresh`] in the background, with the connection as the settings say now.
fn refresh_later(state: &crate::State) {
    let Ok(fetching) = FETCHING.clone().try_lock_owned() else {
        return;
    };
    let state = state.clone();
    tokio::spawn(async move {
        let _fetching = fetching;
        let (config, secret) = match state.settings.get().await {
            Ok(settings) => (
                settings.noprofit.config(),
                settings.rux.noprofit_secret.as_ref().map(|s| s.to_string()),
            ),
            Err(err) => {
                tracing::warn!("rux: could not read the settings: {err:#}");
                return;
            }
        };
        let Some(config) = config else {
            return;
        };
        if !configured(&config, secret.as_deref()) {
            return;
        }
        let source = source_of(&config, secret.as_deref());
        refresh(&state, &config, secret.as_deref(), &source).await;
    });
}

/// rux's settings as noprofit.space keeps them, or `None` while the panel's own apply (not
/// configured for rux, or no copy fetched yet and noprofit.space down). This runs for every
/// message the bot sees, so only the very first fetch is waited for: once there is a copy, an
/// old one is answered and fetched again in the background.
pub async fn settings(
    state: &crate::State,
    config: &crate::noprofit::Config,
    secret: Option<&str>,
) -> Option<SettingsCopy> {
    if !configured(config, secret) {
        return None;
    }

    let source = source_of(config, secret);
    let now = chrono::Utc::now().timestamp();
    let (current, retry_at) = {
        let mut memory = MEMORY.lock();
        if memory.source != source {
            memory.source = source.clone();
            memory.copy = None;
            memory.retry_at = 0;
        }
        (memory.copy.clone(), memory.retry_at)
    };
    if let Some(copy) = &current
        && copy.fetched_at > now - REFRESH
    {
        return current;
    }

    // the copy another instance (or this one before a restart) fetched
    let current = match current {
        Some(copy) => Some(copy),
        None => {
            let stored = load_copy(state, &source).await;
            if let Some(stored) = &stored {
                let mut memory = MEMORY.lock();
                if memory.source == source {
                    memory.copy = Some(stored.clone());
                }
            }
            stored
        }
    };
    if let Some(copy) = &current
        && copy.fetched_at > now - REFRESH
    {
        return current;
    }
    if now < retry_at {
        return current;
    }

    match current {
        Some(copy) => {
            refresh_later(state);
            Some(copy)
        }
        None => {
            // nothing to run on yet: this one fetch is waited for (5 seconds at most)
            let Ok(_fetching) = FETCHING.try_lock() else {
                return None;
            };
            refresh(state, config, secret, &source).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accounts_answers_are_read_leniently() {
        let found = parse_accounts(&json!({"accounts": [
            {"discord_id": "100000000000000001", "account_id": 7, "name": "Ann", "is_admin": true},
            {"discord_id": 100000000000000002u64, "account_id": "8", "is_admin": "yes"},
            {"discord_id": "100000000000000003"},
            "junk"
        ]}));

        assert_eq!(found.len(), 2);
        assert_eq!(
            found["100000000000000001"],
            RemoteAccount {
                account_id: 7,
                name: "Ann".into(),
                is_admin: true
            }
        );
        // only a real `true` makes an admin
        assert!(!found["100000000000000002"].is_admin);
        assert!(parse_accounts(&json!({})).is_empty());
    }

    #[test]
    fn a_configured_answer_needs_the_switch_and_the_server() {
        assert!(parse_settings(&json!({}), 1).is_err());
        assert!(parse_settings(&json!({"configured": true, "settings": null}), 1).is_err());
        assert!(
            parse_settings(
                &json!({"configured": true, "settings": {"enabled": "1", "guild_id": "1"}}),
                1
            )
            .is_err()
        );

        let copy = parse_settings(&json!({"configured": false, "settings": {"x": 1}}), 5).unwrap();
        assert!(!copy.configured && copy.settings.is_none());

        let copy = parse_settings(
            &json!({
                "configured": true,
                "settings": {"enabled": true, "guild_id": ""},
                "admin_url": "javascript:alert(1)",
                "saved_by": "Ann"
            }),
            5,
        )
        .unwrap();
        assert!(copy.configured && copy.settings.is_some());
        assert_eq!(copy.admin_url, None);
        assert_eq!(copy.saved_by.as_deref(), Some("Ann"));
    }

    #[test]
    fn short_secrets_do_not_sign() {
        assert_eq!(own_secret(Some("short")), None);
        assert_eq!(own_secret(Some(&"x".repeat(40))).map(str::len), Some(40));
        assert_eq!(own_secret(None), None);
    }
}
