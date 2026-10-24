//! plugged.host: Google AI Studio's `generateContent` (Gemini and Gemma models). Its rules: the
//! system prompt is a top-level `systemInstruction`, the assistant role is `model`, every
//! `functionCall` is answered by a `functionResponse` (which needs the function's name) in the next
//! user turn, empty objects stay objects. Gemini 3 models reject a function call sent back without
//! its thought signature, so signatures are handed out with the answer and asked for again.

use super::Answer;
use crate::neo::chat::{self, Part, str_of};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Google's documented stand-in for a function call whose signature is unknown. Gemma models
/// answer it with HTTP 500 and need no signatures, so they never get it.
pub const SKIP_SIGNATURE: &str = "skip_thought_signature_validator";

/// The extra fields a Gemini request takes besides `generationConfig`.
pub const EXTRA_FIELDS: &[&str] = &[
    "safetySettings",
    "safety_settings",
    "cachedContent",
    "cached_content",
];

/// The bare model id: `models/gemini-3-flash` and `gemma-4:generateContent` both name a model.
pub fn model_id(model: &str) -> String {
    let model = model.trim();
    let model = model.strip_prefix("models/").unwrap_or(model);

    match model.rsplit_once(':') {
        Some((id, method)) if method.chars().all(|c| c.is_ascii_alphabetic()) => id.to_string(),
        _ => model.to_string(),
    }
}

/// `{base}/models/{model}:generateContent`: the base is whatever comes before `/models/` in the
/// endpoint (`…/v1beta` when only the host is given); a query string is dropped, the key always
/// goes in a header.
pub fn request_url(endpoint: &str, model: &str) -> String {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return format!(
            "{}/models/{}:generateContent",
            endpoint.trim_end_matches('/'),
            model_id(model)
        );
    };

    let mut path = url.path().trim_end_matches('/').to_string();
    if let Some(at) = format!("{path}/").find("/models/") {
        path.truncate(at);
    }
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();

    format!(
        "{}://{}{}{}/models/{}:generateContent",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        port,
        if path.is_empty() { "/v1beta" } else { &path },
        model_id(model)
    )
}

pub fn to_body(
    messages: &[Value],
    tools: &[Value],
    tool_choice: &str,
    model: &str,
    signatures: &HashMap<String, String>,
) -> Value {
    let mut system: Vec<String> = Vec::new();
    let mut contents: Vec<(String, Vec<Value>)> = Vec::new();
    let mut names: HashMap<String, String> = HashMap::new();
    let needs_signatures = !model_id(model).to_lowercase().starts_with("gemma");

    for message in messages {
        match str_of(message, "role") {
            "system" => {
                let text = chat::text(&message["content"]);
                if !text.trim().is_empty() {
                    system.push(text.trim().to_string());
                }
            }
            "tool" => {
                let id = str_of(message, "tool_call_id").to_string();
                let content = chat::text(&message["content"]);
                let content = if content.is_empty() {
                    "(no output)".to_string()
                } else {
                    content
                };
                let key = if message["is_error"].as_bool().unwrap_or(false) {
                    "error"
                } else {
                    "output"
                };
                let name =
                    names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| match str_of(message, "name") {
                            "" => "tool".to_string(),
                            name => name.to_string(),
                        });
                let mut response = json!({"name": name, "response": {key: content}});
                if !id.is_empty() {
                    response["id"] = Value::String(id);
                }
                append(
                    &mut contents,
                    "user",
                    vec![json!({"functionResponse": response})],
                );
            }
            "assistant" => {
                let mut parts = Vec::new();
                let text = chat::text(&message["content"]);
                if !text.trim().is_empty() {
                    parts.push(json!({"text": text.trim()}));
                }
                let mut signed = false;
                let mut first_call = None;
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    let id = str_of(call, "id").to_string();
                    let name = call["function"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    names.insert(id.clone(), name.clone());

                    let mut function_call = json!({
                        "name": name,
                        "args": chat::decode_arguments(&call["function"]["arguments"]),
                    });
                    if !id.is_empty() {
                        function_call["id"] = Value::String(id.clone());
                    }
                    let mut part = json!({"functionCall": function_call});
                    if let Some(signature) = signatures.get(&id).filter(|s| !s.is_empty()) {
                        part["thoughtSignature"] = Value::String(signature.clone());
                        signed = true;
                    }
                    first_call.get_or_insert(parts.len());
                    parts.push(part);
                }
                // Gemini only checks the first function call of a turn
                if needs_signatures
                    && !signed
                    && let Some(index) = first_call
                {
                    parts[index]["thoughtSignature"] = Value::String(SKIP_SIGNATURE.to_string());
                }
                if !parts.is_empty() {
                    append(&mut contents, "model", parts);
                }
            }
            _ => {
                let parts: Vec<Value> = chat::parts(&message["content"])
                    .into_iter()
                    .filter_map(|part| match part {
                        Part::Image { mime, data } => {
                            Some(json!({"inlineData": {"mimeType": mime, "data": data}}))
                        }
                        Part::Text(text) if !text.trim().is_empty() => Some(json!({"text": text})),
                        Part::Text(_) => None,
                    })
                    .collect();
                if !parts.is_empty() {
                    append(&mut contents, "user", parts);
                }
            }
        }
    }

    if contents.first().is_none_or(|(role, _)| role != "user") {
        contents.insert(
            0,
            (
                "user".into(),
                vec![json!({"text": "(conversation continues)"})],
            ),
        );
    }
    if contents.last().is_some_and(|(role, _)| role == "model") {
        contents.push(("user".into(), vec![json!({"text": "(continue)"})]));
    }

    let mut body = json!({
        "contents": contents
            .into_iter()
            .map(|(role, parts)| json!({"role": role, "parts": parts}))
            .collect::<Vec<_>>(),
    });
    if !system.is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": system.join("\n\n")}]});
    }
    if !tools.is_empty() {
        let declarations: Vec<Value> = tools
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
                    "parametersJsonSchema": schema,
                })
            })
            .collect();
        body["tools"] = json!([{"functionDeclarations": declarations}]);
        body["toolConfig"] = json!({"functionCallingConfig": {"mode": match tool_choice {
            "none" => "NONE",
            "required" => "ANY",
            _ => "AUTO",
        }}});
    }

    body
}

/// Appends parts, merged into the previous turn when the role repeats; in a merged user turn the
/// function responses come first.
fn append(contents: &mut Vec<(String, Vec<Value>)>, role: &str, parts: Vec<Value>) {
    if let Some((last_role, last)) = contents.last_mut()
        && last_role == role
    {
        last.extend(parts);
        if role == "user" {
            last.sort_by_key(|part| u8::from(part.get("functionResponse").is_none()));
        }
        return;
    }

    contents.push((role.to_string(), parts));
}

/// The answer, or `None` when it is not a usable `generateContent` answer. `signatures` holds
/// the thought signature of each tool call that came with one.
pub fn from_response(raw: &str) -> Option<Answer> {
    let json: Value = serde_json::from_str(raw).ok()?;
    let candidate = json["candidates"].get(0).filter(|c| c.is_object());
    if candidate.is_none() && json["promptFeedback"]["blockReason"].is_null() {
        return None;
    }

    let finish = candidate
        .map(|c| c["finishReason"].as_str().unwrap_or("STOP"))
        .unwrap_or("SAFETY");
    if matches!(finish, "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL") {
        return None;
    }

    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut signatures = Vec::new();
    let parts = candidate
        .and_then(|c| c["content"]["parts"].as_array())
        .cloned()
        .unwrap_or_default();
    for part in &parts {
        // thought summaries come back even unasked; they are not part of the answer
        if part["thought"].as_bool().unwrap_or(false) {
            continue;
        }
        if let Some(piece) = part["text"].as_str() {
            text.push_str(piece);
        }
        if let Some(name) = part["functionCall"]["name"]
            .as_str()
            .filter(|n| !n.is_empty())
        {
            // Gemini's own ids can repeat across turns; ours never do
            let id = chat::new_call_id();
            let args = &part["functionCall"]["args"];
            let args = if args.is_object() {
                args.clone()
            } else {
                json!({})
            };
            tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {"name": name, "arguments": args.to_string()},
            }));
            if let Some(signature) = part["thoughtSignature"].as_str().filter(|s| !s.is_empty()) {
                signatures.push((id, signature.to_string()));
            }
        }
    }

    let usage = &json["usageMetadata"];
    let text = text.trim().to_string();
    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else {
        match finish {
            "MAX_TOKENS" => "length",
            "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII"
            | "IMAGE_SAFETY" => "content_filter",
            _ => "stop",
        }
    };

    Some(Answer {
        content: (!text.is_empty()).then_some(text),
        tool_calls,
        finish_reason,
        prompt_tokens: if usage.is_object() {
            usage["promptTokenCount"].as_i64().unwrap_or(0)
                + usage["toolUsePromptTokenCount"].as_i64().unwrap_or(0)
        } else {
            (raw.len() as i64 + 3) / 4
        },
        // thinking is billed as output
        completion_tokens: usage["candidatesTokenCount"].as_i64().unwrap_or(0)
            + usage["thoughtsTokenCount"].as_i64().unwrap_or(0),
        signatures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_request_urls() {
        assert_eq!(
            request_url(
                "https://generativelanguage.googleapis.com",
                "models/gemini-3-flash"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash:generateContent"
        );
        assert_eq!(
            request_url(
                "https://generativelanguage.googleapis.com/v1beta/models/x:generateContent?key=1",
                "gemma-4-31b-it:generateContent"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemma-4-31b-it:generateContent"
        );
    }

    #[test]
    fn signs_the_first_call_and_names_responses() {
        let messages = [
            json!({"role": "user", "content": "go"}),
            json!({"role": "assistant", "content": null, "tool_calls": [
                {"id": "call_a", "function": {"name": "read_file", "arguments": "{\"path\":\"/a\"}"}},
                {"id": "call_b", "function": {"name": "read_file", "arguments": "{}"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_a", "content": "A"}),
            json!({"role": "tool", "tool_call_id": "call_b", "content": "B", "is_error": true}),
        ];

        let body = to_body(&messages, &[], "auto", "gemini-3-flash", &HashMap::new());
        let model = &body["contents"][1];
        assert_eq!(model["role"], "model");
        assert_eq!(model["parts"][0]["thoughtSignature"], SKIP_SIGNATURE);
        assert!(model["parts"][1].get("thoughtSignature").is_none());
        let user = &body["contents"][2];
        assert_eq!(user["parts"][0]["functionResponse"]["name"], "read_file");
        assert_eq!(
            user["parts"][1]["functionResponse"]["response"]["error"],
            "B"
        );
        assert_eq!(body["contents"].as_array().unwrap().len(), 3);

        let gemma = to_body(&messages, &[], "auto", "gemma-4", &HashMap::new());
        assert!(
            gemma["contents"][1]["parts"][0]
                .get("thoughtSignature")
                .is_none()
        );

        let signed = to_body(
            &messages,
            &[],
            "auto",
            "gemini-3-flash",
            &HashMap::from([("call_b".to_string(), "sig".to_string())]),
        );
        assert_eq!(signed["contents"][1]["parts"][1]["thoughtSignature"], "sig");
        assert!(
            signed["contents"][1]["parts"][0]
                .get("thoughtSignature")
                .is_none()
        );
    }

    #[test]
    fn reads_calls_signatures_and_blocks() {
        let answer = from_response(
            r#"{"candidates":[{"finishReason":"STOP","content":{"parts":[
                {"text":"thinking...","thought":true},
                {"text":"Done."},
                {"functionCall":{"name":"power","args":{"signal":"stop"}},"thoughtSignature":"s1"}
            ]}}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":2,"thoughtsTokenCount":3}}"#,
        )
        .unwrap();
        assert_eq!(answer.content.as_deref(), Some("Done."));
        assert_eq!(answer.tool_calls.len(), 1);
        assert_eq!(answer.signatures[0].1, "s1");
        assert_eq!((answer.prompt_tokens, answer.completion_tokens), (5, 5));

        let blocked = from_response(r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#).unwrap();
        assert_eq!(blocked.finish_reason, "content_filter");
        assert!(
            from_response(r#"{"candidates":[{"finishReason":"MALFORMED_FUNCTION_CALL"}]}"#)
                .is_none()
        );
    }
}
