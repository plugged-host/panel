//! plugged.host: who may join a Minecraft server and who runs it (docs/features/minecraft.md):
//! `whitelist.json`, `ops.json`, `banned-players.json` and `white-list` in `server.properties`.
//!
//! A running server holds these lists in memory and writes the files itself, so a change goes in
//! as a console command (`whitelist add`, `op`, `ban` …) while it runs, and into the file while it
//! is stopped. The answer says which, since a file edit only counts from the next start.

use super::{config, files};
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const WHITELIST: &str = "whitelist.json";
const OPS: &str = "ops.json";
const BANS: &str = "banned-players.json";
const PROPERTIES: &str = "server.properties";
const MAX_SIZE: u64 = 4 * 1024 * 1024;

const PROFILE_TTL: u64 = 24 * 60 * 60;
const MISS_TTL: u64 = 5 * 60;

/// A player of the server, merged from the three lists.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Player {
    pub name: compact_str::CompactString,
    pub uuid: compact_str::CompactString,
    pub whitelisted: bool,
    /// The operator level, when the player is one.
    pub operator: Option<u8>,
    pub ban: Option<Ban>,
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Ban {
    pub reason: compact_str::CompactString,
    pub created: compact_str::CompactString,
    pub expires: compact_str::CompactString,
    pub source: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Roster {
    pub players: Vec<Player>,
    /// `white-list` in `server.properties`: only listed players join.
    pub enforced: bool,
    /// `online-mode`: accounts are checked with Mojang.
    pub online_mode: bool,
    /// `op-permission-level`, what a new operator gets.
    pub level: u8,
    pub running: bool,
}

/// What a change does.
#[derive(Debug, Clone)]
pub enum Change {
    Whitelist(bool),
    Operator(bool),
    Ban(Option<String>),
    Pardon,
}

impl Change {
    pub fn event(&self) -> &'static str {
        match self {
            Self::Whitelist(true) => "server:minecraft.players.whitelist",
            Self::Whitelist(false) => "server:minecraft.players.unwhitelist",
            Self::Operator(true) => "server:minecraft.players.op",
            Self::Operator(false) => "server:minecraft.players.deop",
            Self::Ban(_) => "server:minecraft.players.ban",
            Self::Pardon => "server:minecraft.players.pardon",
        }
    }
}

#[derive(Debug)]
pub enum PlayerError {
    InvalidName,
    InvalidReason,
    UnknownPlayer(String),
    Lookup(anyhow::Error),
    Wings(anyhow::Error),
}

impl PlayerError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let (message, status) = match self {
            Self::InvalidName => (
                "a minecraft name has 1 to 16 letters, digits and underscores".to_string(),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidReason => (
                "the reason must be one line of at most 200 characters".to_string(),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnknownPlayer(name) => (
                format!("there is no minecraft account called {name}"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Lookup(err) => {
                tracing::warn!("minecraft profile lookup: {err:#}");
                (
                    "mojang did not answer, try again in a moment".to_string(),
                    StatusCode::BAD_GATEWAY,
                )
            }
            Self::Wings(err) => {
                tracing::warn!("minecraft players: {err:#}");
                (
                    "the server's files could not be reached, try again in a moment".to_string(),
                    StatusCode::BAD_GATEWAY,
                )
            }
        };

        crate::response::ApiResponse::error(message).with_status(status)
    }
}

/// A Minecraft account name: 1 to 16 letters, digits and underscores.
pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
    pub uuid: compact_str::CompactString,
    pub name: compact_str::CompactString,
}

/// The account's UUID and spelling from Mojang, `None` when there is no such account.
pub async fn profile(state: &crate::State, name: &str) -> Result<Option<Profile>, anyhow::Error> {
    let key = format!("minecraft::profile::{}", name.to_ascii_lowercase());
    if let Ok(Some(cached)) = state.cache.get::<Option<Profile>>(&key).await {
        return Ok(cached);
    }

    let response = state
        .client
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{name}"
        ))
        .timeout(super::TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .send()
        .await?;

    // 404 is "no such account"; older answers were 204 without a body
    if matches!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NO_CONTENT
    ) {
        state.cache.set(&key, MISS_TTL, &None::<Profile>).await.ok();
        return Ok(None);
    }

    let body: serde_json::Value = response.error_for_status()?.json().await?;
    let id = body["id"].as_str().unwrap_or_default();
    let Some(uuid) = dashed(id) else {
        anyhow::bail!("mojang answered without an id");
    };

    let profile = Profile {
        uuid,
        name: body["name"].as_str().unwrap_or(name).into(),
    };
    state
        .cache
        .set(&key, PROFILE_TTL, &Some(profile.clone()))
        .await
        .ok();

    Ok(Some(profile))
}

/// The UUID an offline-mode server gives a name: Java's `nameUUIDFromBytes("OfflinePlayer:<name>")`.
pub fn offline_uuid(name: &str) -> compact_str::CompactString {
    let mut bytes = md5::compute(format!("OfflinePlayer:{name}")).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    uuid::Uuid::from_bytes(bytes)
        .hyphenated()
        .to_string()
        .into()
}

fn dashed(id: &str) -> Option<compact_str::CompactString> {
    uuid::Uuid::try_parse(id)
        .ok()
        .map(|uuid| uuid.hyphenated().to_string().into())
}

/// The entries of one of the JSON lists; a missing or broken file is an empty list.
fn entries(raw: Option<&str>) -> Vec<serde_json::Map<String, serde_json::Value>> {
    raw.and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| match entry {
            serde_json::Value::Object(map)
                if map
                    .get("name")
                    .and_then(|name| name.as_str())
                    .is_some_and(|name| !name.trim().is_empty()) =>
            {
                Some(map)
            }
            _ => None,
        })
        .collect()
}

fn text(
    entry: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> compact_str::CompactString {
    entry
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .into()
}

fn same_name(entry: &serde_json::Map<String, serde_json::Value>, name: &str) -> bool {
    text(entry, "name").eq_ignore_ascii_case(name)
}

/// The roster's row for a list entry, added when the name is new.
fn player<'a>(
    players: &'a mut Vec<Player>,
    entry: &serde_json::Map<String, serde_json::Value>,
) -> &'a mut Player {
    let name = text(entry, "name");
    let index = match players
        .iter()
        .position(|player| player.name.eq_ignore_ascii_case(&name))
    {
        Some(index) => index,
        None => {
            players.push(Player {
                name,
                uuid: text(entry, "uuid"),
                whitelisted: false,
                operator: None,
                ban: None,
            });
            players.len() - 1
        }
    };

    &mut players[index]
}

struct Files {
    properties: Option<String>,
    whitelist: Vec<serde_json::Map<String, serde_json::Value>>,
    ops: Vec<serde_json::Map<String, serde_json::Value>>,
    bans: Vec<serde_json::Map<String, serde_json::Value>>,
}

impl Files {
    async fn read(
        client: &wings_api::client::WingsClient,
        server: &Server,
    ) -> Result<Self, anyhow::Error> {
        let properties = files::read_text(client, server, PROPERTIES, MAX_SIZE).await?;
        let whitelist = files::read_text(client, server, WHITELIST, MAX_SIZE).await?;
        let ops = files::read_text(client, server, OPS, MAX_SIZE).await?;
        let bans = files::read_text(client, server, BANS, MAX_SIZE).await?;

        Ok(Self {
            properties,
            whitelist: entries(whitelist.as_deref()),
            ops: entries(ops.as_deref()),
            bans: entries(bans.as_deref()),
        })
    }

    fn flag(&self, key: &str, default: bool) -> bool {
        self.properties
            .as_deref()
            .and_then(|raw| config::property(raw, key))
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(default)
    }

    fn level(&self) -> u8 {
        self.properties
            .as_deref()
            .and_then(|raw| config::property(raw, "op-permission-level"))
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(4)
            .clamp(1, 4)
    }

    fn roster(&self, running: bool) -> Roster {
        let level = self.level();
        let mut players: Vec<Player> = Vec::new();

        for entry in &self.whitelist {
            player(&mut players, entry).whitelisted = true;
        }
        for entry in &self.ops {
            player(&mut players, entry).operator = Some(
                entry
                    .get("level")
                    .and_then(|value| value.as_u64())
                    .map(|value| value.min(4) as u8)
                    .unwrap_or(level),
            );
        }
        for entry in &self.bans {
            player(&mut players, entry).ban = Some(Ban {
                reason: text(entry, "reason"),
                created: text(entry, "created"),
                expires: text(entry, "expires"),
                source: text(entry, "source"),
            });
        }

        players.sort_by_key(|player| player.name.to_ascii_lowercase());

        Roster {
            players,
            enforced: self.flag("white-list", false),
            online_mode: self.flag("online-mode", true),
            level,
            running,
        }
    }
}

/// The server's players.
pub async fn roster(state: &crate::State, server: &Server) -> Result<Roster, anyhow::Error> {
    let client = files::client(state, server).await?;
    let running = matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Running)
    );

    Ok(Files::read(&client, server).await?.roster(running))
}

/// Whether the change went in live (console) or into the files (from the next start).
#[derive(ToSchema, Serialize, Debug)]
pub struct Outcome {
    pub live: bool,
}

/// Changes one player. `console` says the caller may use the console; without it a running
/// server's files are still written, and the server may overwrite them.
pub async fn change(
    state: &crate::State,
    server: &Server,
    name: &str,
    change: Change,
    console: bool,
) -> Result<Outcome, PlayerError> {
    let name = name.trim();
    if !valid_name(name) {
        return Err(PlayerError::InvalidName);
    }
    if let Change::Ban(Some(reason)) = &change
        && (reason.chars().count() > 200 || reason.chars().any(char::is_control))
    {
        return Err(PlayerError::InvalidReason);
    }

    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    let files_now = Files::read(&client, server)
        .await
        .map_err(PlayerError::Wings)?;
    let level = files_now.level();

    let (list, file) = match change {
        Change::Whitelist(_) => (&files_now.whitelist, WHITELIST),
        Change::Operator(_) => (&files_now.ops, OPS),
        Change::Ban(_) | Change::Pardon => (&files_now.bans, BANS),
    };
    let existing = list.iter().find(|entry| same_name(entry, name)).cloned();
    let adding = matches!(
        change,
        Change::Whitelist(true) | Change::Operator(true) | Change::Ban(_)
    );

    // already so: nothing to do
    if adding == existing.is_some() {
        return Ok(Outcome { live: true });
    }

    let (name, uuid) = match &existing {
        Some(entry) => (text(entry, "name"), text(entry, "uuid")),
        None if files_now.flag("online-mode", true) => {
            match profile(state, name).await.map_err(PlayerError::Lookup)? {
                Some(profile) => (profile.name, profile.uuid),
                None => return Err(PlayerError::UnknownPlayer(name.to_string())),
            }
        }
        None => (name.into(), offline_uuid(name)),
    };

    if console
        && matches!(
            files::power_state(&client, server).await,
            Some(wings_api::ServerState::Running)
        )
    {
        let command = match &change {
            Change::Whitelist(true) => format!("whitelist add {name}"),
            Change::Whitelist(false) => format!("whitelist remove {name}"),
            Change::Operator(true) => format!("op {name}"),
            Change::Operator(false) => format!("deop {name}"),
            Change::Ban(Some(reason)) if !reason.trim().is_empty() => {
                format!("ban {name} {}", reason.trim())
            }
            Change::Ban(_) => format!("ban {name}"),
            Change::Pardon => format!("pardon {name}"),
        };

        if files::command(&client, server, &command).await.is_ok() {
            // the server writes its file on its next tick; the page reads it right after
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            return Ok(Outcome { live: true });
        }
    }

    let mut list = list.clone();
    if adding {
        let mut entry = serde_json::Map::new();
        entry.insert("uuid".into(), uuid.as_str().into());
        entry.insert("name".into(), name.as_str().into());
        match &change {
            Change::Operator(_) => {
                entry.insert("level".into(), level.into());
                entry.insert("bypassesPlayerLimit".into(), false.into());
            }
            Change::Ban(reason) => {
                entry.insert(
                    "created".into(),
                    chrono::Utc::now()
                        .format("%Y-%m-%d %H:%M:%S %z")
                        .to_string()
                        .into(),
                );
                entry.insert("source".into(), "Server".into());
                entry.insert("expires".into(), "forever".into());
                entry.insert(
                    "reason".into(),
                    reason
                        .as_deref()
                        .map(str::trim)
                        .filter(|reason| !reason.is_empty())
                        .unwrap_or("Banned by an operator.")
                        .into(),
                );
            }
            _ => {}
        }
        list.push(entry);
    } else {
        list.retain(|entry| !same_name(entry, &name));
    }

    // pretty-printed with a trailing newline, as the server writes it
    let mut written =
        serde_json::to_string_pretty(&list).map_err(|err| PlayerError::Wings(err.into()))?;
    written.push('\n');
    files::write_text(&client, server, file, written)
        .await
        .map_err(PlayerError::Wings)?;

    Ok(Outcome { live: false })
}

/// Turns the whitelist on or off: `whitelist on|off` while running, and always `white-list` and
/// `enforce-whitelist` in `server.properties`, so the next start keeps it.
pub async fn enforce(
    state: &crate::State,
    server: &Server,
    enabled: bool,
    console: bool,
) -> Result<Outcome, PlayerError> {
    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;

    let live = console
        && matches!(
            files::power_state(&client, server).await,
            Some(wings_api::ServerState::Running)
        )
        && files::command(
            &client,
            server,
            if enabled {
                "whitelist on"
            } else {
                "whitelist off"
            },
        )
        .await
        .is_ok();

    let raw = files::read_text(&client, server, PROPERTIES, MAX_SIZE)
        .await
        .map_err(PlayerError::Wings)?
        .unwrap_or_default();
    let written = config::set_properties(
        &raw,
        &[
            ("white-list", enabled.to_string()),
            ("enforce-whitelist", enabled.to_string()),
        ],
    );
    files::write_text(&client, server, PROPERTIES, written)
        .await
        .map_err(PlayerError::Wings)?;

    Ok(Outcome { live })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuids_match_minecraft() {
        // what an offline-mode server writes for "Notch"
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn names_are_checked() {
        assert!(valid_name("Steve_01"));
        assert!(!valid_name(""));
        assert!(!valid_name("seventeen_letters"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("x\nop me"));
    }

    #[test]
    fn the_lists_merge_into_one_roster() {
        let files = Files {
            properties: Some("white-list=true\nonline-mode=false\nop-permission-level=3\n".into()),
            whitelist: entries(Some(
                r#"[{"uuid":"a","name":"Alex"},{"uuid":"s","name":"steve"}]"#,
            )),
            ops: entries(Some(
                r#"[{"uuid":"a","name":"alex","level":4,"bypassesPlayerLimit":false},{"name":"Zed"}]"#,
            )),
            bans: entries(Some(
                r#"[{"uuid":"g","name":"Griefer","created":"2026-10-01 10:00:00 +0000","source":"Server","expires":"forever","reason":"Griefing"},{"broken":true}]"#,
            )),
        };

        let roster = files.roster(false);
        let names: Vec<&str> = roster
            .players
            .iter()
            .map(|player| player.name.as_str())
            .collect();

        assert_eq!(names, vec!["Alex", "Griefer", "steve", "Zed"]);
        assert!(roster.enforced);
        assert!(!roster.online_mode);
        assert_eq!(roster.level, 3);
        assert_eq!(roster.players[0].operator, Some(4));
        assert!(roster.players[0].whitelisted);
        assert_eq!(roster.players[1].ban.as_ref().unwrap().reason, "Griefing");
        assert_eq!(roster.players[3].operator, Some(3));
        assert!(!roster.players[3].whitelisted);
    }
}
