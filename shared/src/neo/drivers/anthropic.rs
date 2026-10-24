//! plugged.host: Anthropic's Messages API. Its rules: the system prompt is top-level, roles
//! alternate and start with the user, every `tool_use` is answered by `tool_result` blocks at the
//! start of the next user message, no trailing assistant message, empty objects stay objects.

use super::Answer;
use crate::neo::chat::{self, Part, str_of};
use serde_json::{Value, json};

pub fn to_body(
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
    model: &str,
    max_tokens: i32,
) -> Value {
    let mut system: Vec<String> = Vec::new();
    let mut out: Vec<(String, Vec<Value>)> = Vec::new();

    for message in messages {
        match str_of(message, "role") {
            "system" => {
                let text = chat::text(&message["content"]);
                if !text.trim().is_empty() {
                    system.push(text.trim().to_string());
                }
            }
            "tool" => {
                let content = chat::text(&message["content"]);
                let mut block = json!({
                    "type": "tool_result",
                    "tool_use_id": str_of(message, "tool_call_id"),
                    "content": if content.is_empty() { "(no output)".to_string() } else { content },
                });
                if message["is_error"].as_bool().unwrap_or(false) {
                    block["is_error"] = Value::Bool(true);
                }
                append(&mut out, "user", vec![block]);
            }
            "assistant" => {
                let mut blocks = Vec::new();
                let text = chat::text(&message["content"]);
                if !text.trim().is_empty() {
                    blocks.push(json!({"type": "text", "text": text.trim()}));
                }
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": str_of(call, "id"),
                        "name": call["function"]["name"].as_str().unwrap_or_default(),
                        "input": chat::decode_arguments(&call["function"]["arguments"]),
                    }));
                }
                if !blocks.is_empty() {
                    append(&mut out, "assistant", blocks);
                }
            }
            _ => {
                let blocks: Vec<Value> = chat::parts(&message["content"])
                    .into_iter()
                    .filter_map(|part| match part {
                        Part::Image { mime, data } => Some(json!({
                            "type": "image",
                            "source": {"type": "base64", "media_type": mime, "data": data},
                        })),
                        Part::Text(text) if !text.trim().is_empty() => {
                            Some(json!({"type": "text", "text": text}))
                        }
                        Part::Text(_) => None,
                    })
                    .collect();
                if !blocks.is_empty() {
                    append(&mut out, "user", blocks);
                }
            }
        }
    }

    if out.first().is_none_or(|(role, _)| role != "user") {
        out.insert(
            0,
            (
                "user".into(),
                vec![json!({"type": "text", "text": "(conversation continues)"})],
            ),
        );
    }
    if out.last().is_some_and(|(role, _)| role == "assistant") {
        out.push((
            "user".into(),
            vec![json!({"type": "text", "text": "(continue)"})],
        ));
    }

    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": out
            .into_iter()
            .map(|(role, content)| json!({"role": role, "content": content}))
            .collect::<Vec<_>>(),
    });
    if !system.is_empty() {
        body["system"] = Value::String(system.join("\n\n"));
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|tool| {
                    let function = tool.get("function").unwrap_or(tool);
                    let schema = function
                        .get("parameters")
                        .filter(|schema| schema.is_object())
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
                    json!({
                        "name": str_of(function, "name"),
                        "description": str_of(function, "description"),
                        "input_schema": schema,
                    })
                })
                .collect(),
        );
        body["tool_choice"] = json!({"type": if tool_choice == "none" { "none" } else { "auto" }});
    }

    body
}

/// Appends blocks, merged into the previous message when the role repeats; in a merged user
/// message the `tool_result` blocks come first.
fn append(messages: &mut Vec<(String, Vec<Value>)>, role: &str, blocks: Vec<Value>) {
    if let Some((last_role, content)) = messages.last_mut()
        && last_role == role
    {
        content.extend(blocks);
        if role == "user" {
            content.sort_by_key(|block| u8::from(str_of(block, "type") != "tool_result"));
        }
        return;
    }

    messages.push((role.to_string(), blocks));
}

/// The answer, or `None` when it is not a Messages answer.
pub fn from_response(raw: &str) -> Option<Answer> {
    let json: Value = serde_json::from_str(raw).ok()?;
    if json["type"] == "error" {
        return None;
    }
    let blocks = json.get("content")?.as_array()?;

    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut seen = Vec::new();
    for block in blocks {
        match str_of(block, "type") {
            "text" => text.push_str(str_of(block, "text")),
            "tool_use" => {
                let Some(name) = block["name"].as_str() else {
                    continue;
                };
                let input = if block["input"].is_object() {
                    block["input"].clone()
                } else {
                    json!({})
                };
                tool_calls.push(json!({
                    "id": chat::tool_call_id(block["id"].as_str(), &mut seen),
                    "type": "function",
                    "function": {"name": name, "arguments": input.to_string()},
                }));
            }
            _ => {}
        }
    }

    let usage = &json["usage"];
    let text = text.trim().to_string();
    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else {
        match json["stop_reason"].as_str() {
            Some("max_tokens" | "model_context_window_exceeded") => "length",
            Some("refusal") => "content_filter",
            _ => "stop",
        }
    };

    Some(Answer {
        content: (!text.is_empty()).then_some(text),
        tool_calls,
        finish_reason,
        prompt_tokens: usage["input_tokens"].as_i64().unwrap_or(0)
            + usage["cache_creation_input_tokens"].as_i64().unwrap_or(0)
            + usage["cache_read_input_tokens"].as_i64().unwrap_or(0),
        completion_tokens: usage["output_tokens"].as_i64().unwrap_or(0),
        signatures: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternates_roles_and_puts_tool_results_first() {
        let body = to_body(
            &[
                json!({"role": "system", "content": "sys"}),
                json!({"role": "assistant", "content": "hello"}),
                json!({"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "function": {"name": "list_files", "arguments": "{\"path\":\"/\"}"}},
                    {"id": "call_2", "function": {"name": "server_status", "arguments": ""}}
                ]}),
                json!({"role": "user", "content": "and?"}),
                json!({"role": "tool", "tool_call_id": "call_1", "content": "a.txt"}),
                json!({"role": "tool", "tool_call_id": "call_2", "content": "", "is_error": true}),
            ],
            &[
                json!({"type": "function", "function": {"name": "list_files", "description": "d", "parameters": {"type": "object", "properties": {}}}}),
            ],
            "auto",
            "claude-test",
            2048,
        );

        assert_eq!(body["system"], "sys");
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"].as_array().unwrap().len(), 3);
        assert_eq!(messages[1]["content"][1]["input"]["path"], "/");
        assert!(messages[1]["content"][2]["input"].is_object());
        assert_eq!(messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(messages[2]["content"][1]["type"], "tool_result");
        assert_eq!(messages[2]["content"][1]["is_error"], true);
        assert_eq!(messages[2]["content"][2]["type"], "text");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(body["tool_choice"]["type"], "auto");
    }

    #[test]
    fn reads_text_tool_use_and_usage() {
        let answer = from_response(
            r#"{"type":"message","content":[{"type":"text","text":"On it."},{"type":"tool_use","id":"toolu_1","name":"power","input":{"signal":"restart"}}],
            "stop_reason":"tool_use","usage":{"input_tokens":3,"cache_read_input_tokens":4,"output_tokens":2}}"#,
        )
        .unwrap();
        assert_eq!(answer.content.as_deref(), Some("On it."));
        assert_eq!(
            answer.tool_calls[0]["function"]["arguments"],
            r#"{"signal":"restart"}"#
        );
        assert_eq!((answer.prompt_tokens, answer.completion_tokens), (7, 2));
        assert!(from_response(r#"{"type":"error","error":{}}"#).is_none());
    }
}
