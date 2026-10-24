//! plugged.host: devboxes (docs/features/devboxes.md): a private Linux box next to a game server,
//! with the game server's files at `/server`, that VS Code connects to over SSH or opens in the
//! browser.
//!
//! A devbox is a server Wings runs on its game server's node: the devbox egg, owned by the
//! panel's system account (nobody signs in as it), its own allocation as the SSH port, its own
//! volume as the home, and the game server's volume mounted at `/server`. Inside, `ph serve`
//! (`docker/devbox/ph`) is the SSH server; it asks the panel (`/api/devbox`) who may sign in, with
//! the bearer token of its current start.
//!
//! Who may have which devbox:
//! - A member of a game server is its owner, or a subuser the owner gave `devbox.connect` and
//!   every file right SFTP needs ([`SUBUSER_PERMISSIONS`]): a devbox reads, writes and runs the
//!   server's files, so it may not open more than the owner handed out. Checked against the
//!   subuser row, never an admin's or a role's rights, which must not open devboxes on other
//!   people's servers.
//! - A personal devbox needs the person's own membership (or, with Members Only off, nothing);
//!   the tier is theirs.
//! - A shared devbox (one per game server, switched on by the owner) goes by the owner's
//!   membership; while it is on, the server's members use it instead of personal ones.
//! - Whoever pays (the person, or the owner for a shared one) has one running cap for both kinds,
//!   and nobody with a critical abuse finding on a devbox gets one ([`blocks`]).

pub mod agent;
pub mod blocks;
pub mod codes;
pub mod dns;
pub mod pki;
pub mod power;
pub mod provision;
pub mod reconcile;
pub mod view;
pub mod web;

use crate::{
    models::{ByUuid, server::Server},
    settings::{AppSettings, abuse::AbuseMode, devboxes::AppSettingsDevboxes},
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::Row;
use utoipa::ToSchema;

/// The server permission that opens devboxes to a subuser.
pub const PERMISSION: &str = "devbox.connect";
/// What a subuser needs on the game server for a devbox: [`PERMISSION`] and SFTP with every file
/// right.
pub const SUBUSER_PERMISSIONS: &[&str] = &[
    PERMISSION,
    "files.sftp",
    "files.read",
    "files.read-content",
    "files.create",
    "files.update",
    "files.delete",
];
/// A node over this on memory, CPU or disk (latest reading) takes no devbox.
const OVERLOADED_PERCENT: f64 = 90.0;
/// A start that did not report back within this long failed.
pub const BOOT_MINUTES: i64 = 10;

/// What `ph` may do on the game server for everyone in a shared devbox (its token is readable by
/// all of them, so it cannot be more than what they all may do).
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedRights {
    #[serde(default = "yes")]
    pub status: bool,
    #[serde(default = "yes")]
    pub logs: bool,
    #[serde(default)]
    pub power: bool,
    #[serde(default)]
    pub console: bool,
}

fn yes() -> bool {
    true
}

impl Default for SharedRights {
    fn default() -> Self {
        Self {
            status: true,
            logs: true,
            power: false,
            console: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Devbox {
    pub uuid: uuid::Uuid,
    pub game_server_uuid: uuid::Uuid,
    /// `None`: the game server's shared devbox.
    pub user_uuid: Option<uuid::Uuid>,
    /// The server Wings runs for it.
    pub server_uuid: Option<uuid::Uuid>,
    pub node_uuid: Option<uuid::Uuid>,
    pub mount_source: Option<String>,
    pub slug: String,
    pub token_hash: Option<String>,
    pub code_secret: Option<String>,
    pub stopping: Option<chrono::NaiveDateTime>,
    pub boot_status: Option<String>,
    pub boot_detail: Option<String>,
    pub host_key: Option<String>,
    pub version: Option<String>,
    pub connections: i32,
    pub shared_rights: SharedRights,
    /// Who could sign in at the last start.
    pub people: Vec<uuid::Uuid>,
    pub started: Option<chrono::NaiveDateTime>,
    pub booted: Option<chrono::NaiveDateTime>,
    pub last_seen: Option<chrono::NaiveDateTime>,
    pub last_used: Option<chrono::NaiveDateTime>,
    pub created: chrono::NaiveDateTime,
}

const SELECT: &str = "SELECT uuid, game_server_uuid, user_uuid, server_uuid, node_uuid,
        mount_source, slug, token_hash, code_secret, stopping, boot_status, boot_detail, host_key,
        version, connections, shared_rights, people, started, booted, last_seen, last_used,
        created
    FROM devboxes";

impl Devbox {
    fn map(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let rights: Option<serde_json::Value> = row.try_get("shared_rights")?;

        Ok(Self {
            uuid: row.try_get("uuid")?,
            game_server_uuid: row.try_get("game_server_uuid")?,
            user_uuid: row.try_get("user_uuid")?,
            server_uuid: row.try_get("server_uuid")?,
            node_uuid: row.try_get("node_uuid")?,
            mount_source: row.try_get("mount_source")?,
            slug: row.try_get("slug")?,
            token_hash: row.try_get("token_hash")?,
            code_secret: row.try_get("code_secret")?,
            stopping: row.try_get("stopping")?,
            boot_status: row.try_get("boot_status")?,
            boot_detail: row.try_get("boot_detail")?,
            host_key: row.try_get("host_key")?,
            version: row.try_get("version")?,
            connections: row.try_get("connections")?,
            shared_rights: rights
                .and_then(|rights| serde_json::from_value(rights).ok())
                .unwrap_or_default(),
            people: row
                .try_get::<Option<Vec<uuid::Uuid>>, _>("people")?
                .unwrap_or_default(),
            started: row.try_get("started")?,
            booted: row.try_get("booted")?,
            last_seen: row.try_get("last_seen")?,
            last_used: row.try_get("last_used")?,
            created: row.try_get("created")?,
        })
    }

    async fn one(
        database: &crate::database::Database,
        filter: &str,
        value: impl for<'q> sqlx::Encode<'q, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!("{SELECT} WHERE {filter}")))
            .bind(value)
            .fetch_optional(database.write())
            .await?;

        row.as_ref().map(Self::map).transpose()
    }

    pub async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        Self::one(database, "uuid = $1", uuid).await
    }

    /// The running devbox a bearer token belongs to. One that is stopping keeps its token until
    /// Wings confirms the stop, but is no longer answered for: its `ph` gets a 401 and ends every
    /// session.
    pub async fn by_token(
        database: &crate::database::Database,
        token: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        if token.len() < 32 {
            return Ok(None);
        }

        Self::one(
            database,
            "token_hash = $1 AND stopping IS NULL",
            hash_token(token),
        )
        .await
    }

    pub async fn by_slug(
        database: &crate::database::Database,
        slug: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        Self::one(database, "slug = $1", slug.to_string()).await
    }

    pub async fn by_server_uuid(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        Self::one(database, "server_uuid = $1", server_uuid).await
    }

    pub async fn shared_of(
        database: &crate::database::Database,
        game_server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        Self::one(
            database,
            "game_server_uuid = $1 AND user_uuid IS NULL",
            game_server_uuid,
        )
        .await
    }

    pub async fn personal(
        database: &crate::database::Database,
        game_server_uuid: uuid::Uuid,
        user_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE game_server_uuid = $1 AND user_uuid = $2"
        )))
        .bind(game_server_uuid)
        .bind(user_uuid)
        .fetch_optional(database.write())
        .await?;

        row.as_ref().map(Self::map).transpose()
    }

    pub async fn all(database: &crate::database::Database) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!("{SELECT} ORDER BY created")))
            .fetch_all(database.write())
            .await?;

        rows.iter().map(Self::map).collect()
    }

    /// Every devbox of a game server.
    pub async fn all_of_game(
        database: &crate::database::Database,
        game_server_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE game_server_uuid = $1"
        )))
        .bind(game_server_uuid)
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::map).collect()
    }

    /// The devboxes `user_uuid` pays for that hold a token (running or still stopping): their
    /// personal ones, and the shared ones on servers they own.
    pub async fn running_paid_by(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE token_hash IS NOT NULL AND (user_uuid = $1 OR (user_uuid IS NULL
                AND game_server_uuid IN (SELECT uuid FROM servers WHERE owner_uuid = $1)))"
        )))
        .bind(user_uuid)
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::map).collect()
    }

    pub async fn refresh(
        &mut self,
        database: &crate::database::Database,
    ) -> Result<(), sqlx::Error> {
        if let Some(fresh) = Self::by_uuid(database, self.uuid).await? {
            *self = fresh;
        }

        Ok(())
    }

    pub fn is_shared(&self) -> bool {
        self.user_uuid.is_none()
    }

    /// Started and not on its way down: its token is good and people may sign in.
    pub fn is_running(&self) -> bool {
        self.token_hash.is_some() && self.stopping.is_none()
    }

    /// Asked to stop; it keeps its token (and counts against the running cap) until Wings says
    /// it is offline.
    pub fn is_stopping(&self) -> bool {
        self.token_hash.is_some() && self.stopping.is_some()
    }

    /// Who pays for it (their membership, their running cap): its person, or the game server's
    /// owner for a shared one.
    pub fn payer(&self, game: &Server) -> uuid::Uuid {
        self.user_uuid.unwrap_or(game.owner.uuid)
    }

    /// `https://<slug>.<domain>`'s host, for the browser version.
    pub fn web_host(&self, settings: &AppSettingsDevboxes) -> String {
        format!("{}.{}", self.slug, settings.web_domain)
    }
}

pub fn hash_token(token: &str) -> String {
    hex::encode(sha2::Sha256::digest(token.as_bytes()))
}

/// Why something cannot be done with a devbox right now.
#[derive(Debug)]
pub enum DevboxError {
    NotAllowed,
    NotSetUp,
    Missing,
    TooMany(i32),
    Suspended,
    Maintenance,
    Installing,
    Busy,
    Stopping,
    NodeBusy,
    NodeDown,
    /// The node's Wings does not allow mounting its data directory.
    NodeNotReady,
    DiskFull,
    Abuse,
    /// No free allocation on the node.
    NoPort,
    Other(anyhow::Error),
}

impl From<sqlx::Error> for DevboxError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<anyhow::Error> for DevboxError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for DevboxError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl DevboxError {
    pub fn message(&self) -> String {
        match self {
            Self::NotAllowed => "you can't open a devbox on this server".into(),
            Self::NotSetUp => "devboxes aren't set up on this panel".into(),
            Self::Missing => "the devbox could not be found, try again".into(),
            Self::TooMany(cap) => format!(
                "you already have {cap} devboxes running, which is the most at once: stop one first"
            ),
            Self::Suspended => "the server is suspended".into(),
            Self::Maintenance => "the server's node is under maintenance".into(),
            Self::Installing => "the server is still installing".into(),
            Self::Busy => "the server is busy (restoring a backup, moving or parked)".into(),
            Self::Stopping => "the devbox is still stopping, try again in a moment".into(),
            Self::NodeBusy => "the server's node is too busy for a devbox right now".into(),
            Self::NodeDown => "the server's node isn't answering right now".into(),
            Self::NodeNotReady => "the server's node isn't set up for devboxes yet".into(),
            Self::DiskFull => "the server's disk is full: free some space first".into(),
            Self::Abuse => "devboxes are paused here while an abuse report is looked into".into(),
            Self::NoPort => "the server's node has no free port for a devbox".into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotAllowed => 403,
            Self::Missing => 404,
            Self::NodeDown => 502,
            Self::Other(_) => 500,
            _ => 409,
        }
    }

    pub fn response(self) -> crate::response::ApiResponse {
        let status = axum::http::StatusCode::from_u16(self.status())
            .unwrap_or(axum::http::StatusCode::CONFLICT);

        match self {
            Self::Other(err) => crate::response::ApiResponse::from(err),
            err => crate::response::ApiResponse::error(&err.message()).with_status(status),
        }
    }
}

/// Whether players are offered devboxes: switched on with an egg, and abuse protection watching
/// (a devbox nobody checks is a free Linux box for miners).
pub fn offered(settings: &AppSettings) -> bool {
    settings.devboxes.active() && settings.abuse.mode != AbuseMode::Off
}

/// The settings a devbox decision needs, copied out of the settings guard.
#[derive(Clone)]
pub struct Context {
    pub devboxes: AppSettingsDevboxes,
    pub membership_enabled: bool,
    pub abuse_on: bool,
    pub app_name: String,
    pub app_url: String,
    /// The Cloudflare token for the browser version's records: its own, or the subdomains one.
    pub cloudflare_token: Option<String>,
}

impl Context {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            devboxes: settings.devboxes.clone(),
            membership_enabled: settings.economy.membership_enabled,
            abuse_on: settings.abuse.mode != AbuseMode::Off,
            app_name: settings.app.name.to_string(),
            app_url: settings.app.url.trim_end_matches('/').to_string(),
            cloudflare_token: settings
                .devboxes
                .cloudflare_token
                .as_deref()
                .or(settings.subdomains.cloudflare_token.as_deref())
                .map(str::to_string),
        }
    }

    pub async fn load(state: &crate::State) -> Result<Self, anyhow::Error> {
        let settings = state.settings.get().await?;

        Ok(Self::from_settings(&settings))
    }

    /// Switched on with an egg, and abuse protection watching: a devbox nobody checks is a free
    /// Linux box for miners.
    pub fn ready(&self) -> bool {
        self.devboxes.active() && self.abuse_on
    }

    /// Where a devbox reaches the panel.
    pub fn panel_url(&self) -> String {
        if self.devboxes.panel_url.is_empty() {
            self.app_url.clone()
        } else {
            self.devboxes.panel_url.to_string()
        }
    }

    pub fn web_enabled(&self) -> bool {
        self.devboxes.web_ready()
    }
}

/// The resources a devbox gets, by whoever pays for it.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Standard,
    Premium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub cpu: i32,
    pub memory: i32,
    pub disk: i32,
}

impl Tier {
    pub fn limits(self, settings: &AppSettingsDevboxes) -> Limits {
        match self {
            Self::Standard => Limits {
                cpu: settings.cpu,
                memory: settings.memory,
                disk: settings.disk,
            },
            Self::Premium => Limits {
                cpu: settings.premium_cpu,
                memory: settings.premium_memory,
                disk: settings.premium_disk,
            },
        }
    }
}

/// The tier a devbox paid by `user_uuid` gets, `None` when they may not have one: the second
/// membership is Premium, the first (or anyone, with Members Only off) Standard.
pub async fn tier(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
) -> Result<Option<Tier>, sqlx::Error> {
    let members_only = context.devboxes.members_only;
    if !context.membership_enabled {
        return Ok((!members_only).then_some(Tier::Standard));
    }

    let membership = crate::economy::membership(&state.database, user_uuid).await?;
    let now = chrono::Utc::now().naive_utc();

    Ok(tier_of(&membership, now, members_only))
}

fn tier_of(
    membership: &crate::economy::Membership,
    now: chrono::NaiveDateTime,
    members_only: bool,
) -> Option<Tier> {
    if membership.has_plusplus(now) {
        Some(Tier::Premium)
    } else if membership.has_plus() || !members_only {
        Some(Tier::Standard)
    } else {
        None
    }
}

/// Whether a subuser's permission list holds everything a devbox needs.
pub fn grants(permissions: &[compact_str::CompactString]) -> bool {
    SUBUSER_PERMISSIONS
        .iter()
        .all(|needed| permissions.iter().any(|permission| permission == needed))
}

/// The subuser permissions of `user_uuid` on a server, `None` when they are not a subuser.
pub async fn subuser_permissions(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
) -> Result<Option<Vec<compact_str::CompactString>>, sqlx::Error> {
    let permissions: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT permissions FROM server_subusers WHERE server_uuid = $1 AND user_uuid = $2",
    )
    .bind(server_uuid)
    .bind(user_uuid)
    .fetch_optional(database.write())
    .await?;

    Ok(permissions.map(|permissions| permissions.into_iter().map(Into::into).collect()))
}

/// The owner, or a subuser whose row grants everything a devbox needs.
pub async fn is_member(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
    game: &Server,
) -> Result<bool, sqlx::Error> {
    if game.owner.uuid == user_uuid {
        return Ok(true);
    }

    Ok(subuser_permissions(database, game.uuid, user_uuid)
        .await?
        .is_some_and(|permissions| grants(&permissions)))
}

/// Whether `user_uuid` holds `permission` on the game server themselves: the owner holds all, a
/// subuser what their row says. Never an admin's override.
pub async fn holds(
    database: &crate::database::Database,
    game: &Server,
    user_uuid: uuid::Uuid,
    permission: &str,
) -> Result<bool, sqlx::Error> {
    if game.owner.uuid == user_uuid {
        return Ok(true);
    }

    Ok(subuser_permissions(database, game.uuid, user_uuid)
        .await?
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|held| held == permission || held == "*")
        }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Personal,
    Shared,
}

/// The devbox `user_uuid` works in on `game`, or the one they would get: its kind, and the row
/// when it exists. `None` when they may not have one at all.
pub async fn resolve(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
    game: &Server,
) -> Result<Option<(Kind, Option<Devbox>)>, sqlx::Error> {
    if !context.ready() || !is_member(&state.database, user_uuid, game).await? {
        return Ok(None);
    }

    if let Some(shared) = Devbox::shared_of(&state.database, game.uuid).await? {
        return Ok(tier(state, context, game.owner.uuid)
            .await?
            .map(|_| (Kind::Shared, Some(shared))));
    }
    if tier(state, context, user_uuid).await?.is_none() {
        return Ok(None);
    }

    Ok(Some((
        Kind::Personal,
        Devbox::personal(&state.database, game.uuid, user_uuid).await?,
    )))
}

/// Someone who may sign in to a devbox.
#[derive(Clone, Debug)]
pub struct Person {
    pub uuid: uuid::Uuid,
    pub username: String,
}

/// Everyone who may sign in to `devbox` right now.
pub async fn people(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    game: &Server,
) -> Result<Vec<Person>, anyhow::Error> {
    let payer = devbox.payer(game);
    if !context.ready()
        || tier(state, context, payer).await?.is_none()
        || blocks::blocked(state, payer).await?
    {
        return Ok(Vec::new());
    }

    if let Some(user_uuid) = devbox.user_uuid {
        if !is_member(&state.database, user_uuid, game).await? {
            return Ok(Vec::new());
        }
        let username: Option<String> =
            sqlx::query_scalar("SELECT username FROM users WHERE uuid = $1")
                .bind(user_uuid)
                .fetch_optional(state.database.read())
                .await?;

        return Ok(username
            .map(|username| {
                vec![Person {
                    uuid: user_uuid,
                    username,
                }]
            })
            .unwrap_or_default());
    }

    // the owner pays: an owner with a live abuse finding takes the shared devbox down with them
    let mut people = vec![Person {
        uuid: game.owner.uuid,
        username: game.owner.username.to_string(),
    }];
    let rows = sqlx::query(
        "SELECT users.uuid, users.username, server_subusers.permissions
        FROM server_subusers
        JOIN users ON users.uuid = server_subusers.user_uuid
        WHERE server_subusers.server_uuid = $1",
    )
    .bind(game.uuid)
    .fetch_all(state.database.read())
    .await?;
    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let permissions: Vec<String> = row.try_get("permissions")?;
        let permissions: Vec<compact_str::CompactString> =
            permissions.into_iter().map(Into::into).collect();
        if uuid == game.owner.uuid || !grants(&permissions) || blocks::blocked(state, uuid).await? {
            continue;
        }
        people.push(Person {
            uuid,
            username: row.try_get("username")?,
        });
    }

    Ok(people)
}

/// Why the game server can't have its devbox started right now.
pub async fn blocked_reason(
    state: &crate::State,
    game: &Server,
) -> Result<Option<DevboxError>, anyhow::Error> {
    if game.suspended {
        return Ok(Some(DevboxError::Suspended));
    }
    let node = game.node.fetch_cached(&state.database).await?;
    if node.maintenance_enabled {
        return Ok(Some(DevboxError::Maintenance));
    }

    Ok(match game.status {
        Some(crate::models::server::ServerStatus::Installing) => Some(DevboxError::Installing),
        Some(_) => Some(DevboxError::Busy),
        None if game.destination_node.is_some()
            || game
                .limbo
                .is_some_and(|limbo| limbo != crate::limbo::LimboState::Stopped) =>
        {
            Some(DevboxError::Busy)
        }
        None => None,
    })
}

/// Why `actor` can't create or start a devbox on `game` (paid by `payer`) right now: the game
/// server's state, an abuse block on either of them, or a node that takes nothing new.
pub async fn refusal(
    state: &crate::State,
    actor: uuid::Uuid,
    game: &Server,
    payer: uuid::Uuid,
) -> Result<Option<DevboxError>, anyhow::Error> {
    if let Some(reason) = blocked_reason(state, game).await? {
        return Ok(Some(reason));
    }
    if blocks::blocked(state, actor).await?
        || (payer != actor && blocks::blocked(state, payer).await?)
    {
        return Ok(Some(DevboxError::Abuse));
    }

    node_refusal(state, game.node.uuid).await
}

/// Whether a node may take a devbox (gigabytes of memory, CPU and disk on top of what runs
/// there): not while its deployment is paused or its latest reading is over the line, and not
/// while that reading got no answer. With no reading at all it is let through, like the other
/// node checks.
pub async fn node_refusal(
    state: &crate::State,
    node_uuid: uuid::Uuid,
) -> Result<Option<DevboxError>, anyhow::Error> {
    let Some(node) =
        crate::models::node::Node::by_uuid_optional(&state.database, node_uuid).await?
    else {
        return Ok(Some(DevboxError::Missing));
    };
    if node.deployment_paused {
        return Ok(Some(DevboxError::NodeBusy));
    }

    let Some(sample) = crate::node_telemetry::latest(&state.database, node.uuid).await? else {
        return Ok(None);
    };
    if !sample.reachable {
        return Ok(Some(DevboxError::NodeDown));
    }

    Ok(overloaded(&sample).then_some(DevboxError::NodeBusy))
}

fn overloaded(sample: &crate::node_telemetry::AdminApiNodeTelemetry) -> bool {
    let percent = |used: Option<i64>, total: Option<i64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f64 / total as f64 * 100.0),
        _ => None,
    };

    [
        sample.cpu_percent.map(f64::from),
        percent(sample.memory_used, sample.memory_total),
        percent(sample.disk_used, sample.disk_total),
    ]
    .into_iter()
    .flatten()
    .any(|value| value > OVERLOADED_PERCENT)
}

/// Logs an activity on the game server, `user_uuid` as who did it (which also keeps limbo from
/// parking a server someone works on).
pub async fn log_game(
    state: &crate::State,
    game_server_uuid: uuid::Uuid,
    user_uuid: Option<uuid::Uuid>,
    event: &'static str,
    data: serde_json::Value,
) {
    use crate::models::CreatableModel;

    if let Err(err) = crate::models::server_activity::ServerActivity::create(
        state,
        crate::models::server_activity::CreateServerActivityOptions {
            server_uuid: game_server_uuid,
            user_uuid,
            impersonator_uuid: None,
            api_key_uuid: None,
            schedule_uuid: None,
            event: event.into(),
            ip: None,
            data,
            created: None,
        },
    )
    .await
    {
        tracing::warn!(server = %game_server_uuid, "failed to log {event}: {err:#?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subusers_need_every_file_right() {
        let all: Vec<compact_str::CompactString> =
            SUBUSER_PERMISSIONS.iter().map(|p| (*p).into()).collect();
        assert!(grants(&all));

        let without_delete: Vec<compact_str::CompactString> = all
            .iter()
            .filter(|p| *p != "files.delete")
            .cloned()
            .collect();
        assert!(!grants(&without_delete));
    }

    #[test]
    fn tiers_follow_the_membership() {
        let now = chrono::Utc::now().naive_utc();
        let none = crate::economy::Membership::default();
        let plus = crate::economy::Membership {
            plus_since: Some(now),
            ..Default::default()
        };
        let plusplus = crate::economy::Membership {
            plus_since: Some(now),
            plusplus_until: Some(now + chrono::Duration::days(3)),
            plusplus_purchases: 1,
        };

        assert_eq!(tier_of(&none, now, true), None);
        assert_eq!(tier_of(&none, now, false), Some(Tier::Standard));
        assert_eq!(tier_of(&plus, now, true), Some(Tier::Standard));
        assert_eq!(tier_of(&plusplus, now, true), Some(Tier::Premium));
    }

    #[test]
    fn shared_rights_default_to_status_and_logs() {
        let rights: SharedRights = serde_json::from_value(serde_json::json!({})).unwrap();

        assert_eq!(rights, SharedRights::default());
        assert!(rights.status && rights.logs && !rights.power && !rights.console);
    }

    #[test]
    fn a_node_over_the_line_is_busy() {
        let mut sample = crate::node_telemetry::AdminApiNodeTelemetry {
            reachable: true,
            refused: false,
            response_time_ms: None,
            cpu_percent: Some(40.0),
            cpu_threads: None,
            memory_used: Some(50),
            memory_total: Some(100),
            disk_used: Some(10),
            disk_total: Some(100),
            load1: None,
            servers_running: None,
            pressure: None,
            consecutive_failures: 0,
            consecutive_outages: 0,
            error: None,
            last_reachable: None,
            sampled: chrono::Utc::now(),
        };
        assert!(!overloaded(&sample));

        sample.memory_used = Some(95);
        assert!(overloaded(&sample));
    }
}
