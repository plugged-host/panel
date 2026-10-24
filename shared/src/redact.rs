//! plugged.host: masking secrets in logs and file contents before they leave the panel (the
//! Minecraft log check, docs/features/minecraft.md).
//!
//! `key=value`, `key: value` and `"key": "value"` entries whose key looks secret (passwords,
//! RCON, tokens, API keys, webhooks), JWTs, Discord bot tokens, the token half of Discord webhook
//! URLs and common API key prefixes.

use regex::Regex;
use std::sync::LazyLock;

pub const MASK: &str = "••••";

const SECRET_KEY: &str = r"[\w.\-]*(?:password|passwd|secret|token|api[_\-]?key|apikey|rcon[_.\-]?pass|private[_\-]?key|webhook|credential|license[_\-]?key)[\w.\-]*";

static PATTERNS: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    [
        // "key": "value" (JSON)
        (
            format!(r#"(?i)("{SECRET_KEY}"\s*:\s*")[^"]+"#),
            format!("${{1}}{MASK}"),
        ),
        // key=value / key: value (properties, YAML, TOML, env)
        (
            format!(r#"(?im)^(\s*{SECRET_KEY}\s*[=:]\s*["']?)[^\s"'#]+"#),
            format!("${{1}}{MASK}"),
        ),
        // Discord webhook URLs: the id stays, the token goes
        (
            r"(?i)(discord(?:app)?\.com/api/webhooks/\d+/)[\w\-]+".to_string(),
            format!("${{1}}{MASK}"),
        ),
        // JWTs
        (
            r"\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\b".to_string(),
            MASK.to_string(),
        ),
        // Discord bot tokens
        (
            r"\b[MNO][A-Za-z\d_\-]{23,27}\.[\w\-]{6}\.[\w\-]{27,40}\b".to_string(),
            MASK.to_string(),
        ),
        // well-known API key prefixes
        (
            r"\b(?:sk-[A-Za-z0-9_\-]{16,}|gsk_[A-Za-z0-9]{16,}|ghp_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9\-]{10,})\b"
                .to_string(),
            MASK.to_string(),
        ),
    ]
    .into_iter()
    .map(|(pattern, replacement)| {
        (
            Regex::new(&pattern).expect("the secret patterns are valid"),
            replacement,
        )
    })
    .collect()
});

static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\r")
        .expect("the ANSI pattern is valid")
});

/// The text with every secret it recognises replaced by [`MASK`].
pub fn redact(text: &str) -> String {
    let mut text = text.to_string();
    for (pattern, replacement) in PATTERNS.iter() {
        if let std::borrow::Cow::Owned(replaced) = pattern.replace_all(&text, replacement.as_str())
        {
            text = replaced;
        }
    }

    text
}

/// The text without terminal colour codes and carriage returns.
pub fn strip_ansi(text: &str) -> String {
    ANSI.replace_all(text, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_masked() {
        assert_eq!(
            redact("rcon.password=hunter2"),
            format!("rcon.password={MASK}")
        );
        assert_eq!(
            redact("  api-key: 'abc123'"),
            format!("  api-key: '{MASK}'")
        );
        assert_eq!(
            redact(r#"{"bot_token": "abc.def"}"#),
            format!(r#"{{"bot_token": "{MASK}"}}"#)
        );
        assert_eq!(
            redact("see https://discord.com/api/webhooks/123/AbC-def_9"),
            format!("see https://discord.com/api/webhooks/123/{MASK}")
        );
        assert_eq!(
            redact("Bearer eyJhbGciOiJI.eyJzdWIiOiIx.SflKxwRJSMeKKF2Q"),
            format!("Bearer {MASK}")
        );
        assert_eq!(redact("key sk-abcdefghijklmnop1234"), format!("key {MASK}"));
        assert_eq!(redact("motd=A Minecraft Server"), "motd=A Minecraft Server");
        assert_eq!(
            redact("[12:00:00 INFO]: Done (3.2s)!"),
            "[12:00:00 INFO]: Done (3.2s)!"
        );
    }

    #[test]
    fn colours_are_stripped() {
        assert_eq!(strip_ansi("\x1b[33;1mWarn\x1b[0m\r\n"), "Warn\n");
    }
}
