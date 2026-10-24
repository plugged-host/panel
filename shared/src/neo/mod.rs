//! plugged.host: neo, the assistant in the chat (docs/features/neo.md).
//!
//! Where neo's model turns run and its usage is metered: on noprofit.space's neo gateway while the
//! noprofit.space settings carry its secret ([`gateway`]), else on the panel itself ([`local`]):
//! the providers of Admin › Settings › neo ([`chain`]) and the 5-hour and weekly windows in
//! `neo_usage_*` ([`usage`]). Both answer in the same shapes, so the agent never asks which.
//!
//! Every slot is a model people pick by name in the chat; a turn runs on the picked one (the
//! default model when none was picked), never on another as a fallback. A membership raises the
//! limits ([`crate::settings::neo::AppSettingsNeo::usage_percent`]) and opens the models only
//! members may pick.

pub mod chain;
pub mod chat;
pub mod drivers;
mod gateway;
mod local;
pub mod prompted;
pub mod tools;
pub mod usage;

use crate::settings::neo::NeoPlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;
use utoipa::ToSchema;

pub use usage::{Limits, Window};

/// A piece of the answer while it is written.
#[derive(Debug, Clone)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// What came so far is void (the call is being made again).
    Reset,
}

pub type DeltaSender = tokio::sync::mpsc::UnboundedSender<Delta>;

/// Why a turn did not run.
#[derive(Debug, Clone)]
pub enum TurnError {
    /// A window is used up; `window` is short, long, model or images.
    LimitReached {
        window: String,
        retry_at: Option<String>,
        limits: Value,
    },
    /// noprofit.space does not know the account.
    NotLinked,
    /// The picked model is gone or not open to the account (`model_not_allowed`,
    /// `model_unavailable`, `no_model`).
    ModelUnavailable {
        reason: String,
        label: Option<String>,
        access: Option<String>,
    },
    /// Every provider is busy (or the account already has a turn running); try again.
    Busy { retry_after: u64 },
    /// neo cannot answer now.
    Unavailable { reason: String },
}

/// A model in the chat's picker.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Model {
    pub key: String,
    pub label: String,
    pub description: String,
    /// everyone, plus or plusplus.
    pub access: String,
    pub allowed: bool,
    /// It has a window of its own (noprofit.space's limits mode `models`).
    pub limited: bool,
    pub multiplier: i64,
    pub vision: bool,
    pub thinking: bool,
    pub window: Option<Window>,
}

/// Where a person's windows stand and what they can pick.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default)]
pub struct UsageInfo {
    /// `false` when noprofit.space does not know the account.
    pub linked: bool,
    #[schema(value_type = Option<Object>)]
    pub limits: Option<Limits>,
    /// What is metered: `all` (every model in the shared windows) or `models` (only limited ones).
    pub mode: String,
    pub models: Vec<Model>,
    pub default_model: Option<String>,
}

/// A picture an image model made.
#[derive(Debug, Clone)]
pub struct GeneratedImage {
    pub mime: String,
    /// base64
    pub data: String,
}

/// One answered turn.
#[derive(Debug, Clone)]
pub struct TurnAnswer {
    pub content: Option<String>,
    pub tool_calls: Vec<Value>,
    pub thinking: String,
    pub images: Vec<GeneratedImage>,
    pub finish_reason: String,
    pub provider_label: Option<String>,
    pub provider_model: Option<String>,
    pub charged_tokens: i64,
    pub usage: UsageInfo,
}

/// What a turn asks.
pub struct Ask<'a> {
    pub conversation: &'a str,
    pub messages: Vec<Value>,
    pub tools: Vec<Value>,
    /// auto or none.
    pub tool_choice: &'static str,
}

static KEYS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\bAIza[A-Za-z0-9_\-]{20,}").unwrap());

/// Google API keys masked (providers echo them in errors).
pub fn redact_keys(text: &str) -> String {
    KEYS.replace_all(text, "[redacted]").into_owned()
}

/// noprofit.space's connection when it runs neo.
pub fn on_noprofit(settings: &crate::settings::AppSettings) -> Option<crate::noprofit::Config> {
    settings
        .noprofit
        .config()
        .filter(|config| config.neo_enabled())
}

/// Whether neo can think at all (no network involved): noprofit.space runs it, or a provider is
/// set up.
pub fn configured(settings: &crate::settings::AppSettings) -> bool {
    on_noprofit(settings).is_some()
        || (settings.neo.gateway_enabled && settings.neo.usable_providers().next().is_some())
}

/// The account's membership, as neo sees it.
pub async fn plan(
    state: &crate::State,
    settings: &crate::settings::AppSettings,
    user_uuid: uuid::Uuid,
) -> NeoPlan {
    if !settings.economy.membership_enabled {
        return NeoPlan::Free;
    }

    match crate::economy::membership(&state.database, user_uuid).await {
        Ok(membership) if membership.has_plusplus(chrono::Utc::now().naive_utc()) => {
            NeoPlan::Plusplus
        }
        Ok(membership) if membership.has_plus() => NeoPlan::Plus,
        Ok(_) => NeoPlan::Free,
        Err(err) => {
            tracing::warn!("failed to read a membership for neo: {err:#}");
            NeoPlan::Free
        }
    }
}

/// The model picked in the chat (its key), or `None` for the default model.
pub async fn picked_model(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT model FROM user_neo_settings WHERE user_uuid = $1",
    )
    .bind(user_uuid)
    .fetch_optional(database.read())
    .await
    .ok()
    .flatten()
    .flatten()
    .filter(|model| !model.is_empty() && model.chars().all(|c| c.is_ascii_digit()))
}

/// One model turn for a panel user. With `deltas` the answer is handed on while it is written.
pub async fn turn(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    ask: Ask<'_>,
    deltas: Option<&DeltaSender>,
) -> Result<TurnAnswer, TurnError> {
    let settings = state
        .settings
        .get()
        .await
        .map_err(|err| TurnError::Unavailable {
            reason: format!("settings: {err:#}"),
        })?;
    let noprofit = on_noprofit(&settings);
    let plan = plan(state, &settings, user_uuid).await;
    let usage_percent = settings.neo.usage_percent(plan);
    let neo = settings.neo.clone();
    drop(settings);

    match noprofit {
        Some(config) => {
            gateway::turn(state, &config, user_uuid, plan, usage_percent, ask, deltas).await
        }
        None => local::turn(state, &neo, user_uuid, plan, usage_percent, ask, deltas).await,
    }
}

/// The person's windows, the limits mode and the models they can pick. `None` when unknown (the
/// gateway is down).
pub async fn usage(state: &crate::State, user_uuid: uuid::Uuid) -> Option<UsageInfo> {
    let settings = state.settings.get().await.ok()?;
    let noprofit = on_noprofit(&settings);
    let plan = plan(state, &settings, user_uuid).await;
    let usage_percent = settings.neo.usage_percent(plan);
    let neo = settings.neo.clone();
    drop(settings);

    match noprofit {
        Some(config) => gateway::usage(state, &config, user_uuid, plan, usage_percent).await,
        None => local::usage(state, &neo, user_uuid, plan, usage_percent).await,
    }
}

/// Reads a gateway answer's usage part (`limits`, `mode`, `models`, `default_model`).
fn usage_from(data: &Value) -> UsageInfo {
    UsageInfo {
        linked: data["linked"].as_bool().unwrap_or(true),
        limits: serde_json::from_value(data["limits"].clone()).ok(),
        mode: data["mode"].as_str().unwrap_or("all").to_string(),
        models: data["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| serde_json::from_value(model.clone()).ok())
            .collect(),
        default_model: match &data["default_model"] {
            Value::String(model) => Some(model.clone()),
            Value::Number(model) => Some(model.to_string()),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_google_keys() {
        assert_eq!(
            redact_keys("bad key AIzaSyA1234567890123456789012345."),
            "bad key [redacted]."
        );
    }

    #[test]
    fn reads_the_gateways_usage() {
        let usage = usage_from(&serde_json::json!({
            "limits": {"short": {"used": 5, "limit": 10, "percent": 50, "active": true, "started_at": null, "resets_at": "2026-10-24T15:00:00Z", "window_seconds": 18000}},
            "mode": "models",
            "models": [{"key": "2", "label": "Fast", "allowed": true, "extra": 1}, "junk"],
            "default_model": 2,
        }));
        assert!(usage.linked);
        assert_eq!(usage.mode, "models");
        assert_eq!(usage.models.len(), 1);
        assert_eq!(usage.models[0].label, "Fast");
        assert_eq!(usage.default_model.as_deref(), Some("2"));
        assert_eq!(usage.limits.unwrap()["short"].used, 5);
    }
}
