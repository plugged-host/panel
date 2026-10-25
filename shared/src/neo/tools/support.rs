//! plugged.host: what neo's tools share: reading the model's arguments, paths, output caps.

use super::ToolError;
use serde_json::Value;

/// The most of a tool's output the model reads at once.
pub const OUTPUT_CHARS: usize = 12_000;

/// A model-given path as `/a/b`. Wings keeps every path inside the server's volume anyway;
/// refusing `..` and NUL here keeps what the model meant unambiguous.
pub fn path(value: &Value, default: &str) -> Result<String, ToolError> {
    let path = value
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(default);

    if path.contains('\0') || path.len() > 1024 {
        return Err(ToolError::invalid("Invalid path."));
    }

    let mut segments = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            "" | "." => continue,
            ".." => {
                return Err(ToolError::invalid(
                    "Paths may not contain \"..\". Use a path from the server root, like /plugins/config.yml.",
                ));
            }
            segment => segments.push(segment),
        }
    }

    Ok(format!("/{}", segments.join("/")))
}

/// A string argument of at most `max` characters; required ones may not be empty.
pub fn string<'a>(
    arguments: &'a Value,
    key: &str,
    max: usize,
    required: bool,
) -> Result<&'a str, ToolError> {
    match arguments.get(key) {
        None | Some(Value::Null) if !required => Ok(""),
        Some(Value::String(value)) if !(required && value.is_empty()) => {
            if value.chars().count() > max {
                return Err(ToolError::invalid(format!(
                    "\"{key}\" is too long (max {max} characters)."
                )));
            }

            Ok(value)
        }
        _ => Err(ToolError::invalid(format!(
            "\"{key}\" must be a non-empty string."
        ))),
    }
}

/// An integer argument, `default` when it is missing, clamped to `min..=max`.
pub fn integer(arguments: &Value, key: &str, default: i64, min: i64, max: i64) -> i64 {
    match arguments.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|n| n as i64))
            .unwrap_or(default),
        Some(Value::String(text)) => text.trim().parse().unwrap_or(default),
        _ => default,
    }
    .clamp(min, max)
}

pub fn boolean(arguments: &Value, key: &str, default: bool) -> bool {
    match arguments.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(text)) => match text.as_str() {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" => false,
            _ => default,
        },
        _ => default,
    }
}

/// A list of strings (paths, names), at most `max` of them.
pub fn strings(arguments: &Value, key: &str, max: usize) -> Result<Vec<String>, ToolError> {
    let list = match arguments.get(key) {
        Some(Value::Array(list)) => list,
        Some(Value::String(one)) => return Ok(vec![one.clone()]),
        _ => return Err(ToolError::invalid(format!("\"{key}\" must be a list."))),
    };
    if list.is_empty() || list.len() > max {
        return Err(ToolError::invalid(format!(
            "\"{key}\" must hold 1 to {max} entries."
        )));
    }

    list.iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| ToolError::invalid(format!("\"{key}\" must hold strings.")))
        })
        .collect()
}

/// The text cut to [`OUTPUT_CHARS`], saying how much was left out.
pub fn limit_output(text: &str) -> String {
    let count = text.chars().count();
    if count <= OUTPUT_CHARS {
        return text.to_string();
    }

    let cut: String = text.chars().take(OUTPUT_CHARS).collect();
    format!(
        "{cut}\n…[truncated, {} more characters]",
        count - OUTPUT_CHARS
    )
}

/// Secrets masked ([`crate::redact::redact`]).
pub fn redact(text: &str) -> String {
    crate::redact::redact(text)
}

/// The text's last [`OUTPUT_CHARS`] characters, for output whose newest part matters (the
/// console).
pub fn limit_tail(text: &str) -> String {
    let count = text.chars().count();
    if count <= OUTPUT_CHARS {
        return text.to_string();
    }

    let tail: String = text.chars().skip(count - OUTPUT_CHARS).collect();
    format!("…[earlier lines truncated]\n{tail}")
}

/// At most `max` characters, with `…` when cut (summaries and listings).
pub fn short(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }

    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// A lower-case message (the shared code's errors) as a sentence: capitalised, with a full stop.
pub fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    let mut sentence: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !sentence.ends_with(['.', '!', '?']) {
        sentence.push('.');
    }

    sentence
}

/// A string argument for a summary, never failing.
pub fn shown<'a>(arguments: &'a Value, key: &str) -> &'a str {
    arguments.get(key).and_then(Value::as_str).unwrap_or("?")
}

/// A path argument for a summary, never failing.
pub fn shown_path(arguments: &Value, key: &str) -> String {
    path(arguments.get(key).unwrap_or(&Value::Null), "/").unwrap_or_else(|_| "?".into())
}

/// Whether the text carries the secret mask: a value neo never saw, so it must not be written
/// back.
pub fn masked(text: &str) -> bool {
    text.contains(crate::redact::MASK)
}

/// A normalised path as its folder and its name: `/a/b/c` → (`/a/b`, `c`). The root has no name.
pub fn split(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some(("", name)) => ("/".into(), name.into()),
        Some((parent, name)) => (parent.into(), name.into()),
        None => ("/".into(), path.into()),
    }
}

/// Joins a folder and a name or relative path as `/a/b`.
pub fn join(directory: &str, name: &str) -> String {
    let name = name.trim_start_matches('/');
    if directory == "/" || directory.is_empty() {
        format!("/{name}")
    } else {
        format!("{}/{name}", directory.trim_end_matches('/'))
    }
}

/// The chat server's Wings client, refusing the paths a subuser may not see.
pub async fn wings(
    context: &super::ToolContext<'_>,
) -> Result<wings_api::client::WingsClient, ToolError> {
    let server = context.server()?;

    Ok(crate::minecraft::files::client(context.state, server).await?)
}

/// A Wings answer as the model hears it: a refusal (a missing file, a busy server) is something it
/// can act on; anything else is a failure.
pub fn wings_error(err: wings_api::client::ApiHttpError) -> ToolError {
    match err {
        wings_api::client::ApiHttpError::Http(status, error) if status.is_client_error() => {
            ToolError::invalid(format!("The node refused this: {}.", error.error))
        }
        err => ToolError::Failed(err.into()),
    }
}

/// The server's state on Wings as a word.
pub fn state_name(state: wings_api::ServerState) -> &'static str {
    match state {
        wings_api::ServerState::Offline => "offline",
        wings_api::ServerState::Starting => "starting",
        wings_api::ServerState::Stopping => "stopping",
        wings_api::ServerState::Running => "running",
    }
}

/// `1.5 KB`-style sizes.
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];

    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{} {}", (size * 10.0).round() / 10.0, UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalises_paths() {
        assert_eq!(
            path(&json!("plugins//a/./b.yml"), "/").unwrap(),
            "/plugins/a/b.yml"
        );
        assert_eq!(path(&json!(" "), "/").unwrap(), "/");
        assert_eq!(path(&Value::Null, "/logs").unwrap(), "/logs");
        assert_eq!(path(&json!("a\\b"), "/").unwrap(), "/a/b");
        assert!(path(&json!("/a/../etc"), "/").is_err());
        assert!(path(&json!("a\0b"), "/").is_err());
    }

    #[test]
    fn reads_arguments() {
        let arguments = json!({"name": "world", "empty": "", "n": "7", "f": 2.9, "on": "yes", "list": ["a", "b"]});
        assert_eq!(string(&arguments, "name", 10, true).unwrap(), "world");
        assert!(string(&arguments, "name", 3, true).is_err());
        assert!(string(&arguments, "empty", 10, true).is_err());
        assert_eq!(string(&arguments, "missing", 10, false).unwrap(), "");
        assert!(string(&arguments, "missing", 10, true).is_err());
        assert_eq!(integer(&arguments, "n", 1, 1, 5), 5);
        assert_eq!(integer(&arguments, "f", 1, 1, 5), 2);
        assert_eq!(integer(&arguments, "missing", 3, 1, 5), 3);
        assert!(boolean(&arguments, "on", false));
        assert_eq!(strings(&arguments, "list", 5).unwrap(), vec!["a", "b"]);
        assert!(strings(&arguments, "list", 1).is_err());
    }

    #[test]
    fn caps_output_and_sizes() {
        let long = "x".repeat(OUTPUT_CHARS + 5);
        assert!(limit_output(&long).ends_with("…[truncated, 5 more characters]"));
        assert_eq!(limit_output("short"), "short");
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KB");
        assert_eq!(human(5 * 1024 * 1024 * 1024), "5 GB");

        let console = format!("old\n{}", "y".repeat(OUTPUT_CHARS));
        assert!(limit_tail(&console).starts_with("…[earlier lines truncated]\ny"));
        assert!(limit_tail(&console).ends_with('y'));
        assert_eq!(limit_tail("short"), "short");
        assert_eq!(short("  abcdef ", 4), "abc…");
        assert_eq!(short("abc", 4), "abc");
        assert_eq!(sentence("not enough cpu left"), "Not enough cpu left.");
        assert_eq!(sentence("done."), "Done.");
        assert_eq!(sentence(" "), "");
    }

    #[test]
    fn splits_and_joins_paths() {
        assert_eq!(split("/a/b/c"), ("/a/b".into(), "c".into()));
        assert_eq!(split("/c"), ("/".into(), "c".into()));
        assert_eq!(split("/"), ("/".into(), "".into()));
        assert_eq!(join("/", "a.txt"), "/a.txt");
        assert_eq!(join("/plugins/", "/x.jar"), "/plugins/x.jar");
        assert_eq!(shown_path(&json!({"path": "a//b"}), "path"), "/a/b");
        assert_eq!(shown_path(&json!({"path": "../x"}), "path"), "?");
        assert_eq!(shown(&json!({}), "name"), "?");
        assert!(masked("password=••••"));
        assert!(!masked("password=hunter2"));
    }
}
