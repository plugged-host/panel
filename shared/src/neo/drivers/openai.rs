//! plugged.host: OpenAI-compatible `chat/completions` (OpenAI, Groq, OpenRouter, most gateways).
//! The chat shape neo works in is this one, so this mostly cleans up provider quirks on the way
//! back: Qwen-style `<think>` text and Hermes-style `<tool_call>{…}</tool_call>` written as text.

use super::Answer;
use crate::neo::chat::{self, Part, str_of};
use serde_json::{Value, json};
use std::sync::LazyLock;

static THINK: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<think>.*?(</think>|$)").unwrap());
static INLINE_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<tool_call>\s*(\{.*?\})\s*</tool_call>").unwrap());

pub fn to_body(
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
    model: &str,
    max_tokens: i32,
) -> Value {
    let messages: Vec<Value> = messages
        .iter()
        .map(|message| match str_of(message, "role") {
            "tool" => {
                let mut content = chat::text(&message["content"]);
                if message["is_error"].as_bool().unwrap_or(false) {
                    content = format!("ERROR: {content}");
                }
                json!({
                    "role": "tool",
                    "tool_call_id": str_of(message, "tool_call_id"),
                    "content": if content.is_empty() { "(no output)".to_string() } else { content },
                })
            }
            "assistant" => {
                let mut out = json!({
                    "role": "assistant",
                    "content": message.get("content").cloned().unwrap_or(Value::Null),
                });
                if let Some(calls) = message["tool_calls"].as_array().filter(|c| !c.is_empty()) {
                    out["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "id": str_of(call, "id"),
                                    "type": "function",
                                    "function": {
                                        "name": call["function"]["name"].as_str().unwrap_or_default(),
                                        "arguments": call["function"]["arguments"].as_str().unwrap_or("{}"),
                                    },
                                })
                            })
                            .collect(),
                    );
                }
                out
            }
            role => {
                let role = if role.is_empty() { "user" } else { role };
                if chat::has_images(std::slice::from_ref(message)) {
                    let parts: Vec<Value> = chat::parts(&message["content"])
                        .into_iter()
                        .map(|part| match part {
                            Part::Text(text) => json!({"type": "text", "text": text}),
                            Part::Image { mime, data } => json!({
                                "type": "image_url",
                                "image_url": {"url": format!("data:{mime};base64,{data}")},
                            }),
                        })
                        .collect();
                    json!({"role": role, "content": parts})
                } else {
                    json!({"role": role, "content": chat::text(&message["content"])})
                }
            }
        })
        .collect();

    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": max_tokens,
        "stream": false,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools.to_vec());
        body["tool_choice"] = Value::String(tool_choice.to_string());
    }

    body
}

/// The answer, or `None` when it is not a `chat/completions` answer.
pub fn from_response(raw: &str) -> Option<Answer> {
    let json: Value = serde_json::from_str(raw).ok()?;
    let choice = json.get("choices")?.get(0)?;
    let message = choice.get("message").filter(|m| m.is_object())?;

    let mut content = message["content"]
        .as_str()
        .map(|content| THINK.replace_all(content, "").trim().to_string())
        .unwrap_or_default();

    let mut seen = Vec::new();
    let mut tool_calls = Vec::new();
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let Some(name) = call["function"]["name"].as_str().filter(|n| !n.is_empty()) else {
            continue;
        };
        let arguments = match &call["function"]["arguments"] {
            Value::String(text) if !text.trim().is_empty() => text.clone(),
            Value::Object(_) => call["function"]["arguments"].to_string(),
            _ => "{}".to_string(),
        };
        tool_calls.push(json!({
            "id": chat::tool_call_id(call["id"].as_str(), &mut seen),
            "type": "function",
            "function": {"name": name, "arguments": arguments},
        }));
    }

    // Hermes-style calls written as text instead of real ones
    if tool_calls.is_empty() && content.contains("<tool_call>") {
        content = INLINE_CALL
            .replace_all(&content, |captures: &regex::Captures| {
                let Ok(decoded) = serde_json::from_str::<Value>(&captures[1]) else {
                    return captures[0].to_string();
                };
                let Some(name) = decoded["name"].as_str() else {
                    return captures[0].to_string();
                };
                let arguments = match &decoded["arguments"] {
                    Value::String(text) => text.clone(),
                    Value::Object(_) => decoded["arguments"].to_string(),
                    _ => "{}".to_string(),
                };
                tool_calls.push(json!({
                    "id": chat::tool_call_id(None, &mut seen),
                    "type": "function",
                    "function": {"name": name, "arguments": arguments},
                }));
                String::new()
            })
            .trim()
            .to_string();
    }

    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else {
        match choice["finish_reason"].as_str() {
            Some("length") => "length",
            Some("content_filter") => "content_filter",
            _ => "stop",
        }
    };

    Some(Answer {
        content: (!content.is_empty()).then_some(content),
        tool_calls,
        finish_reason,
        prompt_tokens: json["usage"]["prompt_tokens"]
            .as_i64()
            .unwrap_or((raw.len() as i64 + 3) / 4),
        completion_tokens: json["usage"]["completion_tokens"].as_i64().unwrap_or(0),
        signatures: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_errors_and_tool_calls_in_openai_shape() {
        let body = to_body(
            &[
                json!({"role": "system", "content": "sys"}),
                json!({"role": "user", "content": "hi"}),
                json!({"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "server_status", "arguments": "{}"}}
                ]}),
                json!({"role": "tool", "tool_call_id": "call_1", "content": "boom", "is_error": true}),
            ],
            &[
                json!({"type": "function", "function": {"name": "server_status", "parameters": {"type": "object", "properties": {}}}}),
            ],
            "auto",
            "gpt-test",
            1024,
        );

        assert_eq!(body["messages"][3]["content"], "ERROR: boom");
        assert_eq!(
            body["messages"][2]["tool_calls"][0]["function"]["name"],
            "server_status"
        );
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(body["max_completion_tokens"], 1024);
        assert!(body["tools"][0]["function"]["parameters"]["properties"].is_object());
    }

    #[test]
    fn reads_calls_thinking_leaks_and_inline_calls() {
        let answer = from_response(
            r#"{"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[
                {"id":"a b","function":{"name":"read_file","arguments":{"path":"/x"}}},
                {"id":"x","function":{"name":"","arguments":"{}"}}
            ]}}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
        )
        .unwrap();
        assert_eq!(answer.tool_calls.len(), 1);
        assert_eq!(answer.tool_calls[0]["id"], "ab");
        assert_eq!(
            answer.tool_calls[0]["function"]["arguments"],
            r#"{"path":"/x"}"#
        );
        assert_eq!(answer.finish_reason, "tool_calls");
        assert_eq!((answer.prompt_tokens, answer.completion_tokens), (10, 5));

        let answer = from_response(
            r#"{"choices":[{"message":{"content":"<think>hmm</think>Sure. <tool_call>{\"name\":\"power\",\"arguments\":{\"signal\":\"start\"}}</tool_call>"}}]}"#,
        )
        .unwrap();
        assert_eq!(answer.content.as_deref(), Some("Sure."));
        assert_eq!(answer.tool_calls[0]["function"]["name"], "power");
        assert_eq!(answer.finish_reason, "tool_calls");

        assert!(from_response(r#"{"error":{"message":"no"}}"#).is_none());
    }
}
