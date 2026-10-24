//! plugged.host: the chat shape neo works in, as noprofit.space's neo gateway takes it (OpenAI
//! style: `system` / `user` / `assistant` / `tool` messages, `tool_calls` with JSON-string
//! arguments). A message's content is a string, or for a user message with images a list of
//! parts (`{type: text, text}`, `{type: image_url, image_url: {url: data:…}}`).

use serde_json::{Value, json};
use std::sync::LazyLock;

/// Images one model call carries at most.
pub const MAX_IMAGES: usize = 4;

/// What a model that cannot see images gets in place of one.
pub const IMAGE_PLACEHOLDER: &str =
    "[An image was attached here, but the model answering now cannot see images.]";

static DATA_URL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^data:(image/(?:png|jpeg|webp|gif));base64,([A-Za-z0-9+/]+={0,2})$")
        .unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Image { mime: String, data: String },
}

/// A content value's parts: its text, or its text and image parts.
pub fn parts(content: &Value) -> Vec<Part> {
    match content {
        Value::Array(items) => items
            .iter()
            .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") => part
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map(|text| Part::Text(text.to_string())),
                Some("image_url") => part
                    .pointer("/image_url/url")
                    .and_then(Value::as_str)
                    .and_then(|url| DATA_URL.captures(url))
                    .map(|captures| Part::Image {
                        mime: captures[1].to_string(),
                        data: captures[2].to_string(),
                    }),
                _ => None,
            })
            .collect(),
        Value::String(text) if !text.is_empty() => vec![Part::Text(text.clone())],
        Value::Null | Value::String(_) => Vec::new(),
        other => vec![Part::Text(other.to_string())],
    }
}

/// The text parts joined, images left out.
pub fn text(content: &Value) -> String {
    parts(content)
        .into_iter()
        .filter_map(|part| match part {
            Part::Text(text) => Some(text),
            Part::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The images (data URLs) followed by the text; plain text without images.
pub fn with_images(text: &str, images: &[String]) -> Value {
    if images.is_empty() {
        return Value::String(text.to_string());
    }

    let mut parts: Vec<Value> = images
        .iter()
        .map(|url| json!({"type": "image_url", "image_url": {"url": url}}))
        .collect();
    if !text.trim().is_empty() {
        parts.push(json!({"type": "text", "text": text}));
    }

    Value::Array(parts)
}

pub fn has_images(messages: &[Value]) -> bool {
    messages.iter().any(|message| {
        message.get("content").is_some_and(|content| {
            content.is_array()
                && parts(content)
                    .iter()
                    .any(|part| matches!(part, Part::Image { .. }))
        })
    })
}

/// The messages for a model that cannot see images: plain text, with a note where each was.
pub fn without_images(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let Some(content) = message.get("content").filter(|content| content.is_array()) else {
                return message.clone();
            };

            let text = parts(content)
                .into_iter()
                .map(|part| match part {
                    Part::Text(text) => text,
                    Part::Image { .. } => IMAGE_PLACEHOLDER.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            let mut message = message.clone();
            message["content"] = Value::String(text);
            message
        })
        .collect()
}

/// A message's string field, or `""`.
pub(crate) fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// A tool call's arguments as an object; anything else is `{}`.
pub(crate) fn decode_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::Object(_) => arguments.clone(),
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(value @ Value::Object(_)) => value,
            _ => json!({}),
        },
        _ => json!({}),
    }
}

/// A unique tool call id matching `^[A-Za-z0-9_-]{1,64}$` (providers' own ids can repeat or
/// carry other characters).
pub(crate) fn tool_call_id(id: Option<&str>, seen: &mut Vec<String>) -> String {
    let cleaned: String = id
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(64)
        .collect();

    let id = if cleaned.is_empty() || seen.contains(&cleaned) {
        new_call_id()
    } else {
        cleaned
    };
    seen.push(id.clone());

    id
}

pub(crate) fn new_call_id() -> String {
    use rand::distr::SampleString;

    format!(
        "call_{}",
        rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 24)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

    #[test]
    fn reads_text_and_image_parts() {
        let content = with_images("what is this?", &[PNG.to_string()]);
        assert_eq!(
            parts(&content),
            vec![
                Part::Image {
                    mime: "image/png".into(),
                    data: "iVBORw0KGgo=".into()
                },
                Part::Text("what is this?".into())
            ]
        );
        assert_eq!(text(&content), "what is this?");
        assert_eq!(with_images("hi", &[]), json!("hi"));
    }

    #[test]
    fn a_model_without_vision_gets_a_note() {
        let messages = vec![
            json!({"role": "system", "content": "be nice"}),
            json!({"role": "user", "content": with_images("look", &[PNG.to_string()])}),
        ];
        assert!(has_images(&messages));

        let plain = without_images(&messages);
        assert!(!has_images(&plain));
        assert_eq!(
            plain[1]["content"],
            json!(format!("{IMAGE_PLACEHOLDER}\n\nlook"))
        );
        assert_eq!(plain[0], messages[0]);
    }

    #[test]
    fn tool_call_ids_are_unique_and_clean() {
        let mut seen = Vec::new();
        assert_eq!(tool_call_id(Some("toolu_01:abc"), &mut seen), "toolu_01abc");
        let again = tool_call_id(Some("toolu_01abc"), &mut seen);
        assert!(again.starts_with("call_") && again.len() == 29);
        assert!(tool_call_id(None, &mut seen).starts_with("call_"));
    }
}
