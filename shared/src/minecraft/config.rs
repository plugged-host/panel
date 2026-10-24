//! plugged.host: the game settings files of a Minecraft server (docs/features/minecraft.md):
//! `server.properties` and, on plugin servers, `bukkit.yml`, `spigot.yml`, Paper's and Purpur's
//! YAML. Every file is edited value by value on its own lines, so comments, key order, blank lines
//! and keys the panel does not know stay exactly as they are.

use super::{Loader, files};
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Larger files are not offered: no settings file is near this.
const MAX_SIZE: u64 = 512 * 1024;

/// How many values one save may change.
pub const MAX_EDITS: usize = 1000;

/// `server.properties` keys the panel manages: Wings writes the address and ports from the
/// server's allocations on every start, and the RCON password is a secret.
const MANAGED: &[&str] = &[
    "server-ip",
    "server-port",
    "query.port",
    "rcon.port",
    "rcon.password",
];

const TOGGLES: &[&str] = &[
    "accepts-transfers",
    "allow-flight",
    "allow-nether",
    "broadcast-console-to-ops",
    "broadcast-rcon-to-ops",
    "debug",
    "enable-command-block",
    "enable-jmx-monitoring",
    "enable-query",
    "enable-rcon",
    "enable-status",
    "enforce-secure-profile",
    "enforce-whitelist",
    "force-gamemode",
    "generate-structures",
    "hardcore",
    "hide-online-players",
    "log-ips",
    "online-mode",
    "prevent-proxy-connections",
    "pvp",
    "require-resource-pack",
    "spawn-animals",
    "spawn-monsters",
    "spawn-npcs",
    "sync-chunk-writes",
    "use-native-transport",
    "white-list",
];

const NUMBERS: &[&str] = &[
    "entity-broadcast-range-percentage",
    "function-permission-level",
    "max-chained-neighbor-updates",
    "max-players",
    "max-tick-time",
    "max-world-size",
    "network-compression-threshold",
    "op-permission-level",
    "pause-when-empty-seconds",
    "player-idle-timeout",
    "rate-limit",
    "simulation-distance",
    "spawn-protection",
    "view-distance",
];

const CHOICES: &[(&str, &[&str])] = &[
    (
        "gamemode",
        &["survival", "creative", "adventure", "spectator"],
    ),
    ("difficulty", &["peaceful", "easy", "normal", "hard"]),
    (
        "level-type",
        &[
            "minecraft:normal",
            "minecraft:flat",
            "minecraft:large_biomes",
            "minecraft:amplified",
            "minecraft:single_biome_surface",
        ],
    ),
    ("region-file-compression", &["deflate", "lz4", "none"]),
];

/// The values a `server.properties` key takes when it is a choice (the worlds page offers the
/// level types).
pub fn choices(key: &str) -> &'static [&'static str] {
    CHOICES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, choices)| *choices)
        .unwrap_or_default()
}

/// A settings file and the names it may have: the first one that exists is used
/// (`config/paper-global.yml` replaced `paper.yml` in Paper 1.19).
struct Candidate {
    id: &'static str,
    paths: &'static [&'static str],
    /// Only plugin servers (Paper and its forks) have it.
    plugins: bool,
}

const FILES: &[Candidate] = &[
    Candidate {
        id: "properties",
        paths: &["server.properties"],
        plugins: false,
    },
    Candidate {
        id: "bukkit",
        paths: &["bukkit.yml"],
        plugins: true,
    },
    Candidate {
        id: "spigot",
        paths: &["spigot.yml"],
        plugins: true,
    },
    Candidate {
        id: "paper",
        paths: &["config/paper-global.yml", "paper.yml"],
        plugins: true,
    },
    Candidate {
        id: "purpur",
        paths: &["purpur.yml"],
        plugins: true,
    },
];

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Properties,
    Yaml,
}

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Toggle,
    Number,
    Choice,
    Text,
}

/// One value of a file. `path` is the key, or the keys from the top for YAML.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Field {
    pub path: Vec<String>,
    pub kind: FieldKind,
    #[schema(value_type = Object)]
    pub value: serde_json::Value,
    pub choices: Vec<&'static str>,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct ConfigFile {
    pub id: &'static str,
    pub path: &'static str,
    pub format: Format,
    pub fields: Vec<Field>,
}

#[derive(ToSchema, Deserialize, Debug, Clone)]
pub struct Edit {
    pub path: Vec<String>,
    #[schema(value_type = Object)]
    pub value: serde_json::Value,
}

#[derive(Debug)]
pub enum ConfigError {
    UnknownFile,
    Missing,
    Invalid(String),
    Wings(anyhow::Error),
}

impl ConfigError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let (message, status) = match self {
            Self::UnknownFile => (
                "this server has no such settings file".into(),
                StatusCode::NOT_FOUND,
            ),
            Self::Missing => (
                "the file is not on the server anymore, start the server once so it writes it"
                    .into(),
                StatusCode::NOT_FOUND,
            ),
            Self::Invalid(message) => (message, StatusCode::BAD_REQUEST),
            Self::Wings(err) => {
                tracing::warn!("minecraft settings file: {err:#}");
                (
                    "the server's files could not be reached, try again in a moment".into(),
                    StatusCode::BAD_GATEWAY,
                )
            }
        };

        crate::response::ApiResponse::error(&message).with_status(status)
    }
}

fn candidates(loader: Option<Loader>) -> impl Iterator<Item = &'static Candidate> {
    FILES
        .iter()
        .filter(move |candidate| !candidate.plugins || loader == Some(Loader::Paper))
}

/// The settings files the server has, with their values.
pub async fn load(
    state: &crate::State,
    server: &Server,
    loader: Option<Loader>,
) -> Result<Vec<ConfigFile>, anyhow::Error> {
    let client = files::client(state, server).await?;
    let mut out = Vec::new();

    for candidate in candidates(loader) {
        for path in candidate.paths {
            let Some(raw) = files::read_text(&client, server, path, MAX_SIZE).await? else {
                continue;
            };

            out.push(describe(candidate.id, path, &raw));
            break;
        }
    }

    Ok(out)
}

fn describe(id: &'static str, path: &'static str, raw: &str) -> ConfigFile {
    if path.ends_with(".properties") {
        ConfigFile {
            id,
            path,
            format: Format::Properties,
            fields: properties(raw)
                .into_iter()
                .map(|(key, value)| property_field(key, value))
                .collect(),
        }
    } else {
        ConfigFile {
            id,
            path,
            format: Format::Yaml,
            fields: yaml_leaves(raw)
                .into_iter()
                .map(|leaf| leaf.field())
                .collect(),
        }
    }
}

/// Applies the edits to the file as it is now (read again, so only keys it still has are
/// written) and returns the file's path and how many values changed.
pub async fn save(
    state: &crate::State,
    server: &Server,
    loader: Option<Loader>,
    id: &str,
    edits: &[Edit],
) -> Result<(&'static str, usize), ConfigError> {
    let candidate = candidates(loader)
        .find(|candidate| candidate.id == id)
        .ok_or(ConfigError::UnknownFile)?;

    let client = files::client(state, server)
        .await
        .map_err(ConfigError::Wings)?;

    for path in candidate.paths {
        let Some(raw) = files::read_text(&client, server, path, MAX_SIZE)
            .await
            .map_err(ConfigError::Wings)?
        else {
            continue;
        };

        let (written, changed) = if path.ends_with(".properties") {
            apply_properties(&raw, edits)
        } else {
            apply_yaml(&raw, edits)
        }
        .map_err(ConfigError::Invalid)?;

        if changed > 0 {
            files::write_text(&client, server, path, written)
                .await
                .map_err(ConfigError::Wings)?;
        }

        return Ok((path, changed));
    }

    Err(ConfigError::Missing)
}

fn one_line(path: &[String], value: &serde_json::Value) -> Result<String, String> {
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        _ => return Err(format!("{} must be text", path.join("."))),
    };

    if text.chars().any(char::is_control) {
        return Err(format!("{} must be one line", path.join(".")));
    }

    Ok(text)
}

// ---------- server.properties ----------

/// The key a line sets, or `None` for a comment, a blank line or no `key=value`.
fn property_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return None;
    }

    let position = line.find('=')?;
    let key = line[..position].trim();

    (!key.is_empty()).then_some(key)
}

/// The keys and values of a `server.properties`, in file order, the managed keys left out.
pub fn properties(raw: &str) -> Vec<(String, String)> {
    raw.lines()
        .filter_map(|line| {
            let key = property_key(line)?;
            if MANAGED.contains(&key) {
                return None;
            }

            Some((key.to_string(), unescape(&line[line.find('=')? + 1..])))
        })
        .collect()
}

fn property_kind(key: &str, value: &str) -> (FieldKind, Vec<&'static str>) {
    if let Some((_, choices)) = CHOICES.iter().find(|(name, _)| *name == key)
        && choices.contains(&value)
    {
        return (FieldKind::Choice, choices.to_vec());
    }

    if TOGGLES.contains(&key) || value == "true" || value == "false" {
        (FieldKind::Toggle, Vec::new())
    } else if NUMBERS.contains(&key) || (!value.is_empty() && value.parse::<i64>().is_ok()) {
        (FieldKind::Number, Vec::new())
    } else {
        (FieldKind::Text, Vec::new())
    }
}

fn property_field(key: String, value: String) -> Field {
    let (kind, choices) = property_kind(&key, &value);

    let json = match kind {
        FieldKind::Toggle => serde_json::Value::Bool(value.trim().eq_ignore_ascii_case("true")),
        FieldKind::Number => value
            .trim()
            .parse::<i64>()
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::String(value)),
        _ => serde_json::Value::String(value),
    };

    Field {
        path: vec![key],
        kind,
        value: json,
        choices,
    }
}

/// An edit as the text Minecraft reads: booleans spelled out, numbers whole, one line.
fn property_value(kind: FieldKind, choices: &[&str], edit: &Edit) -> Result<String, String> {
    let key = edit.path.join(".");

    match kind {
        FieldKind::Toggle => match &edit.value {
            serde_json::Value::Bool(flag) => Ok(flag.to_string()),
            _ => Err(format!("{key} must be on or off")),
        },
        FieldKind::Number => match &edit.value {
            serde_json::Value::Number(number) if number.is_i64() => Ok(number.to_string()),
            serde_json::Value::String(text) if text.trim().parse::<i64>().is_ok() => {
                Ok(text.trim().to_string())
            }
            _ => Err(format!("{key} must be a whole number")),
        },
        FieldKind::Choice => {
            let value = one_line(&edit.path, &edit.value)?;
            if choices.contains(&value.as_str()) {
                Ok(value)
            } else {
                Err(format!("{value} is not a value {key} takes"))
            }
        }
        FieldKind::Text => one_line(&edit.path, &edit.value),
    }
}

/// Writes the edits for keys the file has; returns the new file and how many values changed.
fn apply_properties(raw: &str, edits: &[Edit]) -> Result<(String, usize), String> {
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
    let mut changed = 0;

    for edit in edits {
        let [key] = edit.path.as_slice() else {
            continue;
        };
        if MANAGED.contains(&key.as_str()) {
            continue;
        }

        let Some(index) = lines
            .iter()
            .position(|line| property_key(line) == Some(key.as_str()))
        else {
            continue;
        };

        let current = unescape(&lines[index][lines[index].find('=').unwrap_or_default() + 1..]);
        let (kind, choices) = property_kind(key, &current);
        let value = property_value(kind, &choices, edit)?;

        if value != current {
            lines[index] = format!("{key}={}", escape(&value));
            changed += 1;
        }
    }

    Ok((join(lines), changed))
}

/// Sets keys in a `server.properties`, adding the ones it lacks at the end (the players page sets
/// `white-list` on a file that may not have it yet).
pub fn set_properties(raw: &str, values: &[(&str, String)]) -> String {
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();

    for (key, value) in values {
        let value: String = value.chars().filter(|c| !c.is_control()).collect();

        match lines
            .iter()
            .position(|line| property_key(line) == Some(*key))
        {
            Some(index) => lines[index] = format!("{key}={}", escape(&value)),
            None => {
                while lines.last().is_some_and(|line| line.trim().is_empty()) {
                    lines.pop();
                }
                lines.push(format!("{key}={}", escape(&value)));
            }
        }
    }

    join(lines)
}

/// A `server.properties` value by key.
pub fn property(raw: &str, key: &str) -> Option<String> {
    raw.lines()
        .find(|line| property_key(line) == Some(key))
        .and_then(|line| {
            line.find('=')
                .map(|position| unescape(&line[position + 1..]))
        })
}

/// A value as Java's `Properties.load` reads it: leading blanks skipped, `\:` `\=` `\\` and the
/// like taken literally, `\t` `\n` `\r` `\f` and `\uXXXX` decoded (Minecraft writes
/// `level-type=minecraft\:normal` and older versions `motd=\u00A7a…`).
fn unescape(value: &str) -> String {
    let value = value.trim_start_matches([' ', '\t', '\x0c']);
    let mut units: Vec<u16> = Vec::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buffer = [0; 2];
            units.extend_from_slice(c.encode_utf16(&mut buffer));
            continue;
        }

        match chars.next() {
            Some('t') => units.push('\t' as u16),
            Some('n') => units.push('\n' as u16),
            Some('r') => units.push('\r' as u16),
            Some('f') => units.push(0x0c),
            Some('u') => {
                let hex: String = chars.clone().take(4).collect();
                match u16::from_str_radix(&hex, 16) {
                    Ok(unit) if hex.len() == 4 => {
                        units.push(unit);
                        for _ in 0..4 {
                            chars.next();
                        }
                    }
                    _ => units.push('u' as u16),
                }
            }
            Some(other) => {
                let mut buffer = [0; 2];
                units.extend_from_slice(other.encode_utf16(&mut buffer));
            }
            None => {}
        }
    }

    String::from_utf16_lossy(&units)
}

/// A value as Java's `Properties.store` writes it, so every Minecraft version reads it back the
/// same: `\\ : = # !` and a leading space escaped, anything outside printable ASCII as `\uXXXX`.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for (index, c) in value.chars().enumerate() {
        match c {
            '\\' | ':' | '=' | '#' | '!' => {
                out.push('\\');
                out.push(c);
            }
            ' ' if index == 0 => out.push_str("\\ "),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\x0c' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut buffer = [0; 2];
                for unit in c.encode_utf16(&mut buffer) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }

    out
}

fn join(lines: Vec<String>) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

// ---------- YAML ----------

#[derive(Debug, Clone, PartialEq)]
enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Null,
    Text(String, Style),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Single,
    Double,
}

/// A value on a line of its own (`key: value  # comment`): the only kind the form edits. Lists,
/// multi-line and flow values are left out and stay as they are.
#[derive(Debug)]
struct Leaf {
    path: Vec<String>,
    line: usize,
    /// The value's bytes on the line, the comment after it excluded.
    start: usize,
    end: usize,
    scalar: Scalar,
    multiline: bool,
}

impl Leaf {
    fn field(self) -> Field {
        let (kind, value) = match self.scalar {
            Scalar::Bool(flag) => (FieldKind::Toggle, serde_json::Value::Bool(flag)),
            Scalar::Int(number) => (FieldKind::Number, serde_json::Value::from(number)),
            Scalar::Float(number) => (
                FieldKind::Number,
                serde_json::Number::from_f64(number)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
            ),
            Scalar::Null => (FieldKind::Text, serde_json::Value::Null),
            Scalar::Text(text, _) => (FieldKind::Text, serde_json::Value::String(text)),
        };

        Field {
            path: self.path,
            kind,
            value,
            choices: Vec::new(),
        }
    }
}

enum Frame {
    Key {
        indent: usize,
        key: String,
    },
    /// A list item, a block or flow value: everything deeper belongs to it.
    Opaque {
        indent: usize,
    },
}

impl Frame {
    fn indent(&self) -> usize {
        match self {
            Self::Key { indent, .. } | Self::Opaque { indent } => *indent,
        }
    }
}

fn yaml_leaves(raw: &str) -> Vec<Leaf> {
    let mut leaves: Vec<Leaf> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // the last leaf, whose value continues on deeper lines if there are any
    let mut last: Option<(usize, usize)> = None;

    for (number, line) in raw.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim_start_matches(' ');
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();

        if indent == 0 && (trimmed.starts_with("---") || trimmed.starts_with("...")) {
            stack.clear();
            last = None;
            continue;
        }

        if let Some((leaf_indent, index)) = last {
            if indent > leaf_indent {
                leaves[index].multiline = true;
                continue;
            }
        }
        last = None;

        while stack.last().is_some_and(|frame| frame.indent() >= indent) {
            stack.pop();
        }
        if stack
            .iter()
            .any(|frame| matches!(frame, Frame::Opaque { .. }))
        {
            continue;
        }

        if trimmed == "-" || trimmed.starts_with("- ") || trimmed.starts_with('\t') {
            stack.push(Frame::Opaque { indent });
            continue;
        }

        let Some((key, after)) = split_key(trimmed) else {
            stack.push(Frame::Opaque { indent });
            continue;
        };

        let rest = &trimmed[after..];
        let value = rest.trim_start_matches([' ', '\t']);
        if value.is_empty() || value.starts_with('#') {
            stack.push(Frame::Key { indent, key });
            continue;
        }
        if value.starts_with(['[', '{', '|', '>', '&', '*', '!', '%', '@', '`']) {
            stack.push(Frame::Opaque { indent });
            continue;
        }

        let Some((scalar, length)) = parse_scalar(value) else {
            stack.push(Frame::Opaque { indent });
            continue;
        };

        let start = indent + after + (rest.len() - value.len());
        let mut path: Vec<String> = stack
            .iter()
            .filter_map(|frame| match frame {
                Frame::Key { key, .. } => Some(key.clone()),
                Frame::Opaque { .. } => None,
            })
            .collect();
        path.push(key);

        leaves.push(Leaf {
            path,
            line: number,
            start,
            end: start + length,
            scalar,
            multiline: false,
        });
        last = Some((indent, leaves.len() - 1));
    }

    leaves.retain(|leaf| !leaf.multiline);
    leaves
}

/// The key of a `key: …` line and where the text after its colon starts.
fn split_key(line: &str) -> Option<(String, usize)> {
    if line.starts_with(['"', '\'']) {
        let (key, length) = match parse_scalar(line.split(':').next()?)? {
            (Scalar::Text(key, _), length) => (key, length),
            _ => return None,
        };
        let rest = &line[length..];
        let after = rest.strip_prefix(':')?;
        if !after.is_empty() && !after.starts_with([' ', '\t']) {
            return None;
        }

        return Some((key, length + 1));
    }

    if line.starts_with(['?', ':', '#']) {
        return None;
    }

    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && index > 0 && bytes[index - 1] == b' ' {
            return None;
        }
        if *byte == b':' && (index + 1 == bytes.len() || matches!(bytes[index + 1], b' ' | b'\t')) {
            let key = line[..index].trim_end();
            return (!key.is_empty()).then(|| (key.to_string(), index + 1));
        }
    }

    None
}

/// A scalar at the start of `text` and its length, the comment after it excluded. `None` for one
/// that goes on over more lines or is not plain YAML this editor writes back.
fn parse_scalar(text: &str) -> Option<(Scalar, usize)> {
    let (scalar, length) = if let Some(inner) = text.strip_prefix('\'') {
        let mut value = String::new();
        let mut chars = inner.char_indices().peekable();
        let mut end = None;

        while let Some((index, c)) = chars.next() {
            if c == '\'' {
                if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                    chars.next();
                    value.push('\'');
                    continue;
                }
                end = Some(index + 2);
                break;
            }
            value.push(c);
        }

        (Scalar::Text(value, Style::Single), end?)
    } else if let Some(inner) = text.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = inner.char_indices();
        let mut end = None;

        while let Some((index, c)) = chars.next() {
            match c {
                '"' => {
                    end = Some(index + 2);
                    break;
                }
                '\\' => {
                    let (_, escaped) = chars.next()?;
                    match escaped {
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '/' => value.push('/'),
                        _ => return None,
                    }
                }
                c => value.push(c),
            }
        }

        (Scalar::Text(value, Style::Double), end?)
    } else {
        let mut length = text.len();
        let bytes = text.as_bytes();
        for index in 1..bytes.len() {
            if bytes[index] == b'#' && matches!(bytes[index - 1], b' ' | b'\t') {
                length = index;
                break;
            }
        }
        let plain = text[..length].trim_end_matches([' ', '\t']);

        (classify(plain), plain.len())
    };

    let rest = text[length..].trim_start_matches([' ', '\t']);
    (rest.is_empty() || rest.starts_with('#')).then_some((scalar, length))
}

fn classify(plain: &str) -> Scalar {
    match plain {
        "true" | "True" | "TRUE" => return Scalar::Bool(true),
        "false" | "False" | "FALSE" => return Scalar::Bool(false),
        "null" | "Null" | "NULL" | "~" => return Scalar::Null,
        _ => {}
    }

    let digits = plain.strip_prefix(['-', '+']).unwrap_or(plain);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(number) = plain.parse::<i64>() {
            return Scalar::Int(number);
        }
    }

    if is_float(digits)
        && let Ok(number) = plain.parse::<f64>()
    {
        return Scalar::Float(number);
    }

    Scalar::Text(plain.to_string(), Style::Plain)
}

/// YAML's float: `1.5`, `.5`, `1.`, `1e3`, `2.5E-4`.
fn is_float(text: &str) -> bool {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], Some(&text[index + 1..])),
        None => (text, None),
    };

    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            (!whole.is_empty() || !fraction.is_empty())
                && whole.bytes().all(|b| b.is_ascii_digit())
                && fraction.bytes().all(|b| b.is_ascii_digit())
        }
        None => !mantissa.is_empty() && mantissa.bytes().all(|b| b.is_ascii_digit()),
    };
    let exponent_ok = exponent.is_none_or(|exponent| {
        let digits = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    });

    mantissa_ok && exponent_ok && (mantissa.contains('.') || exponent.is_some())
}

/// A string as YAML that reads back as that same string.
fn render_text(text: &str, style: Style) -> String {
    let plain_safe = !text.is_empty()
        && text.trim() == text
        && !text.starts_with([
            '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%',
            '@', '`',
        ])
        && !text.contains(": ")
        && !text.contains(" #")
        && !text.ends_with(':')
        && matches!(classify(text), Scalar::Text(..));

    match style {
        Style::Double => format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")),
        Style::Plain if plain_safe => text.to_string(),
        _ => format!("'{}'", text.replace('\'', "''")),
    }
}

/// The edit as YAML of the leaf's type, `None` when the value is what the file has.
fn render(leaf: &Leaf, edit: &Edit) -> Result<Option<String>, String> {
    let key = edit.path.join(".");

    match &leaf.scalar {
        Scalar::Bool(current) => match edit.value {
            serde_json::Value::Bool(flag) if flag == *current => Ok(None),
            serde_json::Value::Bool(flag) => Ok(Some(flag.to_string())),
            _ => Err(format!("{key} must be on or off")),
        },
        Scalar::Int(current) => match edit.value.as_i64() {
            Some(number) if number == *current => Ok(None),
            Some(number) => Ok(Some(number.to_string())),
            None => Err(format!("{key} must be a whole number")),
        },
        Scalar::Float(current) => match edit.value.as_f64() {
            Some(number) if number == *current => Ok(None),
            Some(number) if number.is_finite() => Ok(Some(format!("{number:?}"))),
            _ => Err(format!("{key} must be a number")),
        },
        Scalar::Null => {
            let text = one_line(&edit.path, &edit.value)?;
            Ok((!text.is_empty()).then(|| render_text(&text, Style::Plain)))
        }
        Scalar::Text(current, style) => {
            let text = one_line(&edit.path, &edit.value)?;
            Ok((text != *current).then(|| render_text(&text, *style)))
        }
    }
}

fn apply_yaml(raw: &str, edits: &[Edit]) -> Result<(String, usize), String> {
    let leaves = yaml_leaves(raw);
    let mut lines: Vec<String> = raw.split('\n').map(str::to_string).collect();
    let mut edited = std::collections::HashSet::new();
    let mut changed = 0;

    for edit in edits {
        let Some(leaf) = leaves.iter().find(|leaf| leaf.path == edit.path) else {
            continue;
        };
        let Some(value) = render(leaf, edit)? else {
            continue;
        };
        // a second edit of the same value would land on shifted offsets
        if !edited.insert(leaf.line) {
            continue;
        }

        // the offsets are into the line without its `\r`, which stays after the value
        lines[leaf.line].replace_range(leaf.start..leaf.end, &value);
        changed += 1;
    }

    Ok((lines.join("\n"), changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(path: &[&str], value: serde_json::Value) -> Edit {
        Edit {
            path: path.iter().map(|part| part.to_string()).collect(),
            value,
        }
    }

    #[test]
    fn properties_keep_comments_and_hide_managed_keys() {
        let raw = "#Minecraft server properties\n#Fri Oct 24\npvp=true\nserver-port=25565\nmotd=A Minecraft Server\ndifficulty=easy\nmax-players=20\n";

        let keys: Vec<String> = properties(raw).into_iter().map(|(key, _)| key).collect();
        assert_eq!(keys, vec!["pvp", "motd", "difficulty", "max-players"]);

        let (written, changed) = apply_properties(
            raw,
            &[
                edit(&["pvp"], serde_json::json!(false)),
                edit(&["difficulty"], serde_json::json!("hard")),
                edit(&["max-players"], serde_json::json!(40)),
                edit(&["server-port"], serde_json::json!(1)),
                edit(&["unknown-key"], serde_json::json!("x")),
            ],
        )
        .unwrap();

        assert_eq!(changed, 3);
        assert_eq!(
            written,
            "#Minecraft server properties\n#Fri Oct 24\npvp=false\nserver-port=25565\nmotd=A Minecraft Server\ndifficulty=hard\nmax-players=40\n"
        );
    }

    #[test]
    fn properties_are_read_and_written_with_java_escapes() {
        let raw = "level-type=minecraft\\:normal\nmotd=\\u00A7aHi\\: there\n";

        let values = properties(raw);
        assert_eq!(values[0], ("level-type".into(), "minecraft:normal".into()));
        assert_eq!(values[1], ("motd".into(), "\u{a7}aHi: there".into()));
        assert_eq!(
            property_kind("level-type", &values[0].1).0,
            FieldKind::Choice
        );

        let (written, changed) = apply_properties(
            raw,
            &[
                edit(&["level-type"], serde_json::json!("minecraft:normal")),
                edit(&["motd"], serde_json::json!("\u{a7}bNew = 1 \u{1f600}")),
            ],
        )
        .unwrap();
        assert_eq!(changed, 1);
        assert_eq!(
            written,
            "level-type=minecraft\\:normal\nmotd=\\u00A7bNew \\= 1 \\uD83D\\uDE00\n"
        );
        assert_eq!(
            property(&written, "motd").as_deref(),
            Some("\u{a7}bNew = 1 \u{1f600}")
        );
        assert_eq!(escape(" a\\b"), "\\ a\\\\b");
    }

    #[test]
    fn property_values_are_checked() {
        let raw = "difficulty=easy\nmax-players=20\nmotd=hi\n";

        assert!(
            apply_properties(raw, &[edit(&["difficulty"], serde_json::json!("brutal"))]).is_err()
        );
        assert!(
            apply_properties(raw, &[edit(&["max-players"], serde_json::json!("many"))]).is_err()
        );
        assert!(
            apply_properties(raw, &[edit(&["motd"], serde_json::json!("two\nlines"))]).is_err()
        );
    }

    #[test]
    fn set_properties_adds_missing_keys() {
        let raw = "pvp=true\n\n";

        assert_eq!(
            set_properties(
                raw,
                &[("white-list", "true".into()), ("pvp", "false".into())]
            ),
            "pvp=false\nwhite-list=true\n"
        );
        assert_eq!(
            property("a=1\nwhite-list= true\n", "white-list").as_deref(),
            Some("true")
        );
    }

    const BUKKIT: &str = "# Bukkit settings\nsettings:\n  allow-end: true # the End\n  warn-on-overload: true\n  permissions-file: permissions.yml\n  connection-throttle: 4000\n  shutdown-message: 'Server closed'\nspawn-limits:\n  monsters: 70\nworlds:\n- world\n- world_nether\nchunk-gc:\n  period-in-ticks: 600\nticks-per:\n  autosave: 6000\naliases: now-in-commands.yml\n";

    #[test]
    fn yaml_leaves_follow_the_nesting() {
        let paths: Vec<String> = yaml_leaves(BUKKIT)
            .into_iter()
            .map(|leaf| leaf.path.join("."))
            .collect();

        assert_eq!(
            paths,
            vec![
                "settings.allow-end",
                "settings.warn-on-overload",
                "settings.permissions-file",
                "settings.connection-throttle",
                "settings.shutdown-message",
                "spawn-limits.monsters",
                "chunk-gc.period-in-ticks",
                "ticks-per.autosave",
                "aliases",
            ]
        );
    }

    #[test]
    fn yaml_edits_keep_comments_and_types() {
        let (written, changed) = apply_yaml(
            BUKKIT,
            &[
                edit(&["settings", "allow-end"], serde_json::json!(false)),
                edit(
                    &["settings", "shutdown-message"],
                    serde_json::json!("Bye, it's late"),
                ),
                edit(&["spawn-limits", "monsters"], serde_json::json!(50)),
                edit(&["settings", "permissions-file"], serde_json::json!("true")),
                edit(&["worlds"], serde_json::json!("x")),
            ],
        )
        .unwrap();

        assert_eq!(changed, 4);
        assert!(written.contains("  allow-end: false # the End\n"));
        assert!(written.contains("  shutdown-message: 'Bye, it''s late'\n"));
        assert!(written.contains("  monsters: 50\n"));
        // a string that would read as a boolean is quoted, so it stays a string
        assert!(written.contains("  permissions-file: 'true'\n"));
        assert!(written.contains("worlds:\n- world\n- world_nether\n"));
        assert!(written.starts_with("# Bukkit settings\n"));
    }

    #[test]
    fn yaml_lists_blocks_and_multiline_values_are_left_out() {
        let raw = "a:\n  list:\n  - x: 1\n    y: 2\n  text: |\n    line one\n    key: no\n  long: first\n    second\n  flow: [1, 2]\n  ok: 3\nsecret: \"abc\\\"d\"\nratio: 0.5\nempty: ~\n";
        let leaves = yaml_leaves(raw);
        let paths: Vec<String> = leaves.iter().map(|leaf| leaf.path.join(".")).collect();

        assert_eq!(paths, vec!["a.ok", "secret", "ratio", "empty"]);
        assert_eq!(
            leaves[1].scalar,
            Scalar::Text("abc\"d".into(), Style::Double)
        );
        assert_eq!(leaves[2].scalar, Scalar::Float(0.5));
        assert_eq!(leaves[3].scalar, Scalar::Null);

        let (written, _) = apply_yaml(
            raw,
            &[
                edit(&["ratio"], serde_json::json!(1)),
                edit(&["secret"], serde_json::json!("new\"one")),
            ],
        )
        .unwrap();
        assert!(written.contains("ratio: 1.0\n"));
        assert!(written.contains("secret: \"new\\\"one\"\n"));
    }

    #[test]
    fn yaml_types_are_checked() {
        assert!(
            apply_yaml(
                BUKKIT,
                &[edit(&["spawn-limits", "monsters"], serde_json::json!(1.5))]
            )
            .is_err()
        );
        assert!(
            apply_yaml(
                BUKKIT,
                &[edit(&["settings", "allow-end"], serde_json::json!("yes"))]
            )
            .is_err()
        );
    }

    #[test]
    fn quoted_keys_and_colons_in_keys() {
        let raw = "'0': zero\nminecraft:zombie: 4\n\"quoted key\": 'v'\n";
        let paths: Vec<String> = yaml_leaves(raw)
            .into_iter()
            .map(|leaf| leaf.path.join("|"))
            .collect();

        assert_eq!(paths, vec!["0", "minecraft:zombie", "quoted key"]);
    }
}
