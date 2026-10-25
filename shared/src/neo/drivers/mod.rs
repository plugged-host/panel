//! plugged.host: neo's three provider APIs and one call to a provider slot (no failover; that is
//! [`super::chain`]).

pub mod anthropic;
pub mod gemini;
pub mod openai;

use crate::settings::neo::{NeoDriver, NeoProvider, RESERVED_HEADERS};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::Duration,
};

/// One model answer in the chat shape.
#[derive(Debug, Clone)]
pub struct Answer {
    pub content: Option<String>,
    /// `{id, type: function, function: {name, arguments}}`, arguments a JSON string.
    pub tool_calls: Vec<Value>,
    /// stop, length, tool_calls or content_filter.
    pub finish_reason: &'static str,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    /// Gemini's thought signatures, by tool call id.
    pub signatures: Vec<(String, String)>,
}

impl Answer {
    pub fn total_tokens(&self) -> i64 {
        self.prompt_tokens.max(0) + self.completion_tokens.max(0)
    }
}

/// Why a call failed, and how long the slot should be left alone (0: not at all).
#[derive(Debug, Clone)]
pub struct Failure {
    pub status: Option<u16>,
    pub reason: String,
    pub cooldown: u64,
}

/// Fields a slot's extra body may not set (the driver's own).
const RESERVED_BODY: &[&str] = &[
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
    "systemInstruction",
    "system_instruction",
    "toolConfig",
    "tool_config",
    "generationConfig",
    "generation_config",
];

/// Google answers many requests (free Gemma especially) with a passing 500 or 503; a Gemini call
/// is tried this many times while 10 seconds of its timeout are left.
const GEMINI_ATTEMPTS: u32 = 3;
/// Some gateways turn a share of calls away with a bare 403 before their API sees them.
const BLANK_REJECTION_ATTEMPTS: u32 = 3;

/// The failover timings a call needs.
#[derive(Clone, Copy, Debug)]
pub struct Cooldowns {
    pub default: u64,
    pub auth: u64,
    pub max: u64,
}

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// The HTTP client for provider calls: internal addresses are refused (`APP_BLOCKED_CIDRS`) and
/// redirects are not followed.
fn client(env: &Arc<crate::env::Env>) -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("calagopus-panel {}", crate::VERSION))
            .connect_timeout(Duration::from_secs(5))
            .no_proxy()
            .dns_resolver(Arc::new(crate::net::BlockedIpResolver::new(
                env,
                "neo provider",
            )))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to create the neo HTTP client")
    })
}

/// Where a slot's requests go.
pub fn request_url(provider: &NeoProvider) -> String {
    match provider.driver {
        NeoDriver::Gemini => gemini::request_url(&provider.endpoint, &provider.model),
        driver => {
            let suffix = if driver == NeoDriver::Anthropic {
                "/messages"
            } else {
                "/chat/completions"
            };
            let url = provider.endpoint.trim_end_matches('/');
            if url.ends_with(suffix) {
                url.to_string()
            } else {
                format!("{url}{suffix}")
            }
        }
    }
}

/// The request body for one call on `provider`.
pub fn body(
    provider: &NeoProvider,
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
    signatures: &HashMap<String, String>,
) -> Value {
    let max_tokens = provider.max_tokens.max(256);
    let extra = &provider.extra_body;

    let mut body = match provider.driver {
        NeoDriver::Anthropic => {
            anthropic::to_body(messages, tools, tool_choice, &provider.model, max_tokens)
        }
        NeoDriver::Gemini => {
            gemini::to_body(messages, tools, tool_choice, &provider.model, signatures)
        }
        NeoDriver::Openai => {
            openai::to_body(messages, tools, tool_choice, &provider.model, max_tokens)
        }
    };

    if let Value::Object(fields) = &mut body {
        for (key, value) in extra {
            let allowed = match provider.driver {
                NeoDriver::Gemini => gemini::EXTRA_FIELDS.contains(&key.as_str()),
                _ => !RESERVED_BODY.contains(&key.as_str()),
            };
            if allowed && !fields.contains_key(key) {
                fields.insert(key.clone(), value.clone());
            }
        }

        if provider.driver == NeoDriver::Gemini {
            // an operator's generationConfig (thinkingConfig…) is kept; only the output cap is ours
            let mut config = extra
                .get("generationConfig")
                .or_else(|| extra.get("generation_config"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            config.insert("maxOutputTokens".into(), max_tokens.into());
            fields.insert("generationConfig".into(), Value::Object(config));
        }
    }

    body
}

/// One call to one slot within `timeout` seconds.
#[allow(clippy::too_many_arguments)]
pub async fn call(
    env: &Arc<crate::env::Env>,
    provider: &NeoProvider,
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
    timeout: u64,
    signatures: &HashMap<String, String>,
    cooldowns: Cooldowns,
) -> Result<Answer, Failure> {
    let api_key = provider.api_key.as_deref().unwrap_or_default();
    let body = body(provider, messages, tools, tool_choice, signatures);
    let url = request_url(provider);

    if let Ok(parsed) = reqwest::Url::parse(&url)
        && let Some(ip) = parsed.host_str().and_then(crate::net::host_to_ip)
        && crate::net::is_blocked_ip(&env.app_blocked_cidrs, &ip)
    {
        return Err(Failure {
            status: None,
            reason: "endpoint_not_public".into(),
            cooldown: cooldowns.auth,
        });
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout.max(5));
    let mut attempt = 0;
    let (status, retry_after, text) = loop {
        attempt += 1;
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());

        let mut request = client(env)
            .post(&url)
            .timeout(left.max(Duration::from_secs(1)))
            .header("Accept", "application/json")
            .json(&body);
        for (name, value) in &provider.extra_headers {
            // the Anthropic driver sends `anthropic-version` itself (its value or the default),
            // so it goes out once
            if !RESERVED_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                && !(provider.driver == NeoDriver::Anthropic
                    && name.eq_ignore_ascii_case("anthropic-version"))
            {
                request = request.header(name.as_str(), value.as_str());
            }
        }
        request = match provider.driver {
            NeoDriver::Anthropic => request.header("x-api-key", api_key).header(
                "anthropic-version",
                provider
                    .extra_headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("anthropic-version"))
                    .map_or("2023-06-01", |(_, value)| value.as_str()),
            ),
            NeoDriver::Gemini => request.header("x-goog-api-key", api_key),
            NeoDriver::Openai => request.bearer_auth(api_key),
        };

        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                return Err(Failure {
                    status: None,
                    reason: format!(
                        "connection: {}",
                        crate::neo::redact_keys(&err.without_url().to_string())
                    ),
                    cooldown: cooldowns.default,
                });
            }
        };
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(|seconds| seconds.ceil().max(0.0) as u64);
        let text = response.text().await.unwrap_or_default();
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());

        if status == 403 && text.trim().is_empty() {
            if attempt >= BLANK_REJECTION_ATTEMPTS || left < Duration::from_secs(2) {
                break (status, retry_after, text);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }
        if provider.driver != NeoDriver::Gemini
            || !matches!(status, 500 | 503)
            || attempt >= GEMINI_ATTEMPTS
            || left < Duration::from_secs(11)
        {
            break (status, retry_after, text);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    };

    if !(200..300).contains(&status) {
        return Err(Failure {
            status: Some(status),
            reason: detail(status, &text),
            cooldown: cooldown_for(status, retry_after, &text, cooldowns),
        });
    }

    let answer = match provider.driver {
        NeoDriver::Anthropic => anthropic::from_response(&text),
        NeoDriver::Gemini => gemini::from_response(&text),
        NeoDriver::Openai => openai::from_response(&text),
    };

    answer.ok_or_else(|| Failure {
        status: Some(status),
        reason: "malformed_response".into(),
        cooldown: cooldowns.default,
    })
}

/// noprofit.space's cooldown rules: auth errors a while, requests the provider found too large
/// not at all, rate limits and outages for their Retry-After (Gemini says it in the body) or the
/// default.
pub fn cooldown_for(
    status: u16,
    retry_after: Option<u64>,
    body: &str,
    cooldowns: Cooldowns,
) -> u64 {
    let retry_after = retry_after.or_else(|| retry_delay(body));
    let blank_rejection = status == 403 && body.trim().is_empty();

    let cooldown = match status {
        _ if blank_rejection => cooldowns.default,
        401 | 403 | 404 => cooldowns.auth,
        400 | 413 | 422 => 0,
        408 | 409 | 425 | 429 | 498 | 529 => retry_after.unwrap_or(cooldowns.default),
        status if status >= 500 => retry_after.unwrap_or(cooldowns.default),
        _ => cooldowns.default,
    };

    cooldown.min(cooldowns.max)
}

/// Gemini's `google.rpc.RetryInfo` detail (`"37s"`).
fn retry_delay(body: &str) -> Option<u64> {
    let json: Value = serde_json::from_str(body).ok()?;

    json["error"]["details"]
        .as_array()?
        .iter()
        .filter(|detail| {
            detail["@type"]
                .as_str()
                .is_some_and(|kind| kind.ends_with("google.rpc.RetryInfo"))
        })
        .find_map(|detail| {
            detail["retryDelay"]
                .as_str()?
                .strip_suffix('s')?
                .parse::<f64>()
                .ok()
                .map(|seconds| seconds.ceil().max(1.0) as u64)
        })
}

/// `http_<status>` and what the provider said, with keys masked.
fn detail(status: u16, body: &str) -> String {
    let said = serde_json::from_str::<Value>(body).ok().and_then(|json| {
        json["error"]["message"]
            .as_str()
            .or_else(|| json["message"].as_str())
            .or_else(|| json["error"].as_str())
            .map(str::to_string)
    });

    match said.filter(|said| !said.is_empty()) {
        Some(said) => format!(
            "http_{status}: {}",
            crate::neo::redact_keys(&said)
                .chars()
                .take(200)
                .collect::<String>()
        ),
        None => format!("http_{status}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOLDOWNS: Cooldowns = Cooldowns {
        default: 60,
        auth: 600,
        max: 900,
    };

    #[test]
    fn cooldowns_follow_the_status() {
        assert_eq!(cooldown_for(401, None, "{}", COOLDOWNS), 600);
        assert_eq!(cooldown_for(403, None, "", COOLDOWNS), 60);
        assert_eq!(cooldown_for(413, None, "{}", COOLDOWNS), 0);
        assert_eq!(cooldown_for(429, Some(12), "{}", COOLDOWNS), 12);
        assert_eq!(cooldown_for(503, Some(5000), "{}", COOLDOWNS), 900);
        assert_eq!(
            cooldown_for(
                429,
                None,
                r#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"36.2s"}]}}"#,
                COOLDOWNS
            ),
            37
        );
    }

    #[test]
    fn urls_and_extra_fields() {
        let mut provider = NeoProvider {
            slot: 1,
            enabled: true,
            label: "x".into(),
            description: "".into(),
            driver: NeoDriver::Openai,
            endpoint: "https://api.example.com/v1/".into(),
            api_key: Some("k".into()),
            model: "m".into(),
            max_tokens: 100,
            timeout: 30,
            daily_token_cap: 0,
            extra_headers: Default::default(),
            extra_body: serde_json::from_str(
                r#"{"reasoning_effort":"low","model":"other","safetySettings":[]}"#,
            )
            .unwrap(),
            access: Default::default(),
            multiplier: 1,
            vision: None,
        };
        assert_eq!(
            request_url(&provider),
            "https://api.example.com/v1/chat/completions"
        );
        let built = body(&provider, &[], &[], "auto", &HashMap::new());
        assert_eq!(built["model"], "m");
        assert_eq!(built["reasoning_effort"], "low");
        assert_eq!(built["max_completion_tokens"], 256);

        provider.driver = NeoDriver::Gemini;
        let built = body(&provider, &[], &[], "auto", &HashMap::new());
        assert!(built.get("reasoning_effort").is_none());
        assert!(built["safetySettings"].is_array());
        assert_eq!(built["generationConfig"]["maxOutputTokens"], 256);

        provider.driver = NeoDriver::Anthropic;
        provider.endpoint = "https://api.example.com/v1/messages".into();
        assert_eq!(
            request_url(&provider),
            "https://api.example.com/v1/messages"
        );
    }

    #[test]
    fn details_mask_keys() {
        assert_eq!(
            detail(
                400,
                r#"{"error":{"message":"API key AIzaSyA1234567890123456789012345 not valid"}}"#
            ),
            "http_400: API key [redacted] not valid"
        );
        assert_eq!(detail(502, "<html>"), "http_502");
    }
}
