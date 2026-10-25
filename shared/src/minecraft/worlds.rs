//! plugged.host: a Minecraft server's worlds (docs/features/minecraft.md): which folders are
//! worlds, which one `server.properties` loads, and switching, replacing, deleting and importing
//! them.
//!
//! A world is a folder in the server root with a `level.dat`. Paper-family servers keep the nether
//! and the end beside it as `<world>_nether` and `<world>_the_end`; those belong to the world and
//! are renamed, downloaded and deleted with it. Names come from the live listing or are checked,
//! so nothing here reaches outside the server's root.

use super::{config, files};
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Where an uploaded world archive waits to be unpacked.
pub const UPLOAD_DIRECTORY: &str = ".world-upload";
/// The archives Wings unpacks that a world may come in.
pub const ARCHIVES: &[&str] = &[".zip", ".tar", ".tar.gz", ".tgz", ".tar.xz", ".tar.zst"];
const DIMENSIONS: [&str; 2] = ["_nether", "_the_end"];
/// Root folders that are never worlds, so they are not even looked into.
const NOT_WORLDS: &[&str] = &[
    "plugins",
    "mods",
    "config",
    "defaultconfigs",
    "libraries",
    "logs",
    "cache",
    "versions",
    "crash-reports",
    "bundler",
    "kubejs",
    "journeymap",
    "world-backups",
    "backups",
    "shaderpacks",
    "resourcepacks",
    "datapacks",
    "scripts",
    "local",
    "packmenu",
    "patchouli_books",
];
/// How many folders are looked into for a `level.dat` per listing.
const MAX_LOOKUPS: usize = 12;
const PROPERTIES: &str = "server.properties";
const PROPERTIES_SIZE: u64 = 512 * 1024;
/// How long an import's state is kept: a running one longer than this was lost with a restart.
const RUNNING_TTL: u64 = 2 * 60 * 60;
const DONE_TTL: u64 = 10 * 60;
const FAILED_TTL: u64 = 60 * 60;

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct World {
    pub name: String,
    /// The world `server.properties` loads.
    pub active: bool,
    /// False for the loaded world until its first start generates it.
    pub exists: bool,
    /// The world's folder and its dimension folders that exist.
    pub folders: Vec<String>,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// What the next new world is generated with (`server.properties`).
#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Generation {
    /// `level-name`
    pub name: String,
    /// `level-seed`, empty for a random one
    pub seed: String,
    /// `level-type`
    pub level_type: String,
    /// `generate-structures`
    pub structures: bool,
    /// `generator-settings`
    pub generator: String,
    /// `initial-enabled-packs`
    pub datapacks: Vec<String>,
}

/// How the loaded world is played (`server.properties`), shown beside how it is generated.
#[derive(ToSchema, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Gameplay {
    /// `difficulty`: peaceful, easy, normal or hard
    pub difficulty: String,
    /// `gamemode`: survival, creative, adventure or spectator
    pub gamemode: String,
    /// `hardcore`
    pub hardcore: bool,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImportState {
    Running,
    Done,
    Failed,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Import {
    pub archive: String,
    pub state: ImportState,
    pub error: Option<String>,
    /// The name the old world was kept under.
    pub kept: Option<String>,
    pub started: chrono::DateTime<chrono::Utc>,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Worlds {
    /// The loaded world first, then the others by name.
    pub worlds: Vec<World>,
    pub generation: Generation,
    pub gameplay: Gameplay,
    pub level_types: Vec<&'static str>,
    pub running: bool,
    /// Deleting moves the folders to the server's trash.
    pub trash: bool,
    pub import: Option<Import>,
}

/// What replacing the loaded world did with the old one.
#[derive(ToSchema, Serialize, Debug, Default)]
pub struct Replaced {
    /// The name the old world was kept under, when it was kept.
    pub kept: Option<String>,
    /// The folders deleted (or moved to the trash).
    pub removed: Vec<String>,
}

#[derive(Debug)]
pub enum WorldError {
    InvalidName,
    UnknownWorld,
    Active,
    Exists(String),
    Running,
    Importing,
    Invalid(String),
    NoArchive,
    NoWorld,
    Wings(anyhow::Error),
}

impl From<anyhow::Error> for WorldError {
    fn from(err: anyhow::Error) -> Self {
        Self::Wings(err)
    }
}

impl From<wings_api::client::ApiHttpError> for WorldError {
    fn from(err: wings_api::client::ApiHttpError) -> Self {
        Self::Wings(err.into())
    }
}

impl WorldError {
    /// The text a person reads, also kept as a failed import's reason.
    pub fn message(&self) -> String {
        match self {
            Self::InvalidName => "a world name has 1 to 64 letters, digits, spaces, dots, dashes and underscores and starts with a letter or digit".into(),
            Self::UnknownWorld => "there is no world with that name".into(),
            Self::Active => "the loaded world cannot be deleted, switch to another one first".into(),
            Self::Exists(name) => format!("there already is a folder called {name}"),
            Self::Running => "stop the server first".into(),
            Self::Importing => "a world is still being imported".into(),
            Self::Invalid(message) => message.clone(),
            Self::NoArchive => "the uploaded archive is not there, upload it again".into(),
            Self::NoWorld => "there is no level.dat in the archive, so it holds no world".into(),
            Self::Wings(_) => "the server's files could not be reached, try again in a moment".into(),
        }
    }

    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let status = match &self {
            Self::InvalidName | Self::Invalid(_) | Self::NoArchive | Self::NoWorld => {
                StatusCode::BAD_REQUEST
            }
            Self::UnknownWorld => StatusCode::NOT_FOUND,
            Self::Active | Self::Exists(_) | Self::Running | Self::Importing => {
                StatusCode::CONFLICT
            }
            Self::Wings(err) => {
                tracing::warn!("minecraft worlds: {err:#}");
                StatusCode::BAD_GATEWAY
            }
        };

        crate::response::ApiResponse::error(self.message()).with_status(status)
    }
}

/// A world name: a folder name in the server root, nothing that climbs out of it.
pub fn valid_name(name: &str) -> bool {
    name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '.' | '-'))
        && !name.contains("..")
}

/// An uploaded archive's file name: no path, an archive Wings unpacks.
pub fn valid_archive(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
        && !name.starts_with('.')
        && ARCHIVES
            .iter()
            .any(|extension| name.to_ascii_lowercase().ends_with(extension))
}

/// The world `server.properties` loads.
fn level_name(raw: &str) -> String {
    config::property(raw, "level-name")
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "world".into())
}

fn generation(raw: &str) -> Generation {
    let types = config::choices("level-type");
    let level_type = config::property(raw, "level-type").unwrap_or_default();

    Generation {
        name: level_name(raw),
        seed: config::property(raw, "level-seed").unwrap_or_default(),
        level_type: if types.contains(&level_type.as_str()) {
            level_type
        } else {
            "minecraft:normal".into()
        },
        structures: config::property(raw, "generate-structures")
            .is_none_or(|value| !value.eq_ignore_ascii_case("false")),
        generator: config::property(raw, "generator-settings")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "{}".into()),
        datapacks: datapacks(&config::property(raw, "initial-enabled-packs").unwrap_or_default()),
    }
}

fn gameplay(raw: &str) -> Gameplay {
    Gameplay {
        difficulty: choice(raw, "difficulty", "easy"),
        gamemode: choice(raw, "gamemode", "survival"),
        hardcore: config::property(raw, "hardcore")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true")),
    }
}

/// A `difficulty` or `gamemode` value by its name, the game's default when it is not set. Old
/// versions wrote the number (`difficulty=1` is easy), which the game still reads.
fn choice(raw: &str, key: &str, default: &str) -> String {
    let Some(value) = config::property(raw, key)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return default.into();
    };

    value
        .parse::<usize>()
        .ok()
        .and_then(|index| config::choices(key).get(index))
        .map_or(value, |name| name.to_string())
}

fn datapacks(value: &str) -> Vec<String> {
    let packs: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|pack| !pack.is_empty())
        .map(str::to_string)
        .collect();

    if packs.is_empty() {
        vec!["vanilla".into()]
    } else {
        packs
    }
}

/// Checks new generation settings and turns them into `server.properties` values.
fn generation_values(generation: &Generation) -> Result<Vec<(&'static str, String)>, WorldError> {
    if !valid_name(&generation.name) {
        return Err(WorldError::InvalidName);
    }
    if generation.seed.chars().count() > 64 || generation.seed.chars().any(char::is_control) {
        return Err(WorldError::Invalid(
            "the seed must be one line of at most 64 characters".into(),
        ));
    }
    if !config::choices("level-type").contains(&generation.level_type.as_str()) {
        return Err(WorldError::Invalid(format!(
            "{} is not a world type",
            generation.level_type
        )));
    }
    if generation.generator.chars().count() > 2000
        || generation.generator.chars().any(char::is_control)
    {
        return Err(WorldError::Invalid(
            "the generator settings must be one line of at most 2000 characters".into(),
        ));
    }
    if generation.datapacks.len() > 32
        || generation.datapacks.iter().any(|pack| {
            !(1..=64).contains(&pack.len())
                || !pack.bytes().all(|b| {
                    b.is_ascii_lowercase()
                        || b.is_ascii_digit()
                        || matches!(b, b'_' | b'.' | b':' | b'/' | b'-')
                })
        })
    {
        return Err(WorldError::Invalid(
            "a data pack name has lowercase letters, digits and _ . : / -".into(),
        ));
    }

    let packs = if generation.datapacks.is_empty() {
        "vanilla".to_string()
    } else {
        generation.datapacks.join(",")
    };

    Ok(vec![
        ("level-name", generation.name.clone()),
        ("level-seed", generation.seed.trim().to_string()),
        ("level-type", generation.level_type.clone()),
        ("generate-structures", generation.structures.to_string()),
        (
            "generator-settings",
            generation.generator.trim().to_string(),
        ),
        ("initial-enabled-packs", packs),
    ])
}

/// The folders of the server root that may be worlds: not hidden, not one of the folders that
/// never are, not the loaded world (always shown) and not another world's dimension folder.
fn candidates<'a>(directories: &[&'a str], active: &str) -> Vec<&'a str> {
    directories
        .iter()
        .copied()
        .filter(|name| {
            *name != active
                && !name.starts_with('.')
                && !NOT_WORLDS.contains(&name.to_ascii_lowercase().as_str())
                && !is_dimension(name, directories)
        })
        .collect()
}

fn is_dimension(name: &str, directories: &[&str]) -> bool {
    DIMENSIONS.iter().any(|suffix| {
        name.strip_suffix(suffix)
            .is_some_and(|base| directories.contains(&base))
    })
}

/// A world's folder and its dimension folders, those that exist.
fn folders_of(world: &str, directories: &[&str]) -> Vec<String> {
    std::iter::once(world.to_string())
        .chain(DIMENSIONS.iter().map(|suffix| format!("{world}{suffix}")))
        .filter(|folder| directories.contains(&folder.as_str()))
        .collect()
}

/// A name for keeping the old world beside the new one: `<world>-<date>`, numbered when taken.
fn kept_name(world: &str, now: chrono::DateTime<chrono::Utc>, directories: &[&str]) -> String {
    let base = format!("{world}-{}", now.format("%Y-%m-%d_%H-%M"));
    let base: String = base.chars().take(56).collect();

    let taken = |name: &str| {
        std::iter::once(name.to_string())
            .chain(DIMENSIONS.iter().map(|suffix| format!("{name}{suffix}")))
            .any(|folder| directories.contains(&folder.as_str()))
    };

    if !taken(&base) {
        return base;
    }

    (2..)
        .map(|number| format!("{base}-{number}"))
        .find(|name| !taken(name))
        .unwrap_or(base)
}

struct Entry {
    name: String,
    directory: bool,
    modified: chrono::DateTime<chrono::Utc>,
}

async fn listing(
    client: &wings_api::client::WingsClient,
    server: &Server,
    directory: &str,
) -> Result<Vec<Entry>, anyhow::Error> {
    let list = match client
        .get_servers_server_files_list(
            server.uuid,
            &wings_api::servers_server_files_list::get::Query {
                directory: Some(directory.into()),
                per_page: Some(500),
                ..Default::default()
            },
        )
        .await
    {
        Ok(list) => list,
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            return Ok(Vec::new());
        }
        Err(err) => return Err(err.into()),
    };

    Ok(list
        .entries
        .into_iter()
        .map(|entry| Entry {
            name: entry.name.to_string(),
            directory: entry.directory,
            modified: entry.modified.with_timezone(&chrono::Utc),
        })
        .collect())
}

fn directories(entries: &[Entry]) -> Vec<&str> {
    entries
        .iter()
        .filter(|entry| entry.directory)
        .map(|entry| entry.name.as_str())
        .collect()
}

fn describe(name: &str, active: bool, root: &[Entry]) -> World {
    let directories = directories(root);

    World {
        name: name.to_string(),
        active,
        exists: directories.contains(&name),
        folders: folders_of(name, &directories),
        modified: root
            .iter()
            .find(|entry| entry.directory && entry.name == name)
            .map(|entry| entry.modified),
    }
}

async fn has_level_data(
    client: &wings_api::client::WingsClient,
    server: &Server,
    directory: &str,
) -> Result<bool, anyhow::Error> {
    Ok(listing(client, server, directory)
        .await?
        .iter()
        .any(|entry| !entry.directory && entry.name == "level.dat"))
}

async fn properties(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<String, anyhow::Error> {
    Ok(
        files::read_text(client, server, PROPERTIES, PROPERTIES_SIZE)
            .await?
            .unwrap_or_default(),
    )
}

/// Whether the server is stopped; a Wings that does not answer counts as running.
async fn stopped(client: &wings_api::client::WingsClient, server: &Server) -> bool {
    matches!(
        files::power_state(client, server).await,
        Some(wings_api::ServerState::Offline)
    )
}

/// Deletes folders in the server root, into the trash when the server has one.
async fn remove_folders(
    state: &crate::State,
    client: &wings_api::client::WingsClient,
    server: &Server,
    folders: Vec<String>,
) -> Result<(), WorldError> {
    files::remove(
        state,
        client,
        server,
        "/",
        folders.into_iter().map(Into::into).collect(),
    )
    .await?;

    Ok(())
}

async fn rename(
    client: &wings_api::client::WingsClient,
    server: &Server,
    moves: Vec<(String, String)>,
) -> Result<(), WorldError> {
    if moves.is_empty() {
        return Ok(());
    }

    client
        .put_servers_server_files_rename(
            server.uuid,
            &wings_api::servers_server_files_rename::put::RequestBody {
                root: "/".into(),
                files: moves
                    .into_iter()
                    .map(|(from, to)| {
                        wings_api::servers_server_files_rename::put::RequestBodyFiles {
                            from: from.into(),
                            to: to.into(),
                        }
                    })
                    .collect(),
            },
        )
        .await?;

    Ok(())
}

/// Moves the loaded world out of the way: kept under a new name, or deleted.
async fn replace_active(
    state: &crate::State,
    client: &wings_api::client::WingsClient,
    server: &Server,
    active: &str,
    keep: bool,
    root: &[Entry],
) -> Result<Replaced, WorldError> {
    let directories = directories(root);
    let folders = folders_of(active, &directories);

    if folders.is_empty() {
        return Ok(Replaced::default());
    }

    if keep {
        let kept = kept_name(active, chrono::Utc::now(), &directories);
        let moves = folders
            .iter()
            .map(|folder| (folder.clone(), format!("{kept}{}", &folder[active.len()..])))
            .collect();
        rename(client, server, moves).await?;

        Ok(Replaced {
            kept: Some(kept),
            removed: Vec::new(),
        })
    } else {
        remove_folders(state, client, server, folders.clone()).await?;

        Ok(Replaced {
            kept: None,
            removed: folders,
        })
    }
}

fn import_key(server: &Server) -> String {
    format!("minecraft::worlds::{}::import", server.uuid)
}

async fn current_import(state: &crate::State, server: &Server) -> Option<Import> {
    state
        .cache
        .get::<Import>(&import_key(server))
        .await
        .ok()
        .flatten()
}

async fn store_import(state: &crate::State, server: &Server, import: &Import) {
    let ttl = match import.state {
        ImportState::Running => RUNNING_TTL,
        ImportState::Done => DONE_TTL,
        ImportState::Failed => FAILED_TTL,
    };

    state.cache.set(&import_key(server), ttl, import).await.ok();
}

/// The worlds on the server, the loaded one first.
pub async fn load(state: &crate::State, server: &Server) -> Result<Worlds, WorldError> {
    let client = files::client(state, server).await?;
    let raw = properties(&client, server).await?;
    let active = level_name(&raw);
    let root = listing(&client, server, "/").await?;
    let names = directories(&root);

    let mut worlds = Vec::new();
    for name in candidates(&names, &active).into_iter().take(MAX_LOOKUPS) {
        if has_level_data(&client, server, &format!("/{name}")).await? {
            worlds.push(describe(name, false, &root));
        }
    }
    worlds.sort_by_key(|world| world.name.to_lowercase());
    worlds.insert(0, describe(&active, true, &root));

    Ok(Worlds {
        worlds,
        generation: generation(&raw),
        gameplay: gameplay(&raw),
        level_types: config::choices("level-type").to_vec(),
        running: !stopped(&client, server).await,
        trash: files::trash_enabled(state, server).await?,
        import: current_import(state, server).await,
    })
}

/// Makes another world the loaded one, from the next start.
pub async fn activate(
    state: &crate::State,
    server: &Server,
    world: &str,
) -> Result<(), WorldError> {
    if !valid_name(world) {
        return Err(WorldError::InvalidName);
    }

    let client = files::client(state, server).await?;
    if !has_level_data(&client, server, &format!("/{world}")).await? {
        return Err(WorldError::UnknownWorld);
    }

    let raw = properties(&client, server).await?;
    if level_name(&raw) != world {
        let written = config::set_properties(&raw, &[("level-name", world.to_string())]);
        files::write_text(&client, server, PROPERTIES, written).await?;
    }

    Ok(())
}

/// Deletes a world that is not the loaded one; returns its folders.
pub async fn delete(
    state: &crate::State,
    server: &Server,
    world: &str,
) -> Result<Vec<String>, WorldError> {
    if !valid_name(world) {
        return Err(WorldError::InvalidName);
    }

    let client = files::client(state, server).await?;
    if level_name(&properties(&client, server).await?) == world {
        return Err(WorldError::Active);
    }

    let root = listing(&client, server, "/").await?;
    let folders = folders_of(world, &directories(&root));
    if folders.is_empty() {
        return Err(WorldError::UnknownWorld);
    }

    remove_folders(state, &client, server, folders.clone()).await?;

    Ok(folders)
}

/// A new world in place of the loaded one: the old one kept under a new name or deleted, then the
/// generation settings the next start creates the new one with.
pub async fn create(
    state: &crate::State,
    server: &Server,
    generation: &Generation,
    keep: bool,
) -> Result<Replaced, WorldError> {
    let values = generation_values(generation)?;
    let client = files::client(state, server).await?;
    if !stopped(&client, server).await {
        return Err(WorldError::Running);
    }

    let raw = properties(&client, server).await?;
    let active = level_name(&raw);
    let root = listing(&client, server, "/").await?;

    if generation.name != active
        && directories(&root)
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&generation.name))
    {
        return Err(WorldError::Exists(generation.name.clone()));
    }

    // A world under another name leaves the loaded one where it is when it is kept.
    let replaced = if generation.name != active && keep {
        Replaced::default()
    } else {
        replace_active(state, &client, server, &active, keep, &root).await?
    };

    let written = config::set_properties(&raw, &values);
    files::write_text(&client, server, PROPERTIES, written).await?;

    Ok(replaced)
}

/// Empties the upload folder for a new archive.
pub async fn prepare_upload(state: &crate::State, server: &Server) -> Result<(), WorldError> {
    let client = files::client(state, server).await?;
    if !stopped(&client, server).await {
        return Err(WorldError::Running);
    }
    if current_import(state, server)
        .await
        .is_some_and(|import| import.state == ImportState::Running)
    {
        return Err(WorldError::Importing);
    }

    clear_upload(&client, server).await;
    client
        .post_servers_server_files_create_directory(
            server.uuid,
            &wings_api::servers_server_files_create_directory::post::RequestBody {
                root: "/".into(),
                name: UPLOAD_DIRECTORY.into(),
            },
        )
        .await?;

    Ok(())
}

async fn clear_upload(client: &wings_api::client::WingsClient, server: &Server) {
    client
        .post_servers_server_files_delete(
            server.uuid,
            &wings_api::servers_server_files_delete::post::RequestBody {
                root: "/".into(),
                files: vec![UPLOAD_DIRECTORY.into()],
            },
        )
        .await
        .ok();
}

/// Checks an uploaded archive and marks the import as running; `import` does the rest.
pub async fn start_import(
    state: &crate::State,
    server: &Server,
    archive: &str,
) -> Result<(), WorldError> {
    if !valid_archive(archive) {
        return Err(WorldError::NoArchive);
    }

    let client = files::client(state, server).await?;
    if !stopped(&client, server).await {
        return Err(WorldError::Running);
    }
    if current_import(state, server)
        .await
        .is_some_and(|import| import.state == ImportState::Running)
    {
        return Err(WorldError::Importing);
    }
    if !listing(&client, server, &format!("/{UPLOAD_DIRECTORY}"))
        .await?
        .iter()
        .any(|entry| !entry.directory && entry.name == archive)
    {
        return Err(WorldError::NoArchive);
    }

    store_import(
        state,
        server,
        &Import {
            archive: archive.to_string(),
            state: ImportState::Running,
            error: None,
            kept: None,
            started: chrono::Utc::now(),
        },
    )
    .await;

    Ok(())
}

/// Unpacks the uploaded archive and makes the world in it the loaded one, keeping or deleting the
/// old one. Takes as long as the archive does, so it runs after the request; the page polls the
/// state `start_import` stored.
pub async fn import(
    state: &crate::State,
    server: &Server,
    archive: &str,
    keep: bool,
) -> Result<Replaced, WorldError> {
    let client = files::client(state, server).await?;
    let result = unpack(state, &client, server, archive, keep).await;
    clear_upload(&client, server).await;

    let mut import = current_import(state, server).await.unwrap_or(Import {
        archive: archive.to_string(),
        state: ImportState::Running,
        error: None,
        kept: None,
        started: chrono::Utc::now(),
    });
    match &result {
        Ok(replaced) => {
            import.state = ImportState::Done;
            import.kept = replaced.kept.clone();
        }
        Err(err) => {
            import.state = ImportState::Failed;
            import.error = Some(err.message());
        }
    }
    store_import(state, server, &import).await;

    result
}

async fn unpack(
    state: &crate::State,
    client: &wings_api::client::WingsClient,
    server: &Server,
    archive: &str,
    keep: bool,
) -> Result<Replaced, WorldError> {
    let upload = format!("/{UPLOAD_DIRECTORY}");

    client
        .post_servers_server_files_decompress(
            server.uuid,
            &wings_api::servers_server_files_decompress::post::RequestBody {
                root: upload.clone().into(),
                file: archive.into(),
                foreground: true,
            },
        )
        .await?;
    client
        .post_servers_server_files_delete(
            server.uuid,
            &wings_api::servers_server_files_delete::post::RequestBody {
                root: upload.clone().into(),
                files: vec![archive.into()],
            },
        )
        .await?;

    let source = find_level_data(client, server, UPLOAD_DIRECTORY)
        .await?
        .ok_or(WorldError::NoWorld)?;

    // The owner may have started the server while it unpacked.
    if !stopped(client, server).await {
        return Err(WorldError::Running);
    }

    let raw = properties(client, server).await?;
    let active = level_name(&raw);
    let root = listing(client, server, "/").await?;
    let replaced = replace_active(state, client, server, &active, keep, &root).await?;

    let mut moves = vec![(source.clone(), active.clone())];
    if let Some((parent, base)) = source.rsplit_once('/') {
        let siblings = listing(client, server, &format!("/{parent}")).await?;
        for suffix in DIMENSIONS {
            let folder = format!("{base}{suffix}");
            if siblings
                .iter()
                .any(|entry| entry.directory && entry.name == folder)
            {
                moves.push((format!("{parent}/{folder}"), format!("{active}{suffix}")));
            }
        }
    }
    rename(client, server, moves).await?;

    Ok(replaced)
}

/// Where `level.dat` is in the unpacked upload: the upload folder itself or a folder up to two
/// levels down (archives often wrap the world in a folder of their own).
async fn find_level_data(
    client: &wings_api::client::WingsClient,
    server: &Server,
    directory: &str,
) -> Result<Option<String>, anyhow::Error> {
    let mut queue = vec![(directory.to_string(), 0)];

    while let Some((current, depth)) = queue.pop() {
        let entries = listing(client, server, &format!("/{current}")).await?;
        if entries
            .iter()
            .any(|entry| !entry.directory && entry.name == "level.dat")
        {
            return Ok(Some(current));
        }

        if depth < 2 {
            queue.extend(
                entries
                    .iter()
                    .filter(|entry| entry.directory && !entry.name.starts_with("__MACOSX"))
                    .map(|entry| (format!("{current}/{}", entry.name), depth + 1))
                    .rev(),
            );
        }
    }

    Ok(None)
}

/// Writes the world to disk before it is downloaded from a running server.
pub async fn save(state: &crate::State, server: &Server) -> Result<bool, WorldError> {
    let client = files::client(state, server).await?;
    if stopped(&client, server).await {
        return Ok(false);
    }

    files::command(&client, server, "save-all flush").await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_in_the_root() {
        assert!(valid_name("world"));
        assert!(valid_name("My World 2.0"));
        assert!(!valid_name(""));
        assert!(!valid_name(".hidden"));
        assert!(!valid_name("a/b"));
        assert!(!valid_name("a..b"));
        assert!(!valid_name(&"a".repeat(65)));

        assert!(valid_archive("world.zip"));
        assert!(valid_archive("World Backup.TAR.GZ"));
        assert!(!valid_archive("world.rar"));
        assert!(!valid_archive("../world.zip"));
        assert!(!valid_archive(".world.zip"));
    }

    #[test]
    fn worlds_and_their_dimensions() {
        let directories = [
            "world",
            "world_nether",
            "world_the_end",
            "creative",
            "creative_nether",
            "plugins",
            "Logs",
            ".world-upload",
            "lonely_nether",
        ];

        assert_eq!(
            candidates(&directories, "world"),
            vec!["creative", "lonely_nether"]
        );
        assert_eq!(
            folders_of("world", &directories),
            vec!["world", "world_nether", "world_the_end"]
        );
        assert_eq!(
            folders_of("creative", &directories),
            vec!["creative", "creative_nether"]
        );
        assert!(folders_of("missing", &directories).is_empty());
    }

    #[test]
    fn kept_worlds_get_a_free_name() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-24T02:10:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        assert_eq!(
            kept_name("world", now, &["world"]),
            "world-2026-10-24_02-10"
        );
        assert_eq!(
            kept_name("world", now, &["world", "world-2026-10-24_02-10_nether"]),
            "world-2026-10-24_02-10-2"
        );
    }

    #[test]
    fn generation_is_read_and_checked() {
        let raw = "level-name=survival\nlevel-seed=-42\nlevel-type=minecraft\\:flat\ngenerate-structures=false\ninitial-enabled-packs=vanilla, bundle\n";
        let read = generation(raw);
        assert_eq!(read.name, "survival");
        assert_eq!(read.seed, "-42");
        assert_eq!(read.level_type, "minecraft:flat");
        assert!(!read.structures);
        assert_eq!(read.generator, "{}");
        assert_eq!(read.datapacks, vec!["vanilla", "bundle"]);

        let values = generation_values(&read).unwrap();
        assert!(values.contains(&("initial-enabled-packs", "vanilla,bundle".into())));

        let defaults = generation("");
        assert_eq!(defaults.name, "world");
        assert_eq!(defaults.level_type, "minecraft:normal");
        assert!(defaults.structures);
        assert_eq!(defaults.datapacks, vec!["vanilla"]);

        let mut bad = read.clone();
        bad.level_type = "minecraft:void".into();
        assert!(matches!(
            generation_values(&bad),
            Err(WorldError::Invalid(_))
        ));

        let mut bad = read.clone();
        bad.datapacks = vec!["Not Valid".into()];
        assert!(matches!(
            generation_values(&bad),
            Err(WorldError::Invalid(_))
        ));

        let mut bad = read;
        bad.name = "../escape".into();
        assert!(matches!(
            generation_values(&bad),
            Err(WorldError::InvalidName)
        ));
    }

    #[test]
    fn gameplay_is_read_by_name() {
        let read = gameplay("difficulty=hard\ngamemode=Creative\nhardcore=true\n");
        assert_eq!(read.difficulty, "hard");
        assert_eq!(read.gamemode, "creative");
        assert!(read.hardcore);

        let defaults = gameplay("difficulty=\nhardcore=false\n");
        assert_eq!(defaults.difficulty, "easy");
        assert_eq!(defaults.gamemode, "survival");
        assert!(!defaults.hardcore);

        let numbers = gameplay("difficulty=0\ngamemode=3\n");
        assert_eq!(numbers.difficulty, "peaceful");
        assert_eq!(numbers.gamemode, "spectator");

        let unknown = gameplay("difficulty=9\ngamemode=custom\n");
        assert_eq!(unknown.difficulty, "9");
        assert_eq!(unknown.gamemode, "custom");
    }
}
