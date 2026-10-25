//! plugged.host: one-click frameworks for FiveM and RedM servers: txAdmin's official recipes
//! (ESX Legacy, QBCore, Qbox, vMenu, the plain Cfx server, the RedM ones), installed without
//! txAdmin's setup screens (docs/features/fivem.md#frameworks).
//!
//! An install asks what txAdmin's deployer asks (a server name, the Cfx license key and, for a
//! recipe with a database, which one), then runs in the background:
//!
//! 1. prepare: a fresh deploy folder, `txData/<Name>_<XXXXXX>.base` (txAdmin's own naming), and
//!    the database (a new one from the server's database slots, or one it has);
//! 2. tasks: the recipe, into that folder, while the server keeps running;
//! 3. config: server.cfg's `{{variables}}` filled;
//! 4. restart: the server stops, txAdmin is pointed at the new folder, and it starts again. The
//!    only downtime is this step.
//!
//! Nothing the server had is touched: the folder txAdmin ran before stays where it is, and the
//! page names it. A failed install removes the half-built folder and the database it created,
//! and leaves txAdmin pointing where it pointed.
//!
//! Only recipes from txAdmin's index run. The page names a recipe by an id the index gives, and
//! the install looks it up there again, so nothing a browser sends becomes a recipe URL.

use super::{recipe, txadmin};
use crate::{
    minecraft::files,
    models::{
        CreatableModel, DeletableModel,
        database_host::{DatabaseHost, DatabaseType},
        server::{Server, ServerActivityLogger},
        server_database::{
            CreateServerDatabaseOptions, DeleteServerDatabaseOptions, ServerDatabase,
        },
    },
};
use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// txAdmin's recipe index: what its setup shows under "Popular recipes".
const INDEX: &str = "https://raw.githubusercontent.com/tabarra/txAdmin-recipes/main/indexv4.json";
const TTL: u64 = 600;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const RECIPE_LIMIT: usize = 1024 * 1024;
/// How long a finished install's outcome stays on the page.
const KEEP: u64 = 24 * 60 * 60;
/// An install that has not moved for this long was cut off (the panel restarted under it); no
/// task may take longer (30 minutes at most).
const STALE_MINUTES: i64 = 40;

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    Fivem,
    Redm,
}

impl Game {
    fn tag(self) -> &'static str {
        match self {
            Self::Fivem => "fivem",
            Self::Redm => "redm",
        }
    }
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Recipe {
    /// The start of the SHA-256 of its index URL.
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: Option<String>,
    pub version: Option<String>,
    /// The GitHub owner's avatar (the framework's own logo for every official recipe).
    pub logo: Option<String>,
    pub needs_database: bool,
    pub tasks: usize,
}

pub fn id(url: &str) -> String {
    hex::encode(&sha2::Sha256::digest(url.as_bytes())[..8])
}

fn logo(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://raw.githubusercontent.com/")
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let owner = rest.split('/').next()?;

    (!owner.is_empty()
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-'))
    .then(|| format!("https://github.com/{owner}.png?size=96"))
}

fn scalar(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

/// The index's entries a recipe can come from: https URLs of engine 3.
pub fn entries(index: &serde_json::Value) -> Vec<serde_json::Value> {
    index
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry
                .get("url")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|url| url.starts_with("https://"))
                && entry.get("engine").and_then(serde_json::Value::as_i64) == Some(3)
        })
        .cloned()
        .collect()
}

/// A recipe's YAML, read; `None` when it has no task list.
pub fn parse_recipe(yaml: &str) -> Option<serde_json::Value> {
    serde_saphyr::from_str::<serde_json::Value>(yaml)
        .ok()
        .filter(|recipe| recipe.get("tasks").is_some_and(serde_json::Value::is_array))
}

async fn fetch(state: &crate::State, url: &str) -> Option<String> {
    let response = state
        .client
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let text = response.text().await.ok()?;

    (text.len() <= RECIPE_LIMIT).then_some(text)
}

async fn index(state: &crate::State) -> Vec<serde_json::Value> {
    super::remember(state, "fivem::recipes::index", TTL, || async {
        let index: serde_json::Value = serde_json::from_str(&fetch(state, INDEX).await?).ok()?;
        index.is_array().then_some(index)
    })
    .await
    .map(|index| entries(&index))
    .unwrap_or_default()
}

async fn fetch_recipe(state: &crate::State, url: &str) -> Option<serde_json::Value> {
    super::remember(
        state,
        &format!("fivem::recipes::{}", id(url)),
        TTL,
        || async { parse_recipe(&fetch(state, url).await?) },
    )
    .await
}

fn describe(entry: &serde_json::Value, recipe: &serde_json::Value) -> Recipe {
    let url = entry
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    Recipe {
        id: id(url),
        name: scalar(recipe.get("name"))
            .or_else(|| scalar(entry.get("name")))
            .unwrap_or_else(|| "Recipe".into()),
        description: scalar(entry.get("description"))
            .or_else(|| scalar(recipe.get("description")))
            .unwrap_or_default(),
        author: scalar(recipe.get("author")),
        version: scalar(recipe.get("version")),
        logo: logo(url),
        needs_database: recipe::needs_database(recipe),
        tasks: recipe
            .get("tasks")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len),
    }
}

/// The recipes for a game, in the index's order.
pub async fn for_game(state: &crate::State, game: Game) -> Vec<Recipe> {
    let entries: Vec<serde_json::Value> = index(state)
        .await
        .into_iter()
        .filter(|entry| {
            entry
                .get("tags")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some(game.tag())))
        })
        .collect();
    let recipes = futures_util::future::join_all(entries.iter().map(|entry| {
        fetch_recipe(
            state,
            entry
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
        )
    }))
    .await;

    entries
        .iter()
        .zip(recipes)
        .filter_map(|(entry, recipe)| Some(describe(entry, &recipe?)))
        .collect()
}

/// One recipe of the index by its id, with its body.
async fn find(state: &crate::State, id_: &str) -> Option<(Recipe, serde_json::Value)> {
    for entry in index(state).await {
        let url = entry
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if id(url) == id_ {
            let body = fetch_recipe(state, url).await?;
            return Some((describe(&entry, &body), body));
        }
    }

    None
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Prepare,
    Tasks,
    Config,
    Restart,
    Done,
    Failed,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Install {
    pub recipe: String,
    pub step: Step,
    /// The task running now (from 1) of `total`.
    pub task: u32,
    pub total: u32,
    /// What the task does, in a few words (`download github esx-framework/esx_core`).
    pub current: Option<String>,
    /// The new server data folder.
    pub deploy: Option<String>,
    /// The folder txAdmin ran before, which keeps the server's previous files.
    pub previous: Option<String>,
    /// The database the install created.
    pub database: Option<String>,
    /// txAdmin has no admin yet, so it waits for one before it starts the server.
    pub needs_admin: bool,
    pub error: Option<String>,
    pub started: chrono::DateTime<chrono::Utc>,
    pub updated: chrono::DateTime<chrono::Utc>,
}

impl Install {
    pub fn running(&self) -> bool {
        !matches!(self.step, Step::Done | Step::Failed)
    }
}

fn key(server: &Server) -> String {
    format!("fivem::frameworks::{}", server.uuid)
}

async fn store(state: &crate::State, server: &Server, install: &mut Install) {
    install.updated = chrono::Utc::now();
    state.cache.set(&key(server), KEEP, install).await.ok();
}

/// The server's install, a cut-off one marked as failed.
pub async fn current(state: &crate::State, server: &Server) -> Option<Install> {
    let mut install: Install = state.cache.get(&key(server)).await.ok().flatten()?;
    if install.running()
        && chrono::Utc::now() - install.updated > chrono::Duration::minutes(STALE_MINUTES)
    {
        install.step = Step::Failed;
        install.error = Some("the install stopped: the panel restarted while it ran".into());
        store(state, server, &mut install).await;
    }

    Some(install)
}

/// Forgets a finished install, so the page lists the frameworks again.
pub async fn dismiss(state: &crate::State, server: &Server) -> Result<(), anyhow::Error> {
    if !current(state, server)
        .await
        .is_some_and(|install| install.running())
    {
        state.cache.invalidate(&key(server)).await?;
    }

    Ok(())
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct DatabaseOption {
    pub uuid: uuid::Uuid,
    pub name: String,
}

/// What the Frameworks page shows.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Frameworks {
    pub game: Game,
    /// The server data folder txAdmin runs now; `null` while txAdmin is not set up.
    pub current: Option<String>,
    /// The node did not answer, so `current` and `game` are guesses.
    pub unreachable: bool,
    pub recipes: Vec<Recipe>,
    pub install: Option<Install>,
    /// The server's MySQL databases, for a recipe that needs one.
    pub databases: Vec<DatabaseOption>,
    pub can_create_database: bool,
    /// The license key in the current server.cfg, to start the form with.
    pub license: Option<String>,
}

fn valid_license(license: &str) -> bool {
    !license.is_empty()
        && license.len() <= 128
        && license
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// A MySQL host of the server's node that takes new databases.
async fn database_host(
    state: &crate::State,
    server: &Server,
) -> Result<Option<DatabaseHost>, anyhow::Error> {
    let node = server.node.fetch_cached(&state.database).await?;

    Ok(DatabaseHost::all_public_by_node(&state.database, &node)
        .await?
        .into_iter()
        .find(|host| host.r#type == DatabaseType::Mysql && !host.maintenance_enabled))
}

async fn can_create_database(state: &crate::State, server: &Server) -> Result<bool, anyhow::Error> {
    if ServerDatabase::count_by_server_uuid(&state.database, server.uuid).await?
        >= server.database_limit as i64
    {
        return Ok(false);
    }

    Ok(database_host(state, server).await?.is_some())
}

pub struct Access {
    pub read_databases: bool,
    pub create_databases: bool,
    pub read_license: bool,
}

pub async fn overview(
    state: &crate::State,
    server: &Server,
    access: Access,
) -> Result<Frameworks, anyhow::Error> {
    let client = files::client(state, server).await?;
    let (config, unreachable) = match txadmin::config(&client, server).await {
        Ok(config) => (config, false),
        Err(err) => {
            tracing::debug!(server = %server.uuid, "fivem frameworks, txadmin config: {err:#}");
            (None, true)
        }
    };
    let cfg = match txadmin::cfg_path(config.as_ref()) {
        Some(path) => files::read_text(&client, server, &path, 1024 * 1024)
            .await
            .ok()
            .flatten()
            .map(|raw| super::server_cfg::ServerCfg::new(&raw)),
        None => None,
    };
    let game = if cfg
        .as_ref()
        .is_some_and(super::server_cfg::ServerCfg::is_redm)
    {
        Game::Redm
    } else {
        Game::Fivem
    };

    let databases = if access.read_databases {
        ServerDatabase::all_by_server_uuid(&state.database, server.uuid)
            .await?
            .into_iter()
            .filter(|database| database.database_host.r#type == DatabaseType::Mysql)
            .map(|database| DatabaseOption {
                uuid: database.uuid,
                name: database.name.to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(Frameworks {
        game,
        current: txadmin::data_path(config.as_ref()),
        unreachable,
        recipes: for_game(state, game).await,
        install: current(state, server).await,
        databases,
        can_create_database: access.create_databases && can_create_database(state, server).await?,
        license: cfg
            .filter(|_| access.read_license)
            .and_then(|cfg| cfg.get("sv_licenseKey"))
            .filter(|license| valid_license(license)),
    })
}

#[derive(ToSchema, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum DatabaseChoice {
    /// A new database from the server's slots.
    New,
    Existing {
        uuid: uuid::Uuid,
    },
}

#[derive(Debug)]
pub enum FrameworkError {
    RecipeGone,
    AlreadyRunning,
    NeedsDatabase,
    NoDatabaseSlot,
    UnknownDatabase,
    InvalidName,
    InvalidLicense,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for FrameworkError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for FrameworkError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl FrameworkError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let (message, status) = match self {
            Self::RecipeGone => (
                "this framework is no longer in txadmin's list",
                StatusCode::NOT_FOUND,
            ),
            Self::AlreadyRunning => (
                "an install is already running on this server",
                StatusCode::CONFLICT,
            ),
            Self::NeedsDatabase => ("this framework needs a database", StatusCode::BAD_REQUEST),
            Self::NoDatabaseSlot => (
                "this server cannot get another database",
                StatusCode::EXPECTATION_FAILED,
            ),
            Self::UnknownDatabase => (
                "that database is not one of this server's",
                StatusCode::NOT_FOUND,
            ),
            Self::InvalidName => (
                "the server name must be 1 to 64 characters",
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidLicense => (
                "the license key can only use letters, numbers, _ and -",
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => return crate::response::ApiResponse::from(err),
        };

        crate::response::ApiResponse::error(message).with_status(status)
    }
}

pub struct Request {
    pub recipe: String,
    pub server_name: String,
    pub license: String,
    pub database: Option<DatabaseChoice>,
}

/// An install that passed its checks, for [`install`].
pub struct Start {
    pub install: Install,
    recipe: serde_json::Value,
    server_name: String,
    license: String,
    database: Option<DatabaseChoice>,
}

/// Checks the request and stores the install as started; [`install`] then does the work.
pub async fn start(
    state: &crate::State,
    server: &Server,
    request: Request,
    may_create_database: bool,
) -> Result<Start, FrameworkError> {
    let server_name = super::server_cfg::clean(&request.server_name);
    if server_name.is_empty() || server_name.chars().count() > 64 {
        return Err(FrameworkError::InvalidName);
    }
    if !valid_license(request.license.trim()) {
        return Err(FrameworkError::InvalidLicense);
    }
    let (described, body) = find(state, &request.recipe)
        .await
        .ok_or(FrameworkError::RecipeGone)?;

    let database = if described.needs_database {
        match request.database {
            Some(DatabaseChoice::New) => {
                if !may_create_database || !can_create_database(state, server).await? {
                    return Err(FrameworkError::NoDatabaseSlot);
                }
                Some(DatabaseChoice::New)
            }
            Some(DatabaseChoice::Existing { uuid }) => {
                let database =
                    ServerDatabase::by_server_uuid_uuid(&state.database, server.uuid, uuid)
                        .await?
                        .filter(|database| database.database_host.r#type == DatabaseType::Mysql)
                        .ok_or(FrameworkError::UnknownDatabase)?;
                Some(DatabaseChoice::Existing {
                    uuid: database.uuid,
                })
            }
            None => return Err(FrameworkError::NeedsDatabase),
        }
    } else {
        None
    };

    // two clicks, or two tabs, start one install
    let _lock = state
        .cache
        .lock(key(server), Some(10), Some(3000))
        .await
        .map_err(|_| FrameworkError::AlreadyRunning)?;
    if current(state, server)
        .await
        .is_some_and(|install| install.running())
    {
        return Err(FrameworkError::AlreadyRunning);
    }

    let now = chrono::Utc::now();
    let mut install = Install {
        recipe: described.name,
        step: Step::Prepare,
        task: 0,
        total: described.tasks as u32,
        current: None,
        deploy: None,
        previous: None,
        database: None,
        needs_admin: false,
        error: None,
        started: now,
        updated: now,
    };
    store(state, server, &mut install).await;

    Ok(Start {
        install,
        recipe: body,
        server_name,
        license: request.license.trim().to_string(),
        database,
    })
}

/// The folder name txAdmin's deployer would pick: the recipe's name in ASCII letters and digits.
pub fn slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(32)
        .collect();

    if slug.is_empty() {
        "Server".into()
    } else {
        slug
    }
}

/// oxmysql's connection string, raw the way txAdmin writes it: oxmysql does not URL-decode, and
/// generated passwords only use letters and digits.
pub fn connection_string(user: &str, password: &str, host: &str, port: u16, name: &str) -> String {
    format!("mysql://{user}:{password}@{host}:{port}/{name}?charset=utf8mb4")
}

/// Installs a started framework. Takes minutes, so it runs after the request.
pub async fn install(
    state: crate::State,
    mut server: Server,
    start: Start,
    logger: ServerActivityLogger,
) {
    let mut install = start.install.clone();
    let mut created: Option<ServerDatabase> = None;

    let result = run(
        &state,
        &mut server,
        &start,
        &mut install,
        &mut created,
        &logger,
    )
    .await;
    match result {
        Ok(()) => {
            install.step = Step::Done;
            install.current = None;
            logger
                .log(
                    "server:fivem.framework",
                    serde_json::json!({ "recipe": install.recipe, "folder": install.deploy }),
                )
                .await;
        }
        Err(err) => {
            tracing::warn!(server = %server.uuid, "fivem framework install failed: {err:#}");
            if let Some(deploy) = &install.deploy {
                match recipe::Files::new(&state, &server).await {
                    Ok(mut files) => {
                        if let Err(err) = files.delete(deploy).await {
                            tracing::warn!(server = %server.uuid, "removing {deploy}: {err:#}");
                        }
                    }
                    Err(err) => tracing::warn!(server = %server.uuid, "removing {deploy}: {err:#}"),
                }
            }
            if let Some(database) = created {
                if let Err(err) = database
                    .delete(&state, DeleteServerDatabaseOptions { force: true })
                    .await
                {
                    tracing::warn!(server = %server.uuid, "removing database {}: {err:#}", database.name);
                }
                install.database = None;
            }
            install.step = Step::Failed;
            install.error = Some(format!("{err:#}").chars().take(500).collect());
        }
    }
    store(&state, &server, &mut install).await;
    crate::notifications::framework_ended(&state, logger.user_uuid, &server, &install).await;
}

async fn create_database(
    state: &crate::State,
    server: &Server,
    slug: &str,
    logger: &ServerActivityLogger,
) -> Result<ServerDatabase, anyhow::Error> {
    let _lock = state
        .cache
        .lock(
            format!("servers::{}::databases", server.uuid),
            Some(30),
            Some(5000),
        )
        .await?;
    if ServerDatabase::count_by_server_uuid(&state.database, server.uuid).await?
        >= server.database_limit as i64
    {
        anyhow::bail!("this server has no database slot left");
    }
    let host = database_host(state, server)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no database host takes new databases"))?;

    // the suffix keeps a second install of the same framework off the first one's database
    let name = format!(
        "{}_{}",
        slug.to_lowercase().chars().take(24).collect::<String>(),
        rand::distr::Alphanumeric
            .sample_string(&mut rand::rng(), 4)
            .to_lowercase()
    );
    let database = ServerDatabase::create(
        state,
        CreateServerDatabaseOptions {
            server,
            database_host: &host,
            name: name.into(),
        },
    )
    .await?;
    logger
        .log(
            "server:database.create",
            serde_json::json!({ "uuid": database.uuid, "name": database.name }),
        )
        .await;

    Ok(database)
}

/// The variables txAdmin's deployer fills in a recipe and its server.cfg.
async fn variables(
    state: &crate::State,
    server: &Server,
    client: &wings_api::client::WingsClient,
    body: &serde_json::Value,
    start: &Start,
) -> Result<BTreeMap<String, String>, anyhow::Error> {
    let port = server
        .allocation
        .as_ref()
        .map_or(30120, |allocation| allocation.allocation.port);
    // a marker for people reading server.cfg; one line, whatever the name
    let brand = state
        .settings
        .get()
        .await?
        .app
        .name
        .replace(['\r', '\n'], " ");
    let brand = if brand.trim().is_empty() {
        "panel".into()
    } else {
        brand.trim().to_string()
    };
    let endpoint = format!("\"0.0.0.0:{port}\" # {brand}: do not modify!");
    let principals =
        txadmin::master_principals(&txadmin::admins(client, server).await.unwrap_or_default());

    Ok(BTreeMap::from([
        (
            "recipeName".into(),
            scalar(body.get("name")).unwrap_or_default(),
        ),
        (
            "recipeAuthor".into(),
            scalar(body.get("author")).unwrap_or_default(),
        ),
        (
            "recipeDescription".into(),
            scalar(body.get("description")).unwrap_or_default(),
        ),
        ("serverName".into(), start.server_name.clone()),
        ("maxClients".into(), "48".into()),
        (
            "serverEndpoints".into(),
            format!("endpoint_add_tcp {endpoint}\nendpoint_add_udp {endpoint}"),
        ),
        (
            "addPrincipalsMaster".into(),
            if principals.is_empty() {
                "# add_principal identifier.fivem:YOUR_ID group.admin # your FiveM account, see Server Configuration › Group Memberships".into()
            } else {
                principals.join("\n")
            },
        ),
        ("svLicense".into(), start.license.clone()),
        ("dbConnectionString".into(), String::new()),
    ]))
}

async fn wait_for(
    client: &wings_api::client::WingsClient,
    server: &Server,
    running: bool,
    seconds: u64,
) -> bool {
    let until = tokio::time::Instant::now() + std::time::Duration::from_secs(seconds);
    while tokio::time::Instant::now() < until {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let state = files::power_state(client, server).await;
        let is_running = matches!(
            state,
            Some(
                wings_api::ServerState::Running
                    | wings_api::ServerState::Starting
                    | wings_api::ServerState::Stopping
            )
        );
        if is_running == running {
            return true;
        }
    }

    false
}

async fn power(
    client: &wings_api::client::WingsClient,
    server: &Server,
    action: wings_api::ServerPowerAction,
) -> Result<(), anyhow::Error> {
    client
        .post_servers_server_power(
            server.uuid,
            &wings_api::servers_server_power::post::RequestBody {
                action,
                wait_seconds: None,
            },
        )
        .await
        .map_err(|err| anyhow::anyhow!("the node refused the power action: {err:?}"))?;

    Ok(())
}

async fn run(
    state: &crate::State,
    server: &mut Server,
    start: &Start,
    install: &mut Install,
    created: &mut Option<ServerDatabase>,
    logger: &ServerActivityLogger,
) -> Result<(), anyhow::Error> {
    let tasks: Vec<recipe::Task> = start
        .recipe
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let onesync = start
        .recipe
        .get("$onesync")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let slug = slug(&install.recipe);

    let snapshot = server.clone();
    let mut files = recipe::Files::new(state, &snapshot).await?;

    // 1. the deploy folder, the database, the variables
    let mut deploy = None;
    for _ in 0..5 {
        let candidate = format!(
            "txData/{slug}_{}.base",
            rand::distr::Alphanumeric
                .sample_string(&mut rand::rng(), 6)
                .to_uppercase()
        );
        if !files.exists(&candidate).await? {
            deploy = Some(candidate);
            break;
        }
    }
    let deploy = deploy.ok_or_else(|| anyhow::anyhow!("no free folder name in txData"))?;
    files.make_dir(&deploy).await?;
    install.deploy = Some(deploy.clone());
    store(state, server, install).await;

    let database = match &start.database {
        Some(DatabaseChoice::New) => {
            let database = create_database(state, server, &slug, logger).await?;
            install.database = Some(database.name.to_string());
            store(state, server, install).await;
            *created = Some(database.clone());
            Some(database)
        }
        Some(DatabaseChoice::Existing { uuid }) => Some(
            ServerDatabase::by_server_uuid_uuid(&state.database, server.uuid, *uuid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("the database is gone"))?,
        ),
        None => None,
    };

    let mut vars = variables(state, server, files.client(), &start.recipe, start).await?;
    let login = match &database {
        Some(database) => {
            let details = database
                .database_host
                .credentials
                .parse_connection_details(&state.database)
                .await?;
            let password = database
                .password
                .decrypt(&state.database)
                .await?
                .to_string();
            // FXServer connects from the node, through the address players' servers are given
            let public_host = database
                .database_host
                .public_host
                .clone()
                .map_or_else(|| details.host.to_string(), |host| host.to_string());
            let public_port = database
                .database_host
                .public_port
                .and_then(|port| u16::try_from(port).ok())
                .unwrap_or(details.port);
            vars.extend([
                ("dbHost".to_string(), public_host.clone()),
                ("dbPort".into(), public_port.to_string()),
                ("dbUsername".into(), database.username.to_string()),
                ("dbPassword".into(), password.clone()),
                ("dbName".into(), database.name.to_string()),
                ("dbDelete".into(), "false".into()),
                ("dbConnected".into(), "true".into()),
                (
                    "dbConnectionString".into(),
                    connection_string(
                        &database.username,
                        &password,
                        &public_host,
                        public_port,
                        &database.name,
                    ),
                ),
            ]);

            Some(recipe::DatabaseLogin {
                host: details.host.to_string(),
                port: details.port,
                user: database.username.to_string(),
                password,
                name: database.name.to_string(),
            })
        }
        None => None,
    };

    // 2. the recipe, while the server keeps running
    install.step = Step::Tasks;
    let mut runner = recipe::Runner::new(files, deploy.clone(), vars, login);
    for (index, task) in tasks.iter().enumerate() {
        install.task = index as u32 + 1;
        install.current = Some(recipe::describe(task));
        store(state, server, install).await;
        runner.run(task, index).await?;
    }

    // 3. server.cfg's variables
    install.step = Step::Config;
    install.current = None;
    store(state, server, install).await;
    let cfg = format!("{deploy}/server.cfg");
    if runner.files.exists(&cfg).await? {
        let raw = runner.files.read(&cfg, 1024 * 1024).await?;
        let filled = recipe::fill(&raw, &runner.vars);
        runner.files.write(&cfg, filled).await?;
    }
    runner
        .files
        .delete(&format!("{deploy}/{}", recipe::SCRATCH))
        .await?;

    // 4. txAdmin on the new folder; read before stopping: it names the previous folder
    let client = runner.files.client();
    let config = txadmin::config(client, server).await?;
    install.step = Step::Restart;
    install.previous = txadmin::data_path(config.as_ref());
    install.needs_admin = txadmin::admins(client, server)
        .await
        .is_ok_and(|admins| admins.as_array().is_none_or(Vec::is_empty));
    store(state, server, install).await;

    if !matches!(
        files::power_state(client, server).await,
        Some(wings_api::ServerState::Offline) | None
    ) {
        power(client, server, wings_api::ServerPowerAction::Stop).await?;
        if !wait_for(client, server, false, 60).await {
            power(client, server, wings_api::ServerPowerAction::Kill).await?;
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    let pointed = txadmin::pointed_at(config, &deploy, onesync.as_deref());
    files::write_text(
        client,
        server,
        txadmin::CONFIG,
        format!("{}\n", serde_json::to_string_pretty(&pointed)?),
    )
    .await?;
    super::runtime::adopt(state, server).await?;

    // Wings runs power actions in the background and drops a start that arrives while the stop
    // still holds its lock, so a start is checked and repeated
    for _ in 0..4 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        power(client, server, wings_api::ServerPowerAction::Start)
            .await
            .ok();
        if wait_for(client, server, true, 10).await {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_recipe_are_read() {
        let index = serde_json::json!([
            {"name": "ESX", "url": "https://raw.githubusercontent.com/esx-framework/txAdmin-recipe/main/esxlegacy.yaml", "engine": 3, "tags": ["fivem"]},
            {"name": "Old", "url": "https://example.com/old.yaml", "engine": 2},
            {"name": "Plain", "url": "http://example.com/x.yaml", "engine": 3},
            "junk"
        ]);
        let entries = entries(&index);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            logo(entries[0]["url"].as_str().unwrap()).as_deref(),
            Some("https://github.com/esx-framework.png?size=96")
        );
        assert_eq!(id("a").len(), 16);

        let recipe = parse_recipe(
            "$engine: 3\nname: ESX Legacy\nversion: 1.10\nauthor: ESX\ntasks:\n  - action: download_github\n    src: https://github.com/esx-framework/esx_core\n  - action: query_database\n    file: ./x.sql\n",
        )
        .unwrap();
        let described = describe(&entries[0], &recipe);
        assert_eq!(described.name, "ESX Legacy");
        assert_eq!(described.version.as_deref(), Some("1.1"));
        assert!(described.needs_database);
        assert_eq!(described.tasks, 2);
        assert!(parse_recipe("name: no tasks").is_none());
    }

    #[test]
    fn names_and_connection() {
        assert_eq!(slug("ESX Legacy (v1.10)!"), "ESXLegacyv110");
        assert_eq!(slug("ÄÖÜ"), "Server");
        assert!(valid_license("cfxk_abc-DEF_1") && !valid_license("a b") && !valid_license(""));
        assert_eq!(
            connection_string("u", "p", "db.example.com", 3306, "s1_esx"),
            "mysql://u:p@db.example.com:3306/s1_esx?charset=utf8mb4"
        );
    }
}
