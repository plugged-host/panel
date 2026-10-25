//! plugged.host: who may join a Minecraft server and who runs it (docs/features/minecraft.md):
//! `whitelist.json`, `ops.json`, `banned-players.json` and `white-list` in `server.properties`.
//!
//! A running server holds these lists in memory and writes the files itself, so a change goes in
//! as a console command (`whitelist add`, `op`, `ban` …) while it runs, and into the file while it
//! is stopped. The answer says which, since a file edit only counts from the next start.
//!
//! The roster also holds everyone who has played (a file in `<world>/playerdata`, named from
//! `usercache.json`), and who is online comes from the console's `list uuids`
//! ([`online`]); one player's details are in `player_data.rs`.

use super::{config, files};
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const WHITELIST: &str = "whitelist.json";
const OPS: &str = "ops.json";
const BANS: &str = "banned-players.json";
const PROPERTIES: &str = "server.properties";
pub(super) const USERCACHE: &str = "usercache.json";
pub(super) const MAX_SIZE: u64 = 4 * 1024 * 1024;
/// At most this many player data files are listed (two pages of Wings' listing).
const MAX_JOINED: u64 = 1000;
const JOINED_PAGE: u64 = 500;
/// How long an answer of `list` is reused, so a page opened twice asks the console once.
const ONLINE_TTL: u64 = 10;
/// How long the console is watched for the answer of `list`.
const ONLINE_TRIES: usize = 6;
const ONLINE_WAIT: std::time::Duration = std::time::Duration::from_millis(300);

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
    /// When the server last saved the player's data (`<world>/playerdata/<uuid>.dat`), so when
    /// they last played; `None` for someone who never joined.
    pub last_played: Option<chrono::DateTime<chrono::Utc>>,
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
    /// A player this server has never seen (no list entry, no data file).
    NotFound,
    /// Nothing saved for the player yet: they never played, or the data file is gone.
    NoData,
    /// The player data file is not readable NBT.
    Damaged(anyhow::Error),
    /// A file edit while the server is not stopped.
    Running,
    /// A console command while the server is not running.
    NotRunning,
    NotOnline,
    Invalid(String),
    /// The permission the change needs in the server's current state.
    Forbidden(&'static str),
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
            Self::NotFound => (
                "this player has never been on this server".to_string(),
                StatusCode::NOT_FOUND,
            ),
            Self::NoData => (
                "there is no saved data for this player yet".to_string(),
                StatusCode::NOT_FOUND,
            ),
            Self::Damaged(err) => {
                tracing::warn!("minecraft player data: {err:#}");
                (
                    "the player's data file could not be read".to_string(),
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
            }
            Self::Running => ("stop the server first".to_string(), StatusCode::CONFLICT),
            Self::NotRunning => ("start the server first".to_string(), StatusCode::CONFLICT),
            Self::NotOnline => ("the player is not online".to_string(), StatusCode::CONFLICT),
            Self::Invalid(message) => (message, StatusCode::BAD_REQUEST),
            Self::Forbidden(permission) => (
                format!("you do not have permission to perform this action: {permission}"),
                StatusCode::FORBIDDEN,
            ),
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
                last_played: None,
            });
            players.len() - 1
        }
    };

    &mut players[index]
}

pub(super) struct Files {
    pub(super) properties: Option<String>,
    whitelist: Vec<serde_json::Map<String, serde_json::Value>>,
    ops: Vec<serde_json::Map<String, serde_json::Value>>,
    bans: Vec<serde_json::Map<String, serde_json::Value>>,
}

impl Files {
    pub(super) async fn read(
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

    pub(super) fn flag(&self, key: &str, default: bool) -> bool {
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

    pub(super) fn roster(&self, running: bool) -> Roster {
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

/// The server's players: the three lists and everyone who has played.
pub async fn roster(state: &crate::State, server: &Server) -> Result<Roster, anyhow::Error> {
    let client = files::client(state, server).await?;
    let running = matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Running)
    );

    let files_now = Files::read(&client, server).await?;
    let mut roster = files_now.roster(running);

    // who has played is extra: the lists stay useful when the world's folder cannot be listed
    if let Some(world) = super::player_data::world(files_now.properties.as_deref()) {
        match joined(&client, server, &world).await {
            Ok(joined) => {
                let cache = files::read_text(&client, server, USERCACHE, MAX_SIZE)
                    .await
                    .ok()
                    .flatten();
                merge_joined(&mut roster.players, joined, &usercache(cache.as_deref()));
            }
            Err(err) => tracing::warn!("listing the players who joined: {err:#}"),
        }
    }

    Ok(roster)
}

/// The players with a data file in `<world>/playerdata`, with when it was last written.
async fn joined(
    client: &wings_api::client::WingsClient,
    server: &Server,
    world: &str,
) -> Result<Vec<(compact_str::CompactString, chrono::DateTime<chrono::Utc>)>, anyhow::Error> {
    let mut joined = Vec::new();

    for page in 1..=MAX_JOINED / JOINED_PAGE {
        let list = match client
            .get_servers_server_files_list(
                server.uuid,
                &wings_api::servers_server_files_list::get::Query {
                    directory: Some(format!("/{world}/playerdata").into()),
                    per_page: Some(JOINED_PAGE),
                    page: Some(page),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(list) => list,
            Err(wings_api::client::ApiHttpError::Http(status, _))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        };

        let count = list.entries.len() as u64;
        joined.extend(list.entries.into_iter().filter_map(|entry| {
            if !entry.file {
                return None;
            }
            let stem = entry.name.strip_suffix(".dat")?;
            let uuid = uuid::Uuid::try_parse(stem).ok()?;

            Some((
                uuid.hyphenated().to_string().into(),
                entry.modified.with_timezone(&chrono::Utc),
            ))
        }));

        if count < JOINED_PAGE || page * JOINED_PAGE >= list.total {
            break;
        }
    }

    Ok(joined)
}

/// `usercache.json`: the names the server knows, by UUID (lowercase).
pub(super) fn usercache(
    raw: Option<&str>,
) -> Vec<(compact_str::CompactString, compact_str::CompactString)> {
    entries(raw)
        .iter()
        .filter(|entry| !text(entry, "uuid").is_empty())
        .map(|entry| {
            (
                text(entry, "uuid").to_ascii_lowercase().into(),
                text(entry, "name"),
            )
        })
        .collect()
}

/// Adds the players who joined to the lists' rows: a listed player gets when they last played,
/// anyone else a row of their own, named from the user cache when it knows them (unnamed rows
/// sort last).
fn merge_joined(
    players: &mut Vec<Player>,
    joined: Vec<(compact_str::CompactString, chrono::DateTime<chrono::Utc>)>,
    cache: &[(compact_str::CompactString, compact_str::CompactString)],
) {
    for (uuid, modified) in joined {
        let name = cache
            .iter()
            .find(|(cached, _)| cached.eq_ignore_ascii_case(&uuid))
            .map(|(_, name)| name.clone());

        let index = players
            .iter()
            .position(|player| player.uuid.eq_ignore_ascii_case(&uuid))
            .or_else(|| {
                let name = name.as_deref()?;
                players.iter().position(|player| {
                    player.uuid.is_empty() && player.name.eq_ignore_ascii_case(name)
                })
            });

        match index {
            Some(index) => {
                let player = &mut players[index];
                if player.uuid.is_empty() {
                    player.uuid = uuid;
                }
                player.last_played = Some(modified);
            }
            None => players.push(Player {
                name: name.unwrap_or_default(),
                uuid,
                whitelisted: false,
                operator: None,
                ban: None,
                last_played: Some(modified),
            }),
        }
    }

    players.sort_by(|a, b| {
        (a.name.is_empty(), a.name.to_ascii_lowercase(), &a.uuid).cmp(&(
            b.name.is_empty(),
            b.name.to_ascii_lowercase(),
            &b.uuid,
        ))
    });
}

/// Someone on the server right now, from the console's `list uuids`.
#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OnlinePlayer {
    pub name: compact_str::CompactString,
    /// Missing on servers older than 1.13, whose `list` names only.
    pub uuid: Option<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Online {
    pub running: bool,
    /// Who is online; `None` when it could not be told (no console permission, or the console
    /// did not answer `list` in time). A stopped server has nobody online.
    pub players: Option<Vec<OnlinePlayer>>,
    /// The player limit the server reported.
    pub max: Option<u32>,
}

impl Online {
    /// Whether the player with this UUID is online, `None` when it cannot be told.
    pub fn has(&self, uuid: &str) -> Option<bool> {
        if !self.running {
            return Some(false);
        }
        let players = self.players.as_ref()?;
        if players.iter().any(|player| player.uuid.is_none()) {
            return None;
        }

        Some(players.iter().any(|player| {
            player
                .uuid
                .as_deref()
                .is_some_and(|online| online.eq_ignore_ascii_case(uuid))
        }))
    }
}

/// Who is online. Needs the console (`console`): the server is asked `list uuids` and the answer
/// read from the console's last lines; it is reused for a few seconds.
pub async fn online(
    state: &crate::State,
    server: &Server,
    console: bool,
) -> Result<Online, PlayerError> {
    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    if !matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Running)
    ) {
        return Ok(Online {
            running: false,
            players: Some(Vec::new()),
            max: None,
        });
    }
    if !console {
        return Ok(Online {
            running: true,
            players: None,
            max: None,
        });
    }

    let key = format!("minecraft::online::{}", server.uuid);
    if let Ok(Some(cached)) = state.cache.get::<Online>(&key).await {
        return Ok(cached);
    }
    // one `list` at a time per server: a second page asking waits for the first answer
    let _lock = state
        .cache
        .lock(
            format!("minecraft::online_lock::{}", server.uuid),
            Some(10),
            Some(4000),
        )
        .await
        .ok();
    if let Ok(Some(cached)) = state.cache.get::<Online>(&key).await {
        return Ok(cached);
    }

    // Bukkit's plugins (Essentials) replace `list`; `minecraft:list` is always the game's own,
    // but only Bukkit servers know that name
    let paper = super::software(state, server)
        .await
        .is_ok_and(|software| software.loader == Some(super::Loader::Paper));
    let command = if paper {
        "minecraft:list uuids"
    } else {
        "list uuids"
    };

    let before = console_lines(&client, server).await;
    if files::command(&client, server, command).await.is_err() {
        return Ok(Online {
            running: true,
            players: None,
            max: None,
        });
    }

    for _ in 0..ONLINE_TRIES {
        tokio::time::sleep(ONLINE_WAIT).await;

        let after = console_lines(&client, server).await;
        if let Some((players, max)) = parse_list(&new_lines(&before, after)) {
            let online = Online {
                running: true,
                players: Some(players),
                max,
            };
            state.cache.set(&key, ONLINE_TTL, &online).await.ok();

            return Ok(online);
        }
    }

    Ok(Online {
        running: true,
        players: None,
        max: None,
    })
}

async fn console_lines(client: &wings_api::client::WingsClient, server: &Server) -> Vec<String> {
    files::console_tail(client, server, 80, 128 * 1024)
        .await
        .unwrap_or_default()
        .lines()
        .map(clean_line)
        .collect()
}

/// A console line without its colours (ANSI escapes, `§` codes).
fn clean_line(line: &str) -> String {
    let mut clean = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                // CSI: ESC [ … final byte in @..~
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
            }
            '§' => {
                chars.next();
            }
            c if c.is_control() => {}
            c => clean.push(c),
        }
    }

    clean.trim().to_string()
}

/// The lines that arrived after `before`'s last line.
fn new_lines(before: &[String], after: Vec<String>) -> Vec<String> {
    let Some(last) = before.iter().rev().find(|line| !line.is_empty()) else {
        return after;
    };

    match after.iter().rposition(|line| line == last) {
        Some(index) => after[index + 1..].to_vec(),
        None => after,
    }
}

/// The message of a log line, without the time and thread in front (`[12:00:00] [Server
/// thread/INFO]: `, `[12:00:00 INFO]: `).
fn message(line: &str) -> &str {
    match line.find("]: ") {
        Some(index) if line.starts_with('[') => line[index + 3..].trim(),
        _ => line.trim(),
    }
}

/// The last answer of `list` in the lines: who is online and the limit. 1.13+ answers in one
/// line (`There are 2 of a max of 20 players online: Alex (uuid), Steve (uuid)`), 1.12 in two
/// (`There are 2/20 players online:`, then the names).
fn parse_list(lines: &[String]) -> Option<(Vec<OnlinePlayer>, Option<u32>)> {
    static LIST: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"^There are (\d+)(?: of a max(?:imum)? of |/| out of maximum )(\d+) players online[:.]?\s*(.*)$",
        )
        .expect("valid regex")
    });
    static NAMED: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^(.+?) \(([0-9a-fA-F]{8}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{12})\)$")
            .expect("valid regex")
    });

    let (index, captures) = lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, line)| LIST.captures(message(line)).map(|c| (index, c)))?;

    let count: usize = captures[1].parse().ok()?;
    let max = captures[2].parse::<u32>().ok();
    if count == 0 {
        return Some((Vec::new(), max));
    }

    let names = match captures[3].trim() {
        "" => message(lines.get(index + 1)?).to_string(),
        names => names.to_string(),
    };

    let players: Vec<OnlinePlayer> = names
        .split(", ")
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|entry| match NAMED.captures(entry) {
            Some(named) => OnlinePlayer {
                name: named[1].trim().into(),
                uuid: uuid::Uuid::try_parse(&named[2])
                    .ok()
                    .map(|uuid| uuid.hyphenated().to_string().into()),
            },
            None => OnlinePlayer {
                name: entry.into(),
                uuid: None,
            },
        })
        .collect();

    Some((players, max))
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

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|line| clean_line(line)).collect()
    }

    #[test]
    fn list_answers_are_read() {
        // 1.13+, vanilla's log format, with uuids
        let (players, max) = parse_list(&lines(&[
            "[12:00:00] [Server thread/INFO]: Done (3.2s)!",
            "[12:00:01] [Server thread/INFO]: There are 2 of a max of 20 players online: Alex (6AB43178-89FD-4905-97F6-0F67D9D76FD9), .Bedrock_1 (00000000-0000-0000-0009-01f64f65c7c3)",
        ]))
        .unwrap();
        assert_eq!(max, Some(20));
        assert_eq!(
            players,
            vec![
                OnlinePlayer {
                    name: "Alex".into(),
                    uuid: Some("6ab43178-89fd-4905-97f6-0f67d9d76fd9".into()),
                },
                OnlinePlayer {
                    name: ".Bedrock_1".into(),
                    uuid: Some("00000000-0000-0000-0009-01f64f65c7c3".into()),
                },
            ]
        );

        // Paper's log format with colours, nobody online
        let (players, max) = parse_list(&lines(&[
            "\u{1b}[36m[12:00:00 INFO]: \u{1b}[0mThere are 0 of a max of 10 players online: ",
        ]))
        .unwrap();
        assert!(players.is_empty());
        assert_eq!(max, Some(10));

        // 1.12: the names on the next line, no uuids
        let (players, _) = parse_list(&lines(&[
            "[12:00:00] [Server thread/INFO]: There are 1/20 players online:",
            "[12:00:00] [Server thread/INFO]: Steve",
        ]))
        .unwrap();
        assert_eq!(players[0].name, "Steve");
        assert_eq!(players[0].uuid, None);

        // the last answer counts; no answer is "unknown"
        let (players, _) = parse_list(&lines(&[
            "[1] [Server thread/INFO]: There are 1 of a max of 20 players online: Steve",
            "[2] [Server thread/INFO]: There are 0 of a max of 20 players online: ",
        ]))
        .unwrap();
        assert!(players.is_empty());
        assert!(parse_list(&lines(&["[1] [Server thread/INFO]: Unknown command"])).is_none());
    }

    #[test]
    fn only_new_console_lines_count() {
        let before = lines(&["a", "b"]);
        assert_eq!(new_lines(&before, lines(&["a", "b", "c"])), vec!["c"]);
        assert_eq!(new_lines(&before, lines(&["b", "c", "d"])), vec!["c", "d"]);
        assert_eq!(new_lines(&[], lines(&["x"])), vec!["x"]);
        // the window moved past what was seen: everything is new
        assert_eq!(new_lines(&before, lines(&["y", "z"])), vec!["y", "z"]);
    }

    #[test]
    fn online_is_known_only_with_uuids() {
        let online = Online {
            running: true,
            players: Some(vec![OnlinePlayer {
                name: "Alex".into(),
                uuid: Some("6ab43178-89fd-4905-97f6-0f67d9d76fd9".into()),
            }]),
            max: Some(20),
        };
        assert_eq!(
            online.has("6AB43178-89FD-4905-97F6-0F67D9D76FD9"),
            Some(true)
        );
        assert_eq!(
            online.has("b50ad385-829d-3141-a216-7e7d7539ba7f"),
            Some(false)
        );

        let unknown = Online {
            running: true,
            players: None,
            max: None,
        };
        assert_eq!(unknown.has("x"), None);
        let stopped = Online {
            running: false,
            players: Some(Vec::new()),
            max: None,
        };
        assert_eq!(stopped.has("x"), Some(false));
    }

    #[test]
    fn players_who_joined_join_the_roster() {
        let files = Files {
            properties: None,
            whitelist: entries(Some(
                r#"[{"uuid":"6ab43178-89fd-4905-97f6-0f67d9d76fd9","name":"Alex"},{"name":"Steve"}]"#,
            )),
            ops: Vec::new(),
            bans: Vec::new(),
        };
        let mut players = files.roster(false).players;
        let at = chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let cache = usercache(Some(
            r#"[{"name":"Steve","uuid":"8667BA71-B85A-4004-AF54-457A9734EED7","expiresOn":"2026-11-01 10:00:00 +0000"},{"name":"Zed","uuid":"b50ad385-829d-3141-a216-7e7d7539ba7f"}]"#,
        ));

        merge_joined(
            &mut players,
            vec![
                ("6ab43178-89fd-4905-97f6-0f67d9d76fd9".into(), at),
                ("8667ba71-b85a-4004-af54-457a9734eed7".into(), at),
                ("b50ad385-829d-3141-a216-7e7d7539ba7f".into(), at),
                ("0d472393-237b-483f-b337-a31d485a6f47".into(), at),
            ],
            &cache,
        );

        let rows: Vec<(&str, &str, bool, bool)> = players
            .iter()
            .map(|player| {
                (
                    player.name.as_str(),
                    player.uuid.as_str(),
                    player.whitelisted,
                    player.last_played.is_some(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("Alex", "6ab43178-89fd-4905-97f6-0f67d9d76fd9", true, true),
                // the whitelist's entry without a uuid gets the one the cache knows
                ("Steve", "8667ba71-b85a-4004-af54-457a9734eed7", true, true),
                ("Zed", "b50ad385-829d-3141-a216-7e7d7539ba7f", false, true),
                // a file nobody can name sorts last
                ("", "0d472393-237b-483f-b337-a31d485a6f47", false, true),
            ]
        );
    }
}
