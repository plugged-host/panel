//! plugged.host: a server.cfg read as settings and written back without disturbing anything else
//! (docs/features/fivem.md).
//!
//! FXServer runs the file top to bottom as console commands: `sv_hostname "x"`,
//! `sets sv_projectName "x"`, `ensure chat`, `add_ace group.admin command allow`. A form over it
//! finds "the" hostname among those lines, changes one token of one line, and leaves comments,
//! blank lines and unknown commands exactly as they were. Nothing is re-rendered: every change is
//! an operation on a line of the file as parsed (replace, delete, insert before or after), applied
//! in one pass by [`ServerCfg::render`]. Line numbers therefore always mean the original file,
//! which is what lets the page give list rows a key (`"12"` = line 12) and match them on save.
//!
//! The tokenizer follows FXServer's cfg grammar: whitespace separates tokens, double quotes group
//! one, and a token starting with `#` or `//` starts a comment. A `#` or `//` inside a token is
//! part of it, which keeps an unquoted URL a URL.

use std::collections::{BTreeMap, BTreeSet};

const SET: &[&str] = &["set", "sets", "setr", "seta"];
/// Commands a new convar line goes in front of, so it is set before resources start.
const LOADS: &[&str] = &["ensure", "start", "restart", "exec"];

/// A resource name.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | '[' | ']'))
}

/// A principal or ACE object (`group.admin`, `identifier.fivem:123`, `command.quit`).
pub fn valid_principal(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@' | '[' | ']'))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub quoted: bool,
}

/// A line's tokens with their byte offsets, and where its comment starts.
pub fn tokenize(line: &str) -> (Vec<Token>, Option<usize>) {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        if c == b'#' || (c == b'/' && bytes.get(i + 1) == Some(&b'/')) {
            return (tokens, Some(i));
        }
        if c == b'"' {
            let close = line[i + 1..].find('"').map(|offset| i + 1 + offset);
            let end = close.map_or(bytes.len(), |close| close + 1);
            tokens.push(Token {
                text: line[i + 1..close.unwrap_or(bytes.len())].to_string(),
                start: i,
                end,
                quoted: true,
            });
            i = end;
            continue;
        }

        let start = i;
        while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'"') {
            i += 1;
        }
        tokens.push(Token {
            text: line[start..i].to_string(),
            start,
            end: i,
            quoted: false,
        });
    }

    (tokens, None)
}

fn words(line: &str) -> Vec<String> {
    tokenize(line)
        .0
        .into_iter()
        .map(|token| token.text)
        .collect()
}

fn indent(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// One line, no double quotes: the grammar has no escapes, so a quote would end the token and a
/// line break would start a new command.
pub fn clean(value: &str) -> String {
    value
        .replace('"', "'")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

fn format(value: &str, quote: bool) -> String {
    if quote
        || value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || c == '#' || c == ';')
        || value.starts_with("//")
    {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

/// How a new setting line is written: `name value`, or `set|sets|setr name value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Command,
    Set,
    Sets,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Resource {
    /// The line it came from; `null` for a new row.
    pub key: Option<String>,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Principal {
    pub key: Option<String>,
    pub child: String,
    pub parent: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Ace {
    pub key: Option<String>,
    pub principal: String,
    pub object: String,
    pub access: Access,
}

struct ResourceLine {
    name: String,
    enabled: bool,
    command: String,
    indent: String,
    comment: String,
}

fn resource_line(line: &str) -> Option<ResourceLine> {
    let indent = indent(line).to_string();
    let mut body = line.trim_start();
    let mut enabled = true;

    let hashes = body.len() - body.trim_start_matches('#').len();
    if hashes > 0 {
        enabled = false;
        body = body[hashes..].trim_start();
    } else if let Some(rest) = body.strip_prefix("//") {
        enabled = false;
        body = rest.trim_start();
    }

    let (tokens, comment) = tokenize(body);
    if tokens.len() != 2 {
        return None;
    }
    let command = tokens[0].text.to_lowercase();
    if !matches!(command.as_str(), "ensure" | "start") || !valid_name(&tokens[1].text) {
        return None;
    }

    Some(ResourceLine {
        name: tokens[1].text.clone(),
        enabled,
        command,
        indent,
        comment: comment.map_or(String::new(), |at| format!(" {}", body[at..].trim_start())),
    })
}

/// The position of a setting's value among a line's words.
fn setting_position(words: &[String], name: &str) -> Option<usize> {
    let command = words.first()?.to_lowercase();
    if command == name.to_lowercase() {
        return Some(1);
    }
    if SET.contains(&command.as_str()) && words.get(1).is_some_and(|w| w.eq_ignore_ascii_case(name))
    {
        return Some(2);
    }

    None
}

pub struct ServerCfg {
    lines: Vec<String>,
    eol: &'static str,
    final_newline: bool,
    replace: BTreeMap<usize, String>,
    delete: BTreeSet<usize>,
    before: BTreeMap<usize, Vec<String>>,
    after: BTreeMap<usize, Vec<String>>,
    append: Vec<String>,
}

impl ServerCfg {
    pub fn new(raw: &str) -> Self {
        let eol = if raw.contains("\r\n") { "\r\n" } else { "\n" };
        let final_newline = raw.is_empty() || raw.ends_with('\n');

        let mut lines: Vec<String> = raw
            .split("\r\n")
            .flat_map(|part| part.split(['\n', '\r']))
            .map(str::to_string)
            .collect();
        if final_newline && lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }

        Self {
            lines,
            eol,
            final_newline,
            replace: BTreeMap::new(),
            delete: BTreeSet::new(),
            before: BTreeMap::new(),
            after: BTreeMap::new(),
            append: Vec::new(),
        }
    }

    pub fn render(&self) -> String {
        let mut out: Vec<&str> = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            if let Some(lines) = self.before.get(&index) {
                out.extend(lines.iter().map(String::as_str));
            }
            if !self.delete.contains(&index) {
                out.push(self.replace.get(&index).unwrap_or(line));
            }
            if let Some(lines) = self.after.get(&index) {
                out.extend(lines.iter().map(String::as_str));
            }
        }
        out.extend(self.append.iter().map(String::as_str));

        if out.is_empty() {
            return String::new();
        }
        let mut rendered = out.join(self.eol);
        if self.final_newline {
            rendered.push_str(self.eol);
        }

        rendered
    }

    fn find_setting(&self, name: &str) -> Option<(usize, usize)> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                setting_position(&words(line), name).map(|position| (index, position))
            })
            .last()
    }

    /// The value the file leaves a setting at: the last line that sets it wins, as when FXServer
    /// runs the file.
    pub fn get(&self, name: &str) -> Option<String> {
        let (index, position) = self.find_setting(name)?;

        Some(
            tokenize(&self.lines[index])
                .0
                .get(position)
                .map(|token| token.text.clone())
                .unwrap_or_default(),
        )
    }

    /// Changes a setting's value in place, adds it when the file lacks it, or drops it when the
    /// value is empty.
    pub fn set(&mut self, name: &str, value: &str, style: Style, quote: bool) {
        let value = clean(value);

        if value.is_empty() {
            for (index, line) in self.lines.iter().enumerate() {
                if setting_position(&words(line), name).is_some() {
                    self.delete.insert(index);
                }
            }
            return;
        }

        let Some((index, position)) = self.find_setting(name) else {
            let prefix = match style {
                Style::Command => "",
                Style::Set => "set ",
                Style::Sets => "sets ",
            };
            let line = format!("{prefix}{name} {}", format(&value, quote));
            match self.first_load() {
                Some(anchor) => self.before.entry(anchor).or_default().push(line),
                None => self.append.push(line),
            }
            return;
        };

        let line = &self.lines[index];
        let tokens = tokenize(line).0;
        let Some(token) = tokens.get(position) else {
            self.replace.insert(
                index,
                format!("{} {}", line.trim_end(), format(&value, quote)),
            );
            return;
        };
        if token.text == value {
            return;
        }

        self.replace.insert(
            index,
            format!(
                "{}{}{}",
                &line[..token.start],
                format(&value, token.quoted || quote),
                &line[token.end..]
            ),
        );
    }

    fn first_load(&self) -> Option<usize> {
        self.lines.iter().position(|line| {
            words(line)
                .first()
                .is_some_and(|word| LOADS.contains(&word.to_lowercase().as_str()))
        })
    }

    /// The `endpoint_add_tcp`/`endpoint_add_udp` addresses.
    pub fn endpoints(&self) -> Vec<String> {
        let mut endpoints: Vec<String> = Vec::new();
        for line in &self.lines {
            let words = words(line);
            if words.len() >= 2
                && matches!(
                    words[0].to_lowercase().as_str(),
                    "endpoint_add_tcp" | "endpoint_add_udp"
                )
                && !endpoints.contains(&words[1])
            {
                endpoints.push(words[1].clone());
            }
        }

        endpoints
    }

    /// Every `ensure`/`start` line, commented-out ones as disabled, in file order. A commented
    /// line counts only when it is exactly one such command, so prose stays prose.
    pub fn resources(&self) -> Vec<Resource> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                resource_line(line).map(|resource| Resource {
                    key: Some(index.to_string()),
                    name: resource.name,
                    enabled: resource.enabled,
                })
            })
            .collect()
    }

    /// Applies the resource list as the form left it: a row with a known key changes its line,
    /// a row without one is new, a line no row keeps is removed.
    pub fn set_resources(&mut self, rows: &[Resource]) {
        let existing: BTreeMap<usize, ResourceLine> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| resource_line(line).map(|resource| (index, resource)))
            .collect();

        let mut kept = BTreeSet::new();
        let mut new = Vec::new();
        for row in rows {
            let name = row.name.trim();
            if !valid_name(name) {
                continue;
            }

            if let Some(index) = row.key.as_deref().and_then(|key| key.parse::<usize>().ok())
                && let Some(old) = existing.get(&index)
            {
                kept.insert(index);
                if old.name != name || old.enabled != row.enabled {
                    self.replace.insert(
                        index,
                        format!(
                            "{}{}{} {name}{}",
                            old.indent,
                            if row.enabled { "" } else { "# " },
                            old.command,
                            old.comment
                        ),
                    );
                }
                continue;
            }

            new.push(format!(
                "{}ensure {name}",
                if row.enabled { "" } else { "# " }
            ));
        }

        for index in existing.keys() {
            if !kept.contains(index) {
                self.delete.insert(*index);
            }
        }
        self.insert_after_last(existing.keys().copied().collect(), new);
    }

    pub fn principals(&self) -> Vec<Principal> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let words = words(line);
                (words.len() >= 3 && words[0].eq_ignore_ascii_case("add_principal")).then(|| {
                    Principal {
                        key: Some(index.to_string()),
                        child: words[1].clone(),
                        parent: words[2].clone(),
                    }
                })
            })
            .collect()
    }

    pub fn set_principals(&mut self, rows: &[Principal]) {
        let rows: Vec<(Option<String>, Vec<String>)> = rows
            .iter()
            .filter_map(|row| {
                let (child, parent) = (row.child.trim(), row.parent.trim());
                (valid_principal(child) && valid_principal(parent))
                    .then(|| (row.key.clone(), vec![child.to_string(), parent.to_string()]))
            })
            .collect();
        self.set_list("add_principal", 3, rows);
    }

    pub fn aces(&self) -> Vec<Ace> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let words = words(line);
                (words.len() >= 4 && words[0].eq_ignore_ascii_case("add_ace")).then(|| Ace {
                    key: Some(index.to_string()),
                    principal: words[1].clone(),
                    object: words[2].clone(),
                    access: if words[3].eq_ignore_ascii_case("deny") {
                        Access::Deny
                    } else {
                        Access::Allow
                    },
                })
            })
            .collect()
    }

    pub fn set_aces(&mut self, rows: &[Ace]) {
        let rows: Vec<(Option<String>, Vec<String>)> = rows
            .iter()
            .filter_map(|row| {
                let (principal, object) = (row.principal.trim(), row.object.trim());
                (valid_principal(principal) && valid_principal(object)).then(|| {
                    (
                        row.key.clone(),
                        vec![
                            principal.to_string(),
                            object.to_string(),
                            match row.access {
                                Access::Allow => "allow",
                                Access::Deny => "deny",
                            }
                            .to_string(),
                        ],
                    )
                })
            })
            .collect();
        self.set_list("add_ace", 4, rows);
    }

    /// The shape `add_principal` and `add_ace` share: rows keyed by line, each line one command
    /// with fixed arguments and maybe a trailing comment, which a kept or edited row keeps.
    fn set_list(&mut self, command: &str, count: usize, rows: Vec<(Option<String>, Vec<String>)>) {
        struct Existing {
            arguments: Vec<String>,
            indent: String,
            comment: String,
        }

        let existing: BTreeMap<usize, Existing> = self
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let (tokens, comment) = tokenize(line);
                (tokens.len() >= count && tokens[0].text.eq_ignore_ascii_case(command)).then(|| {
                    (
                        index,
                        Existing {
                            arguments: tokens[1..count].iter().map(|t| t.text.clone()).collect(),
                            indent: indent(line).to_string(),
                            comment: comment
                                .map_or(String::new(), |at| format!(" {}", &line[at..])),
                        },
                    )
                })
            })
            .collect();

        let mut kept = BTreeSet::new();
        let mut new = Vec::new();
        for (key, arguments) in rows {
            if let Some(index) = key.as_deref().and_then(|key| key.parse::<usize>().ok())
                && let Some(old) = existing.get(&index)
            {
                kept.insert(index);
                if old.arguments != arguments {
                    self.replace.insert(
                        index,
                        format!(
                            "{}{command} {}{}",
                            old.indent,
                            arguments.join(" "),
                            old.comment
                        ),
                    );
                }
                continue;
            }

            new.push(format!("{command} {}", arguments.join(" ")));
        }

        for index in existing.keys() {
            if !kept.contains(index) {
                self.delete.insert(*index);
            }
        }
        self.insert_after_last(existing.keys().copied().collect(), new);
    }

    fn insert_after_last(&mut self, indexes: Vec<usize>, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        match indexes.into_iter().max() {
            Some(last) => self.after.entry(last).or_default().extend(lines),
            None => self.append.extend(lines),
        }
    }

    /// Whether the file makes this a RedM server (txAdmin's RedM recipes set `gamename rdr3`).
    pub fn is_redm(&self) -> bool {
        self.get("gamename")
            .is_some_and(|game| game.eq_ignore_ascii_case("rdr3"))
    }
}

/// `mysql_connection_string` as fields, in either form oxmysql reads (a URI it does not
/// URL-decode, or `key=value;` with its aliases per key).
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
pub struct Connection {
    pub host: String,
    pub port: String,
    pub user: String,
    pub name: String,
    pub password: String,
    /// `key=value` instead of a `mysql://` URI.
    pub key_value: bool,
}

impl Connection {
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        let mut connection = Self::default();
        if value.is_empty() {
            return connection;
        }

        if let Some(at) = value.find("mysql://") {
            let rest = &value[at + "mysql://".len()..];
            let rest = &rest[..rest.find(['?', '#']).unwrap_or(rest.len())];
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let (auth, host) = match authority.rsplit_once('@') {
                Some((auth, host)) => (Some(auth), host),
                None => (None, authority),
            };
            if let Some(auth) = auth {
                let (user, password) = auth.split_once(':').unwrap_or((auth, ""));
                connection.user = user.to_string();
                connection.password = password.to_string();
            }
            let (host, port) = match host.rsplit_once(':') {
                Some((host, port))
                    if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) =>
                {
                    (host, port)
                }
                _ => (host, ""),
            };
            connection.host = host.to_string();
            connection.port = port.to_string();
            connection.name = path.to_string();

            return connection;
        }

        connection.key_value = true;
        for pair in value.split(';') {
            let (key, item) = pair.split_once('=').unwrap_or((pair, ""));
            let item = item.trim().to_string();
            match key.trim().to_lowercase().as_str() {
                "host" | "hostname" | "ip" | "server" | "datasource" | "data source" | "addr"
                | "address" => connection.host = item,
                "user" | "userid" | "user id" | "username" | "user name" | "uid" => {
                    connection.user = item
                }
                "password" | "pwd" | "pass" => connection.password = item,
                "database" | "db" => connection.name = item,
                "port" => connection.port = item,
                _ => {}
            }
        }

        connection
    }

    /// The value written back; empty without a host.
    pub fn compose(&self) -> String {
        let host = self.host.trim();
        if host.is_empty() {
            return String::new();
        }
        let (port, user, name) = (self.port.trim(), self.user.trim(), self.name.trim());

        if self.key_value {
            let mut parts = vec![format!("host={host}")];
            if !port.is_empty() {
                parts.push(format!("port={port}"));
            }
            if !user.is_empty() {
                parts.push(format!("user={user}"));
            }
            if !self.password.is_empty() {
                parts.push(format!("password={}", self.password));
            }
            if !name.is_empty() {
                parts.push(format!("database={name}"));
            }
            return parts.join(";");
        }

        let auth = match (user.is_empty(), self.password.is_empty()) {
            (true, _) => String::new(),
            (false, true) => format!("{user}@"),
            (false, false) => format!("{user}:{}@", self.password),
        };
        let port = if port.is_empty() {
            String::new()
        } else {
            format!(":{port}")
        };

        format!("mysql://{auth}{host}{port}/{name}?charset=utf8mb4")
    }
}

/// `sets tags "a, b"` as a list.
pub fn tags(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "# my server\nendpoint_add_tcp \"0.0.0.0:30120\"\nendpoint_add_udp \"0.0.0.0:30120\"\nsv_hostname \"Old ^1Name\" # shown in the list\nset mysql_connection_string \"mysql://user:pw@db.example.com:3306/esx?charset=utf8mb4\"\n\nensure chat\n# ensure spawnmanager\n# start the next ones later\nstart es_extended\nadd_principal identifier.fivem:1 group.admin # me\nadd_ace group.admin command allow\n";

    #[test]
    fn tokens_keep_urls_and_find_comments() {
        let line = "set url https://x.example.com/#a \"two words\" # note";
        let (tokens, comment) = tokenize(line);
        let texts: Vec<_> = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["set", "url", "https://x.example.com/#a", "two words"]
        );
        assert_eq!(comment, line.find("# note"));
        assert!(tokens[3].quoted);
    }

    #[test]
    fn unchanged_file_renders_byte_for_byte() {
        assert_eq!(ServerCfg::new(CFG).render(), CFG);
        let crlf = CFG.replace('\n', "\r\n");
        assert_eq!(ServerCfg::new(&crlf).render(), crlf);
        assert_eq!(ServerCfg::new("ensure a").render(), "ensure a");
    }

    #[test]
    fn settings_change_in_place_or_are_added_before_resources() {
        let mut cfg = ServerCfg::new(CFG);
        assert_eq!(cfg.get("sv_hostname").as_deref(), Some("Old ^1Name"));
        cfg.set("sv_hostname", "New \"Name\"", Style::Command, true);
        cfg.set("sv_maxclients", "48", Style::Command, false);
        cfg.set("sv_projectName", "Project", Style::Sets, true);
        let out = cfg.render();
        assert!(out.contains("sv_hostname \"New 'Name'\" # shown in the list\n"));
        assert!(out.contains("sv_maxclients 48\nsets sv_projectName \"Project\"\nensure chat\n"));

        let mut cfg = ServerCfg::new(CFG);
        cfg.set("mysql_connection_string", "", Style::Set, true);
        assert!(!cfg.render().contains("mysql_connection_string"));
    }

    #[test]
    fn resources_keep_their_lines_and_comments() {
        let mut cfg = ServerCfg::new(CFG);
        let mut rows = cfg.resources();
        assert_eq!(
            rows.iter()
                .map(|r| (r.name.as_str(), r.enabled))
                .collect::<Vec<_>>(),
            vec![
                ("chat", true),
                ("spawnmanager", false),
                ("es_extended", true)
            ]
        );
        rows[0].enabled = false;
        rows.remove(1);
        rows.push(Resource {
            key: None,
            name: "oxmysql".into(),
            enabled: true,
        });
        cfg.set_resources(&rows);
        let out = cfg.render();
        assert!(out.contains(
            "\n# ensure chat\n# start the next ones later\nstart es_extended\nensure oxmysql\n"
        ));
        assert!(!out.contains("spawnmanager"));
    }

    #[test]
    fn principals_and_aces_round_trip() {
        let mut cfg = ServerCfg::new(CFG);
        let mut principals = cfg.principals();
        principals[0].parent = "group.moderator".into();
        cfg.set_principals(&principals);
        let mut aces = cfg.aces();
        aces.push(Ace {
            key: None,
            principal: "group.moderator".into(),
            object: "command.kick".into(),
            access: Access::Allow,
        });
        aces.push(Ace {
            key: None,
            principal: "bad principal".into(),
            object: "x".into(),
            access: Access::Deny,
        });
        cfg.set_aces(&aces);
        let out = cfg.render();
        assert!(out.contains("add_principal identifier.fivem:1 group.moderator # me\n"));
        assert!(out.ends_with(
            "add_ace group.admin command allow\nadd_ace group.moderator command.kick allow\n"
        ));
    }

    #[test]
    fn connections_parse_both_forms() {
        let uri = Connection::parse("mysql://user:pw@db.example.com:3306/esx?charset=utf8mb4");
        assert_eq!(
            (
                uri.host.as_str(),
                uri.port.as_str(),
                uri.user.as_str(),
                uri.password.as_str(),
                uri.name.as_str(),
                uri.key_value
            ),
            ("db.example.com", "3306", "user", "pw", "esx", false)
        );
        assert_eq!(
            uri.compose(),
            "mysql://user:pw@db.example.com:3306/esx?charset=utf8mb4"
        );

        let kv = Connection::parse("server=db.example.com;uid=u;pwd=p;database=d");
        assert!(kv.key_value);
        assert_eq!(
            kv.compose(),
            "host=db.example.com;user=u;password=p;database=d"
        );
        assert_eq!(Connection::default().compose(), "");
        assert_eq!(tags(Some(" a, ,b ")), vec!["a", "b"]);
    }

    #[test]
    fn redm_and_endpoints() {
        let cfg = ServerCfg::new(
            "set gamename rdr3\nendpoint_add_tcp 0.0.0.0:1\nendpoint_add_udp 0.0.0.0:1\n",
        );
        assert!(cfg.is_redm());
        assert_eq!(cfg.endpoints(), vec!["0.0.0.0:1"]);
    }
}
