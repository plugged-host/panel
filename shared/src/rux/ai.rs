//! plugged.host: rux's AI: neo's providers on neo's chain ([`crate::neo::chain`]), in rux's own
//! scope (`rux`: its own cooldowns, daily counts and tool probes), called directly: no neo
//! limits apply. The providers are the panel's (Admin › Settings › neo), or, while noprofit.space
//! runs neo (its neo secret is set), the ones noprofit.space runs neo on
//! (`GET /api/v1/internal/neo/providers`, signed with the neo secret, reused for a minute and
//! kept as an encrypted last-good copy).
//!
//! Premium models (the ones not open to everyone) answer rux's orders first, before the others;
//! the checks of watched channels and standing rules never reach them: when the other models
//! cannot answer a check, the message is not checked.

use crate::{
    neo::chain::{Chain, ChainError, Turn},
    settings::neo::{AppSettingsNeo, NeoAccess, NeoProvider},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

pub const PROVIDERS_PATH: &str = "/api/v1/internal/neo/providers";
const REFRESH: i64 = 60;
const RETRY_AFTER: i64 = 30;
const COPY_KEY: &str = "rux::neo::providers";
const COPY_TTL: u64 = 30 * 86400;

/// What a turn is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// An order: premium models first, then the others.
    Order,
    /// A check of a member's message: never a premium model.
    Check,
}

#[derive(Debug, Clone)]
pub enum AiError {
    /// Every model is busy; one is back in `retry_after` seconds.
    Busy { retry_after: u64 },
    /// No model can answer (none set up, all failing).
    Unavailable(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy { retry_after } => write!(f, "busy for {retry_after}s"),
            Self::Unavailable(reason) => f.write_str(reason),
        }
    }
}

/// Whether a provider is premium: not open to everyone.
pub fn premium(provider: &NeoProvider) -> bool {
    provider.access != NeoAccess::Everyone
}

/// The slots a turn tries, in order: for orders the premium ones first, then the others; for
/// checks only the others.
pub fn order_of(settings: &AppSettingsNeo, purpose: Purpose) -> Vec<&NeoProvider> {
    let usable: Vec<&NeoProvider> = settings.usable_providers().collect();
    let others = usable.iter().copied().filter(|p| !premium(p));

    match purpose {
        Purpose::Order => usable
            .iter()
            .copied()
            .filter(|p| premium(p))
            .chain(others)
            .collect(),
        Purpose::Check => others.collect(),
    }
}

/// A provider slot as noprofit.space sends it, made into the panel's shape: a `premium` flag
/// makes it premium, PHP's empty `[]` for the maps is taken as empty, an unknown driver leaves
/// the slot out.
pub fn provider_of(value: &Value) -> Option<NeoProvider> {
    let mut value = value.clone();
    let object = value.as_object_mut()?;

    for key in ["extra_headers", "extra_body"] {
        if !object.get(key).is_some_and(Value::is_object) {
            object.insert(key.into(), Value::Object(Default::default()));
        }
    }
    // header values must be text
    if let Some(Value::Object(headers)) = object.get_mut("extra_headers") {
        headers.retain(|_, value| value.is_string());
    }
    if !matches!(
        object.get("driver").and_then(Value::as_str),
        Some("openai" | "anthropic" | "gemini")
    ) {
        return None;
    }
    let access = match object.get("access").and_then(Value::as_str) {
        Some(access @ ("everyone" | "plus" | "plusplus")) => access.to_string(),
        _ => "everyone".to_string(),
    };
    let premium = object.get("premium").and_then(Value::as_bool) == Some(true);
    object.insert(
        "access".into(),
        Value::String(if premium && access == "everyone" {
            "plus".into()
        } else {
            access
        }),
    );
    if !object.get("vision").is_some_and(Value::is_boolean) {
        object.insert("vision".into(), Value::Null);
    }
    if !object.get("enabled").is_some_and(Value::is_boolean) {
        object.insert("enabled".into(), Value::Bool(true));
    }
    for (key, default) in [("max_tokens", 4096), ("timeout", 30), ("multiplier", 1)] {
        if !object.get(key).is_some_and(|v| v.as_i64().is_some()) {
            object.insert(key.into(), Value::from(default));
        }
    }
    if !object
        .get("daily_token_cap")
        .is_some_and(|v| v.as_i64().is_some())
    {
        object.insert("daily_token_cap".into(), Value::from(0));
    }
    for key in ["label", "description", "endpoint", "model"] {
        if !object.get(key).is_some_and(Value::is_string) {
            object.insert(key.into(), Value::String(String::new()));
        }
    }
    if !object.get("api_key").is_some_and(Value::is_string) {
        object.insert("api_key".into(), Value::Null);
    }

    serde_json::from_value(value).ok()
}

/// A providers answer of noprofit.space as the chain's settings.
pub fn settings_of(answer: &Value) -> Option<AppSettingsNeo> {
    let providers = answer["providers"].as_array()?;
    let mut providers: Vec<NeoProvider> = providers.iter().filter_map(provider_of).collect();
    providers.sort_by_key(|provider| provider.slot);
    let failover = &answer["failover"];
    let defaults = AppSettingsNeo::default();
    let number = |key: &str, default: i32| {
        failover[key]
            .as_i64()
            .or_else(|| failover[key].as_str().and_then(|v| v.parse().ok()))
            .map_or(default, |v| v.clamp(1, 3600) as i32)
    };

    Some(AppSettingsNeo {
        default_cooldown: number("default_cooldown", defaults.default_cooldown),
        deadline: number("deadline", defaults.deadline),
        providers,
        ..defaults
    })
}

#[derive(Clone, Serialize, Deserialize)]
struct ProvidersCopy {
    source: String,
    fetched_at: i64,
    answer: Value,
}

struct Memory {
    copy: Option<ProvidersCopy>,
    retry_at: i64,
}

static MEMORY: LazyLock<parking_lot::Mutex<Memory>> = LazyLock::new(|| {
    parking_lot::Mutex::new(Memory {
        copy: None,
        retry_at: 0,
    })
});
static FETCHING: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

async fn stored(state: &crate::State, source: &str) -> Option<ProvidersCopy> {
    let encrypted: String = state.cache.get(COPY_KEY).await.ok().flatten()?;
    let json = state.database.decrypt_base64(encrypted).await.ok()?;
    let copy: ProvidersCopy = serde_json::from_str(&json).ok()?;

    (copy.source == source).then_some(copy)
}

async fn store(state: &crate::State, copy: &ProvidersCopy) {
    let Ok(json) = serde_json::to_string(copy) else {
        return;
    };
    if let Ok(encrypted) = state.database.encrypt_base64(json).await {
        let _ = state
            .cache
            .set(COPY_KEY, COPY_TTL, &encrypted.to_string())
            .await;
    }
}

async fn fetch(state: &crate::State, config: &crate::noprofit::Config) -> Result<Value, String> {
    let headers = config
        .sign_neo_request("GET", PROVIDERS_PATH, b"")
        .ok_or("the neo secret is not set")?;
    let mut request = state
        .client
        .get(format!("{}{PROVIDERS_PATH}", config.accounts_url))
        .timeout(std::time::Duration::from_secs(5))
        .header("Accept", "application/json");
    for (name, value) in headers {
        request = request.header(name, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("noprofit.space could not be reached: {err}"))?;
    let status = response.status();
    let answer: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() || !answer["providers"].is_array() {
        return Err(format!("noprofit.space answered HTTP {}", status.as_u16()));
    }

    Ok(answer)
}

/// noprofit.space's providers answer: fresh, the last one fetched, or `None` when there never
/// was one.
async fn noprofit_answer(state: &crate::State, config: &crate::noprofit::Config) -> Option<Value> {
    let source = config.accounts_url.clone();
    let now = chrono::Utc::now().timestamp();
    let (mut current, retry_at) = {
        let mut memory = MEMORY.lock();
        if memory
            .copy
            .as_ref()
            .is_some_and(|copy| copy.source != source)
        {
            memory.copy = None;
            memory.retry_at = 0;
        }
        (memory.copy.clone(), memory.retry_at)
    };
    if let Some(copy) = &current
        && copy.fetched_at > now - REFRESH
    {
        return Some(copy.answer.clone());
    }

    let Ok(_fetching) = FETCHING.try_lock() else {
        return match current {
            Some(copy) => Some(copy.answer),
            None => stored(state, &source).await.map(|copy| copy.answer),
        };
    };

    if current.is_none()
        && let Some(copy) = stored(state, &source).await
    {
        MEMORY.lock().copy = Some(copy.clone());
        let fresh = copy.fetched_at > now - REFRESH;
        current = Some(copy);
        if fresh {
            return current.map(|copy| copy.answer);
        }
    }
    if now < retry_at {
        return current.map(|copy| copy.answer);
    }

    match fetch(state, config).await {
        Ok(answer) => {
            let copy = ProvidersCopy {
                source,
                fetched_at: now,
                answer,
            };
            store(state, &copy).await;
            let mut memory = MEMORY.lock();
            memory.copy = Some(copy.clone());
            memory.retry_at = 0;
            Some(copy.answer)
        }
        Err(reason) => {
            MEMORY.lock().retry_at = now + RETRY_AFTER;
            tracing::warn!("rux: could not fetch neo's providers from noprofit.space: {reason}");
            current.map(|copy| copy.answer)
        }
    }
}

/// Where the providers come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Panel,
    Noprofit,
}

/// The chain rux runs on now, and where it comes from. noprofit.space's is empty while it was
/// never fetched.
pub async fn providers(state: &crate::State) -> Result<(AppSettingsNeo, Source), anyhow::Error> {
    let (panel, noprofit) = {
        let settings = state.settings.get().await?;
        (settings.neo.clone(), crate::neo::on_noprofit(&settings))
    };

    let Some(config) = noprofit else {
        return Ok((panel, Source::Panel));
    };
    let settings = noprofit_answer(state, &config)
        .await
        .and_then(|answer| settings_of(&answer))
        .unwrap_or_else(|| AppSettingsNeo {
            providers: Vec::new(),
            ..AppSettingsNeo::default()
        });

    Ok((settings, Source::Noprofit))
}

/// The scope rux's cooldowns and counts are kept under.
pub const SCOPE: &str = "rux";

/// One model turn: on the first model that answers, premium ones first for orders, never for
/// checks.
pub async fn turn(
    state: &crate::State,
    purpose: Purpose,
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
) -> Result<Turn, AiError> {
    let (settings, _) = providers(state)
        .await
        .map_err(|err| AiError::Unavailable(format!("settings: {err:#}")))?;
    let chain = Chain {
        scope: SCOPE,
        probe_tools: settings.tools_probe,
    };
    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(settings.deadline.max(10) as u64);
    let remaining = || {
        deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .as_secs()
    };
    let mut busy: Option<u64> = None;
    // premium models that were tried and failed (an order without models open to everyone then
    // reads "every provider failed", not "none set up")
    let mut premium_failed = 0;

    if purpose == Purpose::Order {
        let premium_slots: Vec<u8> = settings
            .usable_providers()
            .filter(|provider| premium(provider))
            .map(|provider| provider.slot)
            .collect();
        for slot in premium_slots {
            if remaining() < 5 {
                break;
            }
            let mut slot_settings = settings.clone();
            slot_settings.deadline = remaining() as i32;
            match chain
                .turn(
                    state,
                    &slot_settings,
                    messages,
                    tools,
                    tool_choice,
                    Some(slot),
                )
                .await
            {
                Ok(turn) => return Ok(turn),
                Err(ChainError::Busy { retry_after, .. }) => sooner(&mut busy, retry_after),
                Err(_) => premium_failed += 1,
            }
        }
    }

    if remaining() >= 5 {
        let mut rest = settings.clone();
        rest.deadline = remaining() as i32;
        match chain
            .turn(state, &rest, messages, tools, tool_choice, None)
            .await
        {
            Ok(turn) => return Ok(turn),
            Err(ChainError::Busy { retry_after, .. }) => sooner(&mut busy, retry_after),
            Err(ChainError::NotConfigured) if busy.is_none() && premium_failed == 0 => {
                return Err(AiError::Unavailable(
                    if settings.usable_providers().next().is_none() {
                        "no AI provider is set up".into()
                    } else {
                        "no AI provider open to everyone is set up".into()
                    },
                ));
            }
            Err(ChainError::Unavailable { attempts }) if busy.is_none() => {
                return Err(AiError::Unavailable(format!(
                    "every AI provider failed ({} tried)",
                    attempts.len() + premium_failed
                )));
            }
            Err(_) => {}
        }
    } else {
        sooner(&mut busy, 2);
    }

    Err(match busy {
        Some(retry_after) => AiError::Busy { retry_after },
        None if premium_failed > 0 => {
            AiError::Unavailable(format!("every AI provider failed ({premium_failed} tried)"))
        }
        None => AiError::Unavailable("no AI provider answered".into()),
    })
}

fn sooner(busy: &mut Option<u64>, retry_after: u64) {
    *busy = Some(busy.map_or(retry_after, |b| b.min(retry_after)));
}

/// "Label (model)" of each slot a turn tries, for the settings page.
pub fn chain_labels(settings: &AppSettingsNeo, purpose: Purpose) -> Vec<String> {
    order_of(settings, purpose)
        .into_iter()
        .map(|provider| format!("{} ({})", provider.label, provider.model))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn slot(slot: u8, access: &str) -> Value {
        json!({
            "slot": slot,
            "enabled": true,
            "label": format!("Model {slot}"),
            "driver": "openai",
            "endpoint": "https://api.example.com/v1",
            "api_key": "key",
            "model": format!("model-{slot}"),
            "max_tokens": 1024,
            "timeout": 20,
            "access": access,
            "extra_headers": [],
            "extra_body": [],
        })
    }

    #[test]
    fn noprofit_slots_are_read_leniently() {
        let provider = provider_of(&slot(1, "everyone")).unwrap();
        assert!(provider.usable());
        assert!(provider.extra_headers.is_empty() && provider.extra_body.is_empty());
        assert!(!premium(&provider));

        let mut flagged = slot(2, "everyone");
        flagged["premium"] = json!(true);
        assert!(premium(&provider_of(&flagged).unwrap()));

        let mut unknown = slot(3, "everyone");
        unknown["driver"] = json!("bard");
        assert!(provider_of(&unknown).is_none());

        let mut sparse = json!({"slot": 4, "driver": "gemini", "access": "nobody"});
        sparse["extra_headers"] = json!({"X-A": "b", "X-B": 1});
        let provider = provider_of(&sparse).unwrap();
        assert_eq!(provider.access, NeoAccess::Everyone);
        assert_eq!(provider.extra_headers.len(), 1);
        assert!(!provider.usable());
    }

    #[test]
    fn orders_try_premium_first_and_checks_never() {
        let settings = settings_of(&json!({
            "providers": [slot(2, "everyone"), slot(1, "plusplus"), slot(3, "plus"), "junk"],
            "failover": {"default_cooldown": "30", "deadline": 9999}
        }))
        .unwrap();
        assert_eq!(settings.default_cooldown, 30);
        assert_eq!(settings.deadline, 3600);

        let order: Vec<u8> = order_of(&settings, Purpose::Order)
            .iter()
            .map(|p| p.slot)
            .collect();
        assert_eq!(order, vec![1, 3, 2]);
        let check: Vec<u8> = order_of(&settings, Purpose::Check)
            .iter()
            .map(|p| p.slot)
            .collect();
        assert_eq!(check, vec![2]);
        assert_eq!(
            chain_labels(&settings, Purpose::Check),
            vec!["Model 2 (model-2)"]
        );

        assert!(settings_of(&json!({"providers": "x"})).is_none());
    }
}
