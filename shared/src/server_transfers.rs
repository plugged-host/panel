//! plugged.host: moving a server to another node, asked for by its owner on the server's Transfer
//! page or by an admin's "Lighten load" on a node (docs/features/transfers.md).
//!
//! The copy is the native transfer (`Server::transfer`): Wings stops the server, streams it and its
//! node-local backups to the new node, and the panel moves the row once the new node confirms.
//! Around it this adds the choice of node and ports, a cooldown, retries on another node of the
//! location, a cancel, queued moves for "Lighten load", and starting the server again when it was
//! running. Each move is a row in `server_transfer_requests`; [`settle`] (the `settle_transfers`
//! task) moves it along once the native transfer is over.

use crate::{
    models::{
        ByUuid,
        egg_configuration::{EggConfiguration, EggConfigurationTarget},
        location::Location,
        nest_egg_variable::NestEggVariable,
        node::Node,
        node_allocation::NodeAllocation,
        server::{AdminApiServerLimits, Server, ServerTransferOptions},
        server_allocation::ServerAllocation,
        server_backup::ServerBackup,
        server_variable::ServerVariable,
        user::User,
    },
    server_creation::{ApiCreateLocation, GAME_ROLE, RANDOMNESS, SUSPENSION_PENALTY},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

/// Waits for its turn ("Lighten load" moves a few at a time).
pub const QUEUED: &str = "queued";
pub const RUNNING: &str = "running";
/// Cancelled while Wings was copying; settles once Wings has stopped.
pub const CANCELLING: &str = "cancelling";
pub const COMPLETED: &str = "completed";
pub const FAILED: &str = "failed";
pub const CANCELLED: &str = "cancelled";

pub const ORIGIN_USER: &str = "user";
pub const ORIGIN_ADMIN: &str = "admin";

const COLUMNS: &str = "server_transfer_requests.uuid, server_transfer_requests.server_uuid, \
    server_transfer_requests.user_uuid, server_transfer_requests.origin, \
    server_transfer_requests.location_uuid, server_transfer_requests.source_node_uuid, \
    server_transfer_requests.destination_node_uuid, server_transfer_requests.tried_node_uuids, \
    server_transfer_requests.attempts, server_transfer_requests.status, \
    server_transfer_requests.error, server_transfer_requests.restart, \
    server_transfer_requests.variables, server_transfer_requests.created, \
    server_transfer_requests.started, server_transfer_requests.finished";

/// One move of a server.
#[derive(Clone, Debug)]
pub struct TransferRequest {
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub user_uuid: Option<uuid::Uuid>,
    pub origin: String,
    pub location_uuid: Option<uuid::Uuid>,
    pub source_node_uuid: Option<uuid::Uuid>,
    pub destination_node_uuid: Option<uuid::Uuid>,
    pub tried_node_uuids: Vec<uuid::Uuid>,
    pub attempts: i32,
    pub status: String,
    pub error: Option<String>,
    /// The server was running when the move began, so it is started again afterwards.
    pub restart: bool,
    /// Egg variables the new ports go into, written once the move is done.
    pub variables: HashMap<String, String>,
    pub created: chrono::NaiveDateTime,
    pub started: Option<chrono::NaiveDateTime>,
    pub finished: Option<chrono::NaiveDateTime>,
}

impl TransferRequest {
    fn map(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            uuid: row.try_get("uuid")?,
            server_uuid: row.try_get("server_uuid")?,
            user_uuid: row.try_get("user_uuid")?,
            origin: row.try_get("origin")?,
            location_uuid: row.try_get("location_uuid")?,
            source_node_uuid: row.try_get("source_node_uuid")?,
            destination_node_uuid: row.try_get("destination_node_uuid")?,
            tried_node_uuids: row.try_get("tried_node_uuids")?,
            attempts: row.try_get("attempts")?,
            status: row.try_get("status")?,
            error: row.try_get("error")?,
            restart: row.try_get("restart")?,
            variables: serde_json::from_value(row.try_get("variables")?).unwrap_or_default(),
            created: row.try_get("created")?,
            started: row.try_get("started")?,
            finished: row.try_get("finished")?,
        })
    }

    pub fn active(&self) -> bool {
        matches!(self.status.as_str(), QUEUED | RUNNING | CANCELLING)
    }

    pub fn into_api_object(self) -> ApiTransferRequest {
        ApiTransferRequest {
            uuid: self.uuid,
            origin: self.origin.into(),
            status: self.status.into(),
            location_uuid: self.location_uuid,
            attempts: self.attempts,
            error: self.error,
            created: self.created.and_utc(),
            finished: self.finished.map(|finished| finished.and_utc()),
        }
    }

    /// The newest move of the server.
    pub async fn latest(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM server_transfer_requests
            WHERE server_transfer_requests.server_uuid = $1
            ORDER BY server_transfer_requests.created DESC
            LIMIT 1"
        )))
        .bind(server_uuid)
        .fetch_optional(database.write())
        .await?;

        row.as_ref().map(Self::map).transpose()
    }

    async fn all_with_status(
        database: &crate::database::Database,
        statuses: &[&str],
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM server_transfer_requests
            WHERE server_transfer_requests.status = ANY($1)
            ORDER BY server_transfer_requests.created"
        )))
        .bind(statuses)
        .fetch_all(database.write())
        .await?;

        rows.iter().map(Self::map).collect()
    }

    async fn save(&self, database: &crate::database::Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE server_transfer_requests
            SET destination_node_uuid = $2, tried_node_uuids = $3, attempts = $4, status = $5,
                error = $6, variables = $7, started = $8, finished = $9
            WHERE uuid = $1
            "#,
        )
        .bind(self.uuid)
        .bind(self.destination_node_uuid)
        .bind(&self.tried_node_uuids)
        .bind(self.attempts)
        .bind(&self.status)
        .bind(&self.error)
        .bind(serde_json::to_value(&self.variables).unwrap_or_default())
        .bind(self.started)
        .bind(self.finished)
        .execute(database.write())
        .await?;

        Ok(())
    }

    fn finish(&mut self, status: &str, error: Option<String>) {
        self.status = status.into();
        self.error = error;
        self.finished = Some(chrono::Utc::now().naive_utc());
    }
}

#[derive(ToSchema, Serialize, Clone)]
pub struct ApiTransferRequest {
    pub uuid: uuid::Uuid,
    /// `user` or `admin`.
    pub origin: compact_str::CompactString,
    /// `queued`, `running`, `cancelling`, `completed`, `failed` or `cancelled`.
    pub status: compact_str::CompactString,
    pub location_uuid: Option<uuid::Uuid>,
    /// Nodes tried so far.
    pub attempts: i32,
    pub error: Option<String>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub finished: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiTransferOptions {
    /// Players may move their servers (an admin always may).
    pub enabled: bool,
    pub current_location_uuid: uuid::Uuid,
    pub locations: Vec<ApiCreateLocation>,
    /// When the server may move again; `null` when it may now.
    pub cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    /// The move under way, else the last one.
    pub request: Option<ApiTransferRequest>,
}

#[derive(Debug)]
pub enum TransferError {
    Disabled,
    Cooldown(chrono::DateTime<chrono::Utc>),
    /// A move of the server is already under way.
    Busy,
    /// Suspended, installing or restoring: the server cannot move now.
    Unavailable,
    NotMoving,
    /// An admin moves the server: only an admin cancels it.
    AdminMove,
    UnknownLocation,
    LocationLocked,
    NoNode(String),
    Other(anyhow::Error),
}

impl TransferError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "moving servers is turned off".into(),
            Self::Cooldown(until) => format!(
                "this server was moved recently, it can move again at {}",
                until.format("%Y-%m-%d %H:%M UTC")
            ),
            Self::Busy => "this server is already being moved".into(),
            Self::Unavailable => "this server cannot be moved right now".into(),
            Self::NotMoving => "this server is not being moved".into(),
            Self::AdminMove => "an admin is moving this server, only an admin can cancel it".into(),
            Self::UnknownLocation => "this location is not offered".into(),
            Self::LocationLocked => "your plan does not include this location".into(),
            Self::NoNode(reason) => format!("no node can take the server right now: {reason}"),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled | Self::LocationLocked | Self::AdminMove => 403,
            Self::Cooldown(_) => 429,
            Self::Busy | Self::Unavailable | Self::NotMoving | Self::NoNode(_) => 409,
            Self::UnknownLocation => 400,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for TransferError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for TransferError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl From<sqlx::Error> for TransferError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

/// When a server whose last player move finished at `finished` may move again.
pub fn cooldown_until(
    finished: Option<chrono::NaiveDateTime>,
    hours: i32,
    now: chrono::NaiveDateTime,
) -> Option<chrono::NaiveDateTime> {
    let until = finished? + chrono::Duration::hours(hours.max(0) as i64);

    (until > now).then_some(until)
}

/// Whether a failed attempt is followed by one on another node.
pub fn retries(attempts: i32, max_attempts: i32) -> bool {
    attempts < max_attempts.max(1)
}

/// The end of the last player move that completed.
async fn last_finished(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Option<chrono::NaiveDateTime>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT MAX(finished) FROM server_transfer_requests
        WHERE server_uuid = $1 AND origin = 'user' AND status = 'completed'
        "#,
    )
    .bind(server_uuid)
    .fetch_one(database.write())
    .await
}

/// What the server's Transfer page shows.
pub async fn options(
    state: &crate::State,
    server: &Server,
) -> Result<ApiTransferOptions, anyhow::Error> {
    let (enabled, cooldown_hours) = {
        let settings = state.settings.get().await?;
        (
            settings.games.transfer_enabled,
            settings.games.transfer_cooldown_hours,
        )
    };

    let node = server.node.fetch_cached(&state.database).await?;
    let locations = crate::server_creation::locations(state, &server.owner).await?;
    let now = chrono::Utc::now().naive_utc();
    let cooldown = cooldown_until(
        last_finished(&state.database, server.uuid).await?,
        cooldown_hours,
        now,
    );

    Ok(ApiTransferOptions {
        enabled,
        current_location_uuid: node.location.uuid,
        locations,
        cooldown_until: cooldown.map(|until| until.and_utc()),
        request: TransferRequest::latest(&state.database, server.uuid)
            .await?
            .map(TransferRequest::into_api_object),
    })
}

/// Whether Wings says the server is running (or starting); `false` when it cannot tell.
async fn running(state: &crate::State, server: &Server) -> bool {
    let usage = async {
        server
            .node
            .fetch_cached(&state.database)
            .await?
            .api_client(&state.database)
            .await?
            .get_servers_utilization()
            .await
            .map_err(anyhow::Error::from)
    }
    .await;

    usage
        .ok()
        .and_then(|usage| usage.get(&server.uuid).cloned())
        .is_some_and(|usage| {
            matches!(
                usage.state,
                wings_api::ServerState::Running | wings_api::ServerState::Starting
            )
        })
}

/// A player (or an admin, `admin`) moves the server to a node of `location_uuid`; the same
/// location finds another node there.
pub async fn request(
    state: &crate::State,
    server: &Server,
    user: &User,
    location_uuid: uuid::Uuid,
    admin: bool,
) -> Result<ApiTransferRequest, TransferError> {
    let (enabled, cooldown_hours) = {
        let settings = state.settings.get().await?;
        (
            settings.games.transfer_enabled,
            settings.games.transfer_cooldown_hours,
        )
    };
    if !admin && !enabled {
        return Err(TransferError::Disabled);
    }
    // the move's routes skip the server's state check, so that a running move stays visible
    // a parked server (limbo) has no files on a node to move
    if (server.suspended && !admin)
        || server.status.is_some()
        || server.limbo.is_some_and(|limbo| limbo.away())
    {
        return Err(TransferError::Unavailable);
    }

    if server.destination_node.is_some()
        || TransferRequest::latest(&state.database, server.uuid)
            .await?
            .is_some_and(|request| request.active())
    {
        return Err(TransferError::Busy);
    }

    if !admin
        && let Some(until) = cooldown_until(
            last_finished(&state.database, server.uuid).await?,
            cooldown_hours,
            chrono::Utc::now().naive_utc(),
        )
    {
        return Err(TransferError::Cooldown(until.and_utc()));
    }

    let location = Location::by_uuid_optional(&state.database, location_uuid)
        .await?
        .filter(|location| admin || location.public)
        .ok_or(TransferError::UnknownLocation)?;
    if !admin
        && !crate::server_creation::tier_unlocked(state, &server.owner, location.tier.as_deref())
            .await?
    {
        return Err(TransferError::LocationLocked);
    }

    // a start waiting in the start queue goes along: the server starts on its new node
    let queued = crate::start_queue::entry(&state.database, server.uuid)
        .await?
        .is_some_and(|entry| entry.waiting());
    let restart = queued || running(state, server).await;
    let mut request = insert(
        state,
        server,
        Some(user.uuid),
        if admin { ORIGIN_ADMIN } else { ORIGIN_USER },
        location.uuid,
        restart,
    )
    .await?;
    if queued && let Err(err) = crate::start_queue::leave(state, server.uuid).await {
        tracing::warn!(server = %server.uuid, "failed to take a moved server out of the start queue: {err:#}");
    }

    if let Err(err) = start_attempt(state, &mut request).await {
        request.finish(FAILED, Some(err.message()));
        request.save(&state.database).await?;

        return Err(err);
    }

    Ok(request.into_api_object())
}

async fn insert(
    state: &crate::State,
    server: &Server,
    user_uuid: Option<uuid::Uuid>,
    origin: &str,
    location_uuid: uuid::Uuid,
    restart: bool,
) -> Result<TransferRequest, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO server_transfer_requests
            (server_uuid, user_uuid, origin, location_uuid, source_node_uuid, status, restart)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING {COLUMNS}"
    )))
    .bind(server.uuid)
    .bind(user_uuid)
    .bind(origin)
    .bind(location_uuid)
    .bind(server.node.uuid)
    .bind(QUEUED)
    .bind(restart)
    .fetch_one(state.database.write())
    .await?;

    TransferRequest::map(&row)
}

/// The allocations a server gets on `node`, and the variables its new ports go into.
pub(crate) struct Picked {
    pub(crate) primary: Option<uuid::Uuid>,
    pub(crate) additional: Vec<uuid::Uuid>,
    pub(crate) variables: HashMap<String, String>,
}

/// Its own port numbers where `node` has them all free on one address, else what the egg
/// configuration deploys, else any free ones. `None` when the node has too few free allocations.
pub(crate) async fn pick_allocations(
    state: &crate::State,
    server: &Server,
    node: &Node,
) -> Result<Option<Picked>, anyhow::Error> {
    let mut ports = Vec::new();
    if let Some(primary) = &server.allocation {
        ports.push(primary.allocation.port);
    }
    for allocation in ServerAllocation::all_by_server_uuid(&state.database, server.uuid).await? {
        if server
            .allocation
            .as_ref()
            .is_some_and(|primary| primary.uuid == allocation.uuid)
        {
            continue;
        }
        ports.push(allocation.allocation.port);
    }

    let has_primary = server.allocation.is_some();
    let split = |uuids: Vec<uuid::Uuid>| {
        let mut uuids = uuids.into_iter();
        let primary = if has_primary { uuids.next() } else { None };

        (primary, uuids.collect::<Vec<_>>())
    };

    if ports.is_empty() {
        return Ok(Some(Picked {
            primary: None,
            additional: Vec::new(),
            variables: HashMap::new(),
        }));
    }

    let used = NodeAllocation::used_by_node_any_ip(state, node).await?;

    if let Some((_, preserved)) =
        NodeAllocation::get_preserved(&state.database, node.uuid, &ports, &used).await?
    {
        let mut by_port: HashMap<i32, Vec<uuid::Uuid>> = HashMap::new();
        for (uuid, port) in preserved {
            by_port.entry(port).or_default().push(uuid);
        }

        let kept = ports
            .iter()
            .map(|port| by_port.get_mut(port).and_then(Vec::pop))
            .collect::<Option<Vec<_>>>();
        if let Some(kept) = kept {
            let (primary, additional) = split(kept);

            return Ok(Some(Picked {
                primary,
                additional,
                variables: HashMap::new(),
            }));
        }
    }

    let configuration = EggConfiguration::merged_for_target(
        &state.database,
        &EggConfigurationTarget::new(&server.egg, node),
    )
    .await?;
    if let Some(deployment) = configuration
        .config_allocations
        .map(|allocations| allocations.deployment)
        .filter(|deployment| deployment.primary.is_some() || !has_primary)
    {
        let mut variables = HashMap::new();
        if let Ok((primary, additional)) =
            NodeAllocation::get_from_deployment(state, &deployment, node, &mut variables).await
            && (primary.is_some() || !has_primary)
        {
            return Ok(Some(Picked {
                primary,
                additional,
                variables: variables
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
            }));
        }
    }

    let random = NodeAllocation::get_random(
        &state.database,
        node.uuid,
        1,
        u16::MAX,
        ports.len() as i64,
        &used,
    )
    .await
    .unwrap_or_default();
    if random.len() < ports.len() {
        return Ok(None);
    }

    let (primary, additional) = split(random);

    Ok(Some(Picked {
        primary,
        additional,
        variables: HashMap::new(),
    }))
}

pub(crate) fn limits(server: &Server) -> AdminApiServerLimits {
    AdminApiServerLimits {
        cpu: server.cpu,
        memory: server.memory,
        memory_overhead: server.memory_overhead,
        swap: server.swap,
        disk: server.disk,
        io_weight: server.io_weight,
        bandwidth: None,
    }
}

/// Starts the native transfer to the best node of the location the request has not tried yet.
async fn start_attempt(
    state: &crate::State,
    request: &mut TransferRequest,
) -> Result<(), TransferError> {
    let server = Server::by_uuid_optional(&state.database, request.server_uuid)
        .await?
        .ok_or(TransferError::NotMoving)?;
    let source = server.node.fetch_cached(&state.database).await?;
    let location_uuid = request.location_uuid.unwrap_or(source.location.uuid);
    // players' servers go to game nodes; an admin's move stays among nodes of the source's role
    let role = if request.origin == ORIGIN_USER {
        GAME_ROLE
    } else {
        source.role.as_str()
    };

    let nodes = Node::by_location_uuids_most_eligible(
        &state.database,
        &[location_uuid],
        limits(&server),
        false,
        SUSPENSION_PENALTY,
        RANDOMNESS,
    )
    .await?
    .into_iter()
    .filter(|node| {
        node.role == role
            && node.uuid != server.node.uuid
            && !request.tried_node_uuids.contains(&node.uuid)
    })
    .collect::<Vec<_>>();

    let mut reason = None;
    for node in nodes {
        let Some(picked) = pick_allocations(state, &server, &node).await? else {
            reason = Some("not enough free allocations".to_string());
            continue;
        };

        let backups =
            ServerBackup::all_uuids_by_server_uuid_not_shared(&state.database, server.uuid).await?;
        let node_uuid = node.uuid;

        server
            .clone()
            .transfer(
                state,
                ServerTransferOptions {
                    destination_node: node,
                    allocation_uuid: picked.primary,
                    allocation_uuids: picked.additional,
                    backups,
                    delete_source_backups: true,
                    archive_format: wings_api::TransferArchiveFormat::TarZstd,
                    compression_level: None,
                    multiplex_channels: 0,
                },
            )
            .await?;

        request.status = RUNNING.into();
        request.destination_node_uuid = Some(node_uuid);
        request.tried_node_uuids.push(node_uuid);
        request.attempts += 1;
        request.error = None;
        request.variables = picked.variables;
        request.started = Some(chrono::Utc::now().naive_utc());
        request.save(&state.database).await?;

        tracing::info!(server = %server.uuid, node = %node_uuid, attempt = request.attempts, "moving a server");

        return Ok(());
    }

    let reason = match Node::find_deployment_blocker(
        &state.database,
        &[location_uuid],
        limits(&server),
        false,
        SUSPENSION_PENALTY,
    )
    .await?
    {
        Some(blocker) if reason.is_none() => blocker.message().to_string(),
        _ => reason.unwrap_or_else(|| "no other node of the location can take it".into()),
    };

    Err(TransferError::NoNode(reason))
}

/// Cancels the server's move: a queued one at once, a running one through Wings (it settles once
/// Wings has stopped copying). A move an admin started (`admin` false) is refused.
pub async fn cancel(
    state: &crate::State,
    server: &Server,
    admin: bool,
) -> Result<(), TransferError> {
    let Some(mut request) = TransferRequest::latest(&state.database, server.uuid)
        .await?
        .filter(TransferRequest::active)
    else {
        // a transfer without a request was started in Admin › Servers
        return Err(if server.destination_node.is_some() {
            TransferError::AdminMove
        } else {
            TransferError::NotMoving
        });
    };
    if request.origin == ORIGIN_ADMIN && !admin {
        return Err(TransferError::AdminMove);
    }

    match request.status.as_str() {
        QUEUED => {
            request.finish(CANCELLED, None);
            request.save(&state.database).await?;
        }
        RUNNING => {
            if server.destination_node.is_some() {
                server
                    .node
                    .fetch_cached(&state.database)
                    .await?
                    .api_client(&state.database)
                    .await?
                    .delete_servers_server_transfer(server.uuid)
                    .await
                    .map_err(anyhow::Error::from)?;
            }

            request.status = CANCELLING.into();
            request.save(&state.database).await?;
        }
        _ => {}
    }

    Ok(())
}

/// An admin cancelled the server's native transfer (Admin › Servers): the move must not be
/// retried on another node.
pub async fn mark_cancelling(state: &crate::State, server_uuid: uuid::Uuid) {
    let result = sqlx::query(
        "UPDATE server_transfer_requests SET status = $2 WHERE server_uuid = $1 AND status = $3",
    )
    .bind(server_uuid)
    .bind(CANCELLING)
    .bind(RUNNING)
    .execute(state.database.write())
    .await;

    if let Err(err) = result {
        tracing::warn!(server = %server_uuid, "failed to mark a move as cancelled: {err}");
    }
}

/// Starts a server again after its move; a suspended one stays off.
async fn start_again(state: &crate::State, server: &Server) {
    if server.suspended {
        return;
    }

    let started = async {
        server
            .node
            .fetch_cached(&state.database)
            .await?
            .api_client(&state.database)
            .await?
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Start,
                    wait_seconds: None,
                },
            )
            .await?;

        Ok::<_, anyhow::Error>(())
    }
    .await;

    if let Err(err) = started {
        tracing::warn!(server = %server.uuid, "failed to start a server after its move: {err:#}");
    }
}

/// Writes the new ports into the egg variables the egg configuration named.
async fn write_variables(
    state: &crate::State,
    server: &Server,
    variables: &HashMap<String, String>,
) -> Result<(), anyhow::Error> {
    if variables.is_empty() {
        return Ok(());
    }

    let egg_variables = NestEggVariable::all_by_egg_uuid(&state.database, server.egg.uuid).await?;
    for (name, value) in variables {
        if let Some(variable) = egg_variables
            .iter()
            .find(|v| v.env_variable == name.as_str())
        {
            ServerVariable::create(&state.database, server.uuid, variable.uuid, value).await?;
        }
    }

    Ok(())
}

/// The `settle_transfers` task: closes moves whose native transfer is over (done, failed and
/// retried on another node, or cancelled), and starts queued ones while their node has a free
/// turn.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let (max_attempts, parallel) = {
        let settings = state.settings.get().await?;
        (
            settings.games.transfer_attempts,
            settings.games.transfer_parallel,
        )
    };

    for mut request in
        TransferRequest::all_with_status(&state.database, &[RUNNING, CANCELLING]).await?
    {
        let Some(server) = Server::by_uuid_optional(&state.database, request.server_uuid).await?
        else {
            continue;
        };
        if server.destination_node.is_some() {
            continue;
        }

        if Some(server.node.uuid) == request.destination_node_uuid {
            if let Err(err) = write_variables(state, &server, &request.variables).await {
                tracing::warn!(server = %server.uuid, "failed to write a moved server's ports: {err:#}");
            }
            request.finish(COMPLETED, None);
            request.save(&state.database).await?;
            tracing::info!(server = %server.uuid, "moved a server");

            if request.restart {
                start_again(state, &server).await;
            }
            continue;
        }

        if request.status == CANCELLING {
            request.finish(CANCELLED, None);
        } else if retries(request.attempts, max_attempts) {
            match start_attempt(state, &mut request).await {
                Ok(()) => continue,
                Err(err) => request.finish(FAILED, Some(err.message())),
            }
        } else {
            request.finish(
                FAILED,
                Some("the copy to the new node failed on every node tried".into()),
            );
        }
        request.save(&state.database).await?;

        if request.restart {
            start_again(state, &server).await;
        }
    }

    // queued moves, a few per source node at a time
    let mut moving: HashMap<uuid::Uuid, i32> = HashMap::new();
    for request in TransferRequest::all_with_status(&state.database, &[RUNNING, CANCELLING]).await?
    {
        if let Some(source) = request.source_node_uuid {
            *moving.entry(source).or_default() += 1;
        }
    }

    for mut request in TransferRequest::all_with_status(&state.database, &[QUEUED]).await? {
        let source = request.source_node_uuid.unwrap_or_default();
        let turns = moving.entry(source).or_default();
        if *turns >= parallel.max(1) {
            continue;
        }

        match Server::by_uuid_optional(&state.database, request.server_uuid).await? {
            Some(server) if server.destination_node.is_none() && server.node.uuid == source => {}
            _ => {
                request.finish(
                    CANCELLED,
                    Some("the server moved or changed meanwhile".into()),
                );
                request.save(&state.database).await?;
                continue;
            }
        }

        if let Err(err) = start_attempt(state, &mut request).await {
            request.finish(FAILED, Some(err.message()));
            request.save(&state.database).await?;
            continue;
        }

        *turns += 1;
    }

    Ok(())
}

/// Which of a node's servers "Lighten load" takes off it.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LightenPick {
    /// Servers that are off: nobody is disconnected, they stay off.
    Stopped,
    /// Running servers using the most memory: stopped, moved and started again.
    Busiest,
}

/// One of the node's servers as "Lighten load" sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub uuid: uuid::Uuid,
    pub running: bool,
    pub memory_used: u64,
}

/// The servers to move, in order.
pub fn choose(mut candidates: Vec<Candidate>, pick: LightenPick, count: usize) -> Vec<Candidate> {
    match pick {
        LightenPick::Stopped => candidates.retain(|candidate| !candidate.running),
        LightenPick::Busiest => {
            candidates.retain(|candidate| candidate.running);
            candidates.sort_by(|a, b| b.memory_used.cmp(&a.memory_used));
        }
    }
    candidates.truncate(count);

    candidates
}

/// Queues moves of `count` of the node's servers to other nodes of its location; [`settle`] starts
/// them a few at a time. Returns how many were queued.
pub async fn lighten(
    state: &crate::State,
    node: &Node,
    admin: &User,
    pick: LightenPick,
    count: usize,
) -> Result<usize, anyhow::Error> {
    let uuids: Vec<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT servers.uuid FROM servers
        WHERE servers.node_uuid = $1 AND servers.destination_node_uuid IS NULL
            AND servers.status IS NULL
            AND NOT EXISTS (SELECT 1 FROM devboxes WHERE devboxes.server_uuid = servers.uuid)
            AND NOT EXISTS (
                SELECT 1 FROM server_transfer_requests requests
                WHERE requests.server_uuid = servers.uuid
                    AND requests.status IN ('queued', 'running', 'cancelling')
            )
        "#,
    )
    .bind(node.uuid)
    .fetch_all(state.database.write())
    .await?;

    let usage = node
        .api_client(&state.database)
        .await?
        .get_servers_utilization()
        .await
        .map_err(anyhow::Error::from)?;

    let candidates = uuids
        .into_iter()
        .map(|uuid| {
            let usage = usage.get(&uuid);
            Candidate {
                uuid,
                running: usage.is_some_and(|usage| {
                    matches!(
                        usage.state,
                        wings_api::ServerState::Running | wings_api::ServerState::Starting
                    )
                }),
                memory_used: usage.map_or(0, |usage| usage.memory_bytes),
            }
        })
        .collect();

    let chosen = choose(candidates, pick, count);
    for candidate in &chosen {
        let Some(server) = Server::by_uuid_optional(&state.database, candidate.uuid).await? else {
            continue;
        };

        insert(
            state,
            &server,
            Some(admin.uuid),
            ORIGIN_ADMIN,
            node.location.uuid,
            candidate.running,
        )
        .await?;
    }

    Ok(chosen.len())
}

/// Queues an admin move of the server to `location_uuid` with no one behind it (the panel's own,
/// a PlusPlus that ran out); [`settle`] starts it.
pub async fn queue_admin_move(
    state: &crate::State,
    server: &Server,
    location_uuid: uuid::Uuid,
) -> Result<TransferRequest, sqlx::Error> {
    let restart = running(state, server).await;

    insert(state, server, None, ORIGIN_ADMIN, location_uuid, restart).await
}

/// What a location can still take, for Admin › Overview › Capacity.
#[derive(ToSchema, Serialize)]
pub struct ApiLocationCapacity {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub nodes: i64,
    /// Nodes that take new servers now.
    pub open_nodes: i64,
    /// MiB not allocated on nodes with a limit (nodes without one are not counted).
    pub memory_free: i64,
    pub memory_total: i64,
    pub disk_free: i64,
    pub disk_total: i64,
    /// The average 5-minute pressure of its answering nodes, in percent.
    pub pressure: Option<f32>,
}

pub async fn capacity(state: &crate::State) -> Result<Vec<ApiLocationCapacity>, anyhow::Error> {
    let rows = sqlx::query(
        r#"
        WITH allocated AS (
            SELECT node_uuid, SUM(memory + memory_overhead)::BIGINT AS memory,
                SUM(disk)::BIGINT AS disk
            FROM servers
            WHERE servers.limbo_state IS DISTINCT FROM 'parked'
            GROUP BY node_uuid
        )
        SELECT locations.uuid, locations.name,
            COUNT(nodes.uuid) AS nodes,
            COUNT(nodes.uuid) FILTER (
                WHERE nodes.deployment_enabled AND NOT nodes.maintenance_enabled
                AND NOT nodes.deployment_paused
            ) AS open_nodes,
            COALESCE(SUM(GREATEST(nodes.memory - COALESCE(allocated.memory, 0), 0))
                FILTER (WHERE nodes.memory > 0), 0)::BIGINT AS memory_free,
            COALESCE(SUM(nodes.memory), 0)::BIGINT AS memory_total,
            COALESCE(SUM(GREATEST(nodes.disk - COALESCE(allocated.disk, 0), 0))
                FILTER (WHERE nodes.disk > 0), 0)::BIGINT AS disk_free,
            COALESCE(SUM(nodes.disk), 0)::BIGINT AS disk_total,
            (AVG(telemetry.pressure) FILTER (WHERE telemetry.reachable))::REAL AS pressure
        FROM locations
        LEFT JOIN nodes ON nodes.location_uuid = locations.uuid
        LEFT JOIN allocated ON allocated.node_uuid = nodes.uuid
        LEFT JOIN node_telemetry telemetry ON telemetry.node_uuid = nodes.uuid
        GROUP BY locations.uuid
        ORDER BY locations.sort, locations.name
        "#,
    )
    .fetch_all(state.database.read())
    .await?;

    rows.iter()
        .map(|row| {
            Ok(ApiLocationCapacity {
                uuid: row.try_get("uuid")?,
                name: row.try_get::<String, _>("name")?.into(),
                nodes: row.try_get("nodes")?,
                open_nodes: row.try_get("open_nodes")?,
                memory_free: row.try_get("memory_free")?,
                memory_total: row.try_get("memory_total")?,
                disk_free: row.try_get("disk_free")?,
                disk_total: row.try_get("disk_total")?,
                pressure: row.try_get("pressure")?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(Into::into)
}

/// A move as Admin › Overview › Capacity lists it.
#[derive(ToSchema, Serialize)]
pub struct ApiAdminTransferRequest {
    #[serde(flatten)]
    pub request: ApiTransferRequest,
    pub server_uuid: uuid::Uuid,
    pub server_name: compact_str::CompactString,
    pub source_node_name: Option<compact_str::CompactString>,
    pub destination_node_name: Option<compact_str::CompactString>,
}

/// The moves under way and the last finished ones, newest first.
pub async fn recent(
    state: &crate::State,
    limit: i64,
) -> Result<Vec<ApiAdminTransferRequest>, anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS}, servers.name AS server_name, source.name AS source_node_name,
            destination.name AS destination_node_name
        FROM server_transfer_requests
        JOIN servers ON servers.uuid = server_transfer_requests.server_uuid
        LEFT JOIN nodes source ON source.uuid = server_transfer_requests.source_node_uuid
        LEFT JOIN nodes destination ON destination.uuid = server_transfer_requests.destination_node_uuid
        ORDER BY server_transfer_requests.status IN ('queued', 'running', 'cancelling') DESC,
            server_transfer_requests.created DESC
        LIMIT $1"
    )))
    .bind(limit)
    .fetch_all(state.database.read())
    .await?;

    rows.iter()
        .map(|row| {
            let request = TransferRequest::map(row)?;
            Ok(ApiAdminTransferRequest {
                server_uuid: request.server_uuid,
                server_name: row.try_get::<String, _>("server_name")?.into(),
                source_node_name: row
                    .try_get::<Option<String>, _>("source_node_name")?
                    .map(Into::into),
                destination_node_name: row
                    .try_get::<Option<String>, _>("destination_node_name")?
                    .map(Into::into),
                request: request.into_api_object(),
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn the_cooldown_counts_from_the_last_finished_move() {
        assert_eq!(cooldown_until(None, 24, at(12)), None);
        assert_eq!(cooldown_until(Some(at(1)), 6, at(12)), None);
        assert_eq!(cooldown_until(Some(at(10)), 6, at(12)), Some(at(16)));
        assert_eq!(cooldown_until(Some(at(10)), 0, at(12)), None);
    }

    #[test]
    fn attempts_stop_at_the_limit() {
        assert!(retries(1, 2));
        assert!(!retries(2, 2));
        assert!(!retries(1, 0));
    }

    #[test]
    fn lighten_picks_stopped_or_the_busiest() {
        let candidate = |n: u128, running: bool, memory_used: u64| Candidate {
            uuid: uuid::Uuid::from_u128(n),
            running,
            memory_used,
        };
        let all = vec![
            candidate(1, true, 100),
            candidate(2, false, 0),
            candidate(3, true, 900),
            candidate(4, false, 0),
            candidate(5, true, 500),
        ];

        let stopped = choose(all.clone(), LightenPick::Stopped, 5);
        assert_eq!(
            stopped.iter().map(|c| c.uuid.as_u128()).collect::<Vec<_>>(),
            vec![2, 4]
        );

        let busiest = choose(all, LightenPick::Busiest, 2);
        assert_eq!(
            busiest.iter().map(|c| c.uuid.as_u128()).collect::<Vec<_>>(),
            vec![3, 5]
        );
    }
}
