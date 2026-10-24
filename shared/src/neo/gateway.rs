//! plugged.host: neo on noprofit.space's neo gateway (accounts `InternalNeoController`): turns are
//! streamed (`data: {json}` lines: `delta` with kind text / thinking / reset, `ping`, then one
//! `result` or `error`), usage is asked for and kept for 30 seconds. Requests are signed with the
//! gateway's own secret (`noprofit.ai.gateway_secret` there) and name the account by its id.

use super::{
    Ask, Delta, DeltaSender, GeneratedImage, TurnAnswer, TurnError, UsageInfo, usage_from,
};
use crate::{noprofit::Config, settings::neo::NeoPlan};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;

const BASE: &str = "/api/v1/internal/neo";
/// How long a streamed turn may take in all (the gateway's own deadline is shorter).
const STREAM_TIMEOUT: Duration = Duration::from_secs(290);
/// Silence after which a stream counts as lost (the gateway pings every 10 seconds).
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const USAGE_TTL: u64 = 30;

/// The service the panel's turns are recorded under: the sign-in client's name.
fn service(config: &Config) -> String {
    config.client.chars().take(32).collect()
}

fn usage_key(user_uuid: uuid::Uuid) -> String {
    format!("neo::usage::{user_uuid}")
}

async fn account_id(state: &crate::State, user_uuid: uuid::Uuid) -> Option<i64> {
    crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
        .await
        .ok()
        .flatten()
        .filter(|link| link.deleted.is_none())
        .map(|link| link.account_id)
}

async fn post(
    state: &crate::State,
    config: &Config,
    path: &str,
    body: &Value,
    accept: &str,
    timeout: Duration,
) -> Result<reqwest::Response, TurnError> {
    let path = format!("{BASE}{path}");
    let body = serde_json::to_vec(body).map_err(|err| TurnError::Unavailable {
        reason: format!("encode: {err}"),
    })?;
    let headers = config
        .sign_neo_request("POST", &path, &body)
        .ok_or_else(|| TurnError::Unavailable {
            reason: "not_configured".into(),
        })?;

    let mut request = state
        .client
        .post(format!("{}{path}", config.accounts_url))
        .timeout(timeout)
        .header("Content-Type", "application/json")
        .header("Accept", accept)
        .body(body);
    for (name, value) in headers {
        request = request.header(name, value);
    }

    request.send().await.map_err(|err| {
        tracing::warn!("noprofit.space's neo gateway is unreachable: {err:#}");
        TurnError::Unavailable {
            reason: "unreachable".into(),
        }
    })
}

/// The error a gateway answer stands for.
fn error_for(status: u16, data: &Value, retry_after_header: u64) -> TurnError {
    let retry_after = data["retry_after"].as_u64().unwrap_or(retry_after_header);
    let error = data["error"].as_str().unwrap_or_default();

    match status {
        429 => TurnError::LimitReached {
            window: data["window"].as_str().unwrap_or("short").to_string(),
            retry_at: data["retry_at"].as_str().map(str::to_string),
            limits: data["limits"].clone(),
        },
        404 => TurnError::NotLinked,
        422 if error.starts_with("model_") || error == "no_model" => TurnError::ModelUnavailable {
            reason: error.to_string(),
            label: data["label"].as_str().map(str::to_string),
            access: data["access"].as_str().map(str::to_string),
        },
        _ if error == "no_model" => TurnError::ModelUnavailable {
            reason: error.to_string(),
            label: None,
            access: None,
        },
        409 => TurnError::Busy {
            retry_after: retry_after.max(1),
        },
        _ if error == "busy" => TurnError::Busy {
            retry_after: retry_after.max(1),
        },
        _ => TurnError::Unavailable {
            reason: if error.is_empty() {
                format!("http_{status}")
            } else {
                error.to_string()
            },
        },
    }
}

/// A finished turn from the gateway's `result`.
fn answer_from(data: &Value) -> TurnAnswer {
    let message = &data["message"];

    TurnAnswer {
        content: message["content"]
            .as_str()
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(str::to_string),
        tool_calls: message["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .filter(|call| call.is_object())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        thinking: data["thinking"].as_str().unwrap_or_default().to_string(),
        images: data["images"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|image| {
                Some(GeneratedImage {
                    mime: image["mime"].as_str().unwrap_or("image/png").to_string(),
                    data: image["data"].as_str()?.to_string(),
                })
            })
            .collect(),
        finish_reason: data["finish_reason"].as_str().unwrap_or("stop").to_string(),
        provider_label: data["provider"]["label"].as_str().map(str::to_string),
        provider_model: data["provider"]["model"].as_str().map(str::to_string),
        charged_tokens: data["usage"]["charged_tokens"].as_i64().unwrap_or(0),
        usage: usage_from(data),
    }
}

/// Reads one `data:` event out of a stream buffer, if a whole one is there. Bytes, not text: a
/// chunk can end inside a character.
fn next_event(buffer: &mut Vec<u8>) -> Option<Option<Value>> {
    let end = buffer.windows(2).position(|pair| pair == b"\n\n")?;
    let block: Vec<u8> = buffer.drain(..end + 2).collect();
    let block = String::from_utf8_lossy(&block);
    let data: String = block
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");

    Some(serde_json::from_str(&data).ok())
}

pub async fn turn(
    state: &crate::State,
    config: &Config,
    user_uuid: uuid::Uuid,
    plan: NeoPlan,
    usage_percent: i64,
    ask: Ask<'_>,
    deltas: Option<&DeltaSender>,
) -> Result<TurnAnswer, TurnError> {
    let Some(account_id) = account_id(state, user_uuid).await else {
        return Err(TurnError::NotLinked);
    };
    let picked = super::picked_model(&state.database, user_uuid).await;

    let body = json!({
        "account_id": account_id,
        "service": service(config),
        "usage_percent": usage_percent.clamp(100, 1000),
        "plan": plan.as_str(),
        "model": picked,
        "conversation": ask.conversation,
        "request_id": uuid::Uuid::new_v4().to_string(),
        "messages": ask.messages,
        "tools": ask.tools,
        "tool_choice": ask.tool_choice,
        "stream": true,
    });

    let response = post(
        state,
        config,
        "/turn",
        &body,
        "text/event-stream",
        STREAM_TIMEOUT,
    )
    .await?;
    let status = response.status().as_u16();
    let streamed = response
        .headers()
        .get("Content-Type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));

    if !(200..300).contains(&status) || !streamed {
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let data: Value = response.json().await.unwrap_or_default();
        if (200..300).contains(&status) && data["message"].is_object() {
            return finish(state, user_uuid, answer_from(&data)).await;
        }

        return Err(error_for(status, &data, retry_after));
    }

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut last = None;

    'read: loop {
        let chunk = match tokio::time::timeout(IDLE_TIMEOUT, stream.next()).await {
            Ok(Some(Ok(chunk))) => chunk,
            Ok(None) => break,
            Ok(Some(Err(err))) => {
                tracing::warn!("noprofit.space's neo stream broke: {err:#}");
                break;
            }
            Err(_) => {
                tracing::warn!("noprofit.space's neo stream went silent");
                break;
            }
        };
        buffer.extend_from_slice(&chunk);

        while let Some(event) = next_event(&mut buffer) {
            let Some(event) = event else {
                continue;
            };
            match event["type"].as_str() {
                Some("delta") => {
                    if let Some(deltas) = deltas {
                        let text = event["text"].as_str().unwrap_or_default().to_string();
                        let _ = deltas.send(match event["kind"].as_str() {
                            Some("thinking") => Delta::Thinking(text),
                            Some("reset") => Delta::Reset,
                            _ => Delta::Text(text),
                        });
                    }
                }
                Some("result") | Some("error") => {
                    last = Some(event);
                    break 'read;
                }
                _ => {}
            }
        }
    }

    match last {
        Some(event) if event["type"] == "result" => {
            finish(state, user_uuid, answer_from(&event)).await
        }
        Some(event) => Err(error_for(
            event["status"].as_u64().unwrap_or(503) as u16,
            &event,
            0,
        )),
        None => Err(TurnError::Unavailable {
            reason: "stream_ended".into(),
        }),
    }
}

async fn finish(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    answer: TurnAnswer,
) -> Result<TurnAnswer, TurnError> {
    let _ = state
        .cache
        .set(&usage_key(user_uuid), USAGE_TTL, &answer.usage)
        .await;

    Ok(answer)
}

pub async fn usage(
    state: &crate::State,
    config: &Config,
    user_uuid: uuid::Uuid,
    plan: NeoPlan,
    usage_percent: i64,
) -> Option<UsageInfo> {
    if let Ok(Some(cached)) = state.cache.get::<UsageInfo>(&usage_key(user_uuid)).await {
        return Some(cached);
    }

    let Some(account_id) = account_id(state, user_uuid).await else {
        return Some(UsageInfo::default());
    };
    let body = json!({
        "account_id": account_id,
        "service": service(config),
        "usage_percent": usage_percent.clamp(100, 1000),
        "plan": plan.as_str(),
    });

    // once more when the connection itself failed: the way there is long
    let response = match post(
        state,
        config,
        "/usage",
        &body,
        "application/json",
        Duration::from_secs(8),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => post(
            state,
            config,
            "/usage",
            &body,
            "application/json",
            Duration::from_secs(8),
        )
        .await
        .ok()?,
    };

    let usage = match response.status().as_u16() {
        200..=299 => usage_from(&response.json::<Value>().await.ok()?),
        404 => UsageInfo::default(),
        _ => return None,
    };
    let _ = state
        .cache
        .set(&usage_key(user_uuid), USAGE_TTL, &usage)
        .await;

    Some(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_stream_events() {
        let mut buffer = b"data: {\"type\":\"delta\",\"kind\":\"text\",\"text\":\"Hel\"}\n\ndata: {\"type\":\"ping\"}\n\ndata: {\"type\":\"res".to_vec();
        assert_eq!(next_event(&mut buffer).unwrap().unwrap()["text"], "Hel");
        assert_eq!(next_event(&mut buffer).unwrap().unwrap()["type"], "ping");
        assert!(next_event(&mut buffer).is_none());
        buffer.extend_from_slice(b"ult\"}\n\n");
        assert_eq!(next_event(&mut buffer).unwrap().unwrap()["type"], "result");
    }

    #[test]
    fn maps_gateway_errors() {
        assert!(matches!(
            error_for(429, &json!({"window": "long", "retry_at": "x"}), 0),
            TurnError::LimitReached { window, .. } if window == "long"
        ));
        assert!(matches!(
            error_for(404, &json!({}), 0),
            TurnError::NotLinked
        ));
        assert!(matches!(
            error_for(422, &json!({"error": "model_not_allowed", "label": "Big"}), 0),
            TurnError::ModelUnavailable { label: Some(label), .. } if label == "Big"
        ));
        assert!(matches!(
            error_for(503, &json!({"error": "busy", "retry_after": 7}), 0),
            TurnError::Busy { retry_after: 7 }
        ));
        assert!(matches!(
            error_for(503, &json!({"error": "providers_unavailable"}), 0),
            TurnError::Unavailable { reason } if reason == "providers_unavailable"
        ));
    }

    #[test]
    fn reads_a_result() {
        let answer = answer_from(&json!({
            "message": {"content": " Done. ", "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "power", "arguments": "{}"}}]},
            "thinking": "hmm",
            "finish_reason": "tool_calls",
            "provider": {"label": "Fast", "model": "m-1"},
            "usage": {"charged_tokens": 1200},
            "mode": "all",
            "models": [],
        }));
        assert_eq!(answer.content.as_deref(), Some("Done."));
        assert_eq!(answer.tool_calls.len(), 1);
        assert_eq!(answer.charged_tokens, 1200);
        assert_eq!(answer.provider_label.as_deref(), Some("Fast"));
    }
}
