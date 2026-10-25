//! plugged.host: node failover (docs/features/failover.md): a down node's servers are restored on
//! other nodes of its location from their newest backup off the node.
//!
//! A failover is a `node_failovers` row with one `server_failovers` row per server. The
//! `settle_failovers` task ([`settle`]) moves each server along, a few at a time:
//!
//! 1. `queued` → `rehoming`: the newest successful backup on storage other nodes reach is picked,
//!    the server's row moves to the best node of the location ([`Server::rehome`], ports kept when
//!    free) and that node's Wings creates it without its install script.
//! 2. `rehoming` → `restoring`: the backup is restored there, the directory emptied first.
//! 3. `restoring` → `done` or `failed`, from the restore's outcome.
//!
//! Once the old node answers again, a `done` server's copy there is removed (the Wings fork's `POST
//! /api/system/orphans`). A server that failed before its row moved stays on the old node; a failed
//! server can be tried again ([`resume`]).
//! Automatic failovers (Admin › Settings › Hosting › Recovery) start for a node that missed enough
//! checks in a row, one at a time, and only while most other nodes answer: when many nodes look
//! down at once, the panel's own network is the likelier cause. In the notify mode the same node is
//! only reported to the admins with what a failover would do ([`plan`], a `notify` row whose servers
//! are `planned`); [`plan`] is also the Failover tab's dry run.

use crate::{
    models::{
        ByUuid, CreatableModel,
        node::Node,
        server::{Server, ServerStatus, rehome::RehomeOptions},
        server_activity::{CreateServerActivityOptions, ServerActivity},
        server_backup::{BackupDisk, ServerBackup, ServerBackupRestoreOptions},
    },
    server_creation::{RANDOMNESS, SUSPENSION_PENALTY},
    settings::recovery::FailoverMode,
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

pub const MODE_AUTO: &str = "auto";
pub const MODE_MANUAL: &str = "manual";
pub const MODE_DRILL: &str = "drill";
/// A down node reported in the notify mode: nothing moves.
pub const MODE_NOTIFY: &str = "notify";

/// A node failover's status.
pub const RUNNING: &str = "running";
/// A node failover's and a server's status.
pub const DONE: &str = "done";
pub const CANCELLED: &str = "cancelled";
/// A `notify` row's status: the admins were told.
pub const NOTIFIED: &str = "notified";

/// A server's status.
pub const QUEUED: &str = "queued";
pub const REHOMING: &str = "rehoming";
pub const RESTORING: &str = "restoring";
pub const FAILED: &str = "failed";
/// A server of a `notify` row: where a failover would put it; nothing moved.
pub const PLANNED: &str = "planned";

/// How long a server may wait for its new node's Wings to take the restore.
const REHOME_TIMEOUT_MINUTES: i64 = 10;
/// How long a restore may run.
const RESTORE_TIMEOUT_HOURS: i64 = 6;

#[derive(Debug)]
pub enum FailoverError {
    /// Admin › Settings › Hosting › Recovery has failover off.
    Disabled,
    /// A failover of the node is under way.
    Running,
    /// The drill's server is not on the node.
    NotOnNode,
    /// Nothing to fail over.
    NoServers,
    NotRunning,
    /// No such server row in the node's failovers.
    NotFound,
    /// The server row cannot be tried again, and why.
    NotResumable(&'static str),
    Other(anyhow::Error),
}

impl FailoverError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => {
                "node failover is turned off (Admin › Settings › Hosting › Recovery)".into()
            }
            Self::Running => "a failover of this node is already under way".into(),
            Self::NotOnNode => "that server is not on this node".into(),
            Self::NoServers => "this node has no servers".into(),
            Self::NotRunning => "no failover of this node is under way".into(),
            Self::NotFound => "that server is not in this node's failovers".into(),
            Self::NotResumable(why) => (*why).into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled => 403,
            Self::Running | Self::NoServers | Self::NotRunning | Self::NotResumable(_) => 409,
            Self::NotOnNode => 400,
            Self::NotFound => 404,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for FailoverError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<sqlx::Error> for FailoverError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<crate::database::DatabaseError> for FailoverError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

/// Whether a backup on `disk` can be restored by another node than the one that took it: S3,
/// Proxmox Backup Server, and restic with a repository that is not a path on the node.
pub fn off_node(disk: BackupDisk, restic_repository: Option<&str>) -> bool {
    match disk {
        BackupDisk::S3 | BackupDisk::ProxmoxBackupServer => true,
        BackupDisk::Restic => restic_repository.is_some_and(|repository| {
            let repository = repository.trim();
            !repository.is_empty() && !repository.starts_with('/')
        }),
        _ => false,
    }
}

/// A node as the automatic failover sees it.
#[derive(Debug, Clone)]
pub struct NodeHealth {
    pub node_uuid: uuid::Uuid,
    /// Checks in a row the machine was gone (a refused port does not count: the machine is up).
    pub outages: i32,
    pub reachable: bool,
    pub servers: i64,
    /// A failover of it ran since it last answered.
    pub failed_over: bool,
}

/// The node an automatic failover starts for, if any: the longest-gone node that missed `after`
/// checks in a row, has servers and was not failed over since it last answered, while at least
/// half of the other nodes answer. `None` while another failover runs.
pub fn auto_pick(nodes: &[NodeHealth], after: i32, failover_running: bool) -> Option<uuid::Uuid> {
    if failover_running {
        return None;
    }

    let candidate = nodes
        .iter()
        .filter(|node| node.outages >= after && node.servers > 0 && !node.failed_over)
        .max_by_key(|node| node.outages)?;

    let others = nodes
        .iter()
        .filter(|node| node.node_uuid != candidate.node_uuid)
        .collect::<Vec<_>>();
    let answering = others.iter().filter(|node| node.reachable).count();
    if answering == 0 || answering * 2 < others.len() {
        return None;
    }

    Some(candidate.node_uuid)
}

/// What a restore's server status says.
#[derive(Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    Done,
    Failed(&'static str),
    Wait,
}

pub fn restore_outcome(status: Option<ServerStatus>, minutes: i64) -> RestoreOutcome {
    match status {
        None => RestoreOutcome::Done,
        Some(ServerStatus::BackupRestoreFailed) => {
            RestoreOutcome::Failed("Wings could not restore the backup")
        }
        _ if minutes >= RESTORE_TIMEOUT_HOURS * 60 => {
            RestoreOutcome::Failed("the restore did not finish within 6 hours")
        }
        _ => RestoreOutcome::Wait,
    }
}

/// A node a failover could put servers on, as the dry run sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanNode {
    pub uuid: uuid::Uuid,
    /// Its limits in MiB; 0 is none.
    pub memory: i64,
    pub disk: i64,
    /// What its servers were given (suspended ones at the placement's penalty).
    pub used_memory: i64,
    pub used_disk: i64,
    /// Its 5-minute load in percent (`node_telemetry::PRESSURE_SQL`).
    pub pressure: Option<f32>,
    /// It answered its newest check.
    pub answers: bool,
}

/// Where each server (`(memory, disk)`, in order) would go: the index of its node, or `None` when
/// no node that answers has room. Like placement without its randomness: the node least full after
/// taking the server (allocated memory, allocated disk or load, whichever is highest), and every
/// server planned takes its room on its node, so one failover spreads like the real one.
pub fn plan_destinations(servers: &[(i64, i64)], nodes: &mut [PlanNode]) -> Vec<Option<usize>> {
    let ratio = |used: i64, limit: i64| {
        if limit == 0 {
            0.0
        } else {
            used as f64 / limit as f64
        }
    };

    servers
        .iter()
        .map(|&(memory, disk)| {
            let picked = nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    node.answers
                        && (node.memory == 0 || node.used_memory + memory <= node.memory)
                        && (node.disk == 0 || node.used_disk + disk <= node.disk)
                })
                .map(|(index, node)| {
                    let fullness = ratio(node.used_memory + memory, node.memory)
                        .max(ratio(node.used_disk + disk, node.disk))
                        .max(
                            node.pressure
                                .map_or(0.0, |pressure| pressure as f64 / 100.0),
                        );

                    (index, fullness)
                })
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index);

            if let Some(index) = picked {
                nodes[index].used_memory += memory;
                nodes[index].used_disk += disk;
            }

            picked
        })
        .collect()
}

/// Whether a failover is creating the server on its new node (the install Wings reports then is
/// the skipped one).
pub async fn creating(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM server_failovers
            WHERE server_uuid = $1 AND status IN ($2, $3)
        )",
    )
    .bind(server_uuid)
    .bind(REHOMING)
    .bind(RESTORING)
    .fetch_one(database.write())
    .await
}

/// Starts a failover of `node` by hand: all its servers, or for a drill only `drill_server`
/// (which is stopped on the node first: the node is up).
pub async fn start(
    state: &crate::State,
    node: &Node,
    started_by: Option<uuid::Uuid>,
    drill_server: Option<uuid::Uuid>,
) -> Result<uuid::Uuid, FailoverError> {
    if state.settings.get().await?.recovery.failover_mode == FailoverMode::Off {
        return Err(FailoverError::Disabled);
    }

    let mode = if drill_server.is_some() {
        MODE_DRILL
    } else {
        MODE_MANUAL
    };

    create(state, node.uuid, mode, started_by, drill_server).await
}

async fn create(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    mode: &str,
    started_by: Option<uuid::Uuid>,
    only_server: Option<uuid::Uuid>,
) -> Result<uuid::Uuid, FailoverError> {
    let mut transaction = state.database.write().begin().await?;

    // one at a time per node: the advisory lock keeps two clicks from starting two
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('node_failover:' || $1::text))")
        .bind(node_uuid)
        .execute(&mut *transaction)
        .await?;

    let running: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM node_failovers WHERE node_uuid = $1 AND status = $2)",
    )
    .bind(node_uuid)
    .bind(RUNNING)
    .fetch_one(&mut *transaction)
    .await?;
    if running {
        return Err(FailoverError::Running);
    }

    let servers: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT uuid, name FROM servers
        WHERE node_uuid = $1 AND ($2::uuid IS NULL OR uuid = $2)
            AND limbo_state IS DISTINCT FROM 'parked'
            AND NOT EXISTS (SELECT 1 FROM devboxes WHERE devboxes.server_uuid = servers.uuid)
        ORDER BY name",
    )
    .bind(node_uuid)
    .bind(only_server)
    .fetch_all(&mut *transaction)
    .await?;
    if servers.is_empty() {
        return Err(if only_server.is_some() {
            FailoverError::NotOnNode
        } else {
            FailoverError::NoServers
        });
    }

    let failover_uuid: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO node_failovers (node_uuid, mode, status, started_by)
        VALUES ($1, $2, $3, $4)
        RETURNING uuid",
    )
    .bind(node_uuid)
    .bind(mode)
    .bind(RUNNING)
    .bind(started_by)
    .fetch_one(&mut *transaction)
    .await?;

    let (uuids, names): (Vec<uuid::Uuid>, Vec<String>) = servers.into_iter().unzip();
    sqlx::query(
        "INSERT INTO server_failovers (node_failover_uuid, server_uuid, server_name,
            source_node_uuid, status)
        SELECT $1, v.uuid, v.name, $4, $5
        FROM UNNEST($2::uuid[], $3::text[]) AS v(uuid, name)",
    )
    .bind(failover_uuid)
    .bind(&uuids)
    .bind(&names)
    .bind(node_uuid)
    .bind(QUEUED)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    tracing::warn!(node = %node_uuid, mode, servers = uuids.len(), "node failover started");

    Ok(failover_uuid)
}

/// Stops the node's running failover: queued servers stay where they are, the ones under way
/// finish.
pub async fn cancel(state: &crate::State, node_uuid: uuid::Uuid) -> Result<u64, FailoverError> {
    let Some(failover_uuid) = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT uuid FROM node_failovers WHERE node_uuid = $1 AND status = $2",
    )
    .bind(node_uuid)
    .bind(RUNNING)
    .fetch_optional(state.database.write())
    .await?
    else {
        return Err(FailoverError::NotRunning);
    };

    let cancelled = sqlx::query(
        "UPDATE server_failovers SET status = $2, updated = NOW(), finished = NOW()
        WHERE node_failover_uuid = $1 AND status = $3",
    )
    .bind(failover_uuid)
    .bind(CANCELLED)
    .bind(QUEUED)
    .execute(state.database.write())
    .await?
    .rows_affected();

    finish_if_over(state, failover_uuid, CANCELLED).await?;

    Ok(cancelled)
}

/// Closes a node failover none of whose servers is still queued or under way.
async fn finish_if_over(
    state: &crate::State,
    failover_uuid: uuid::Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE node_failovers SET status = $2, finished = NOW()
        WHERE uuid = $1 AND status = $3 AND NOT EXISTS (
            SELECT 1 FROM server_failovers
            WHERE node_failover_uuid = $1 AND status IN ($4, $5, $6)
        )",
    )
    .bind(failover_uuid)
    .bind(status)
    .bind(RUNNING)
    .bind(QUEUED)
    .bind(REHOMING)
    .bind(RESTORING)
    .execute(state.database.write())
    .await?;

    Ok(())
}

struct ServerFailover {
    uuid: uuid::Uuid,
    node_failover_uuid: uuid::Uuid,
    mode: String,
    server_uuid: uuid::Uuid,
    source_node_uuid: uuid::Uuid,
    destination_node_uuid: Option<uuid::Uuid>,
    backup_uuid: Option<uuid::Uuid>,
    status: String,
    /// Minutes since the status last changed.
    minutes: i64,
}

async fn active_rows(state: &crate::State) -> Result<Vec<ServerFailover>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT server_failovers.uuid, server_failovers.node_failover_uuid, node_failovers.mode,
            server_failovers.server_uuid, server_failovers.source_node_uuid,
            server_failovers.destination_node_uuid, server_failovers.backup_uuid,
            server_failovers.status,
            (EXTRACT(EPOCH FROM NOW() - server_failovers.updated) / 60)::BIGINT AS minutes
        FROM server_failovers
        JOIN node_failovers ON node_failovers.uuid = server_failovers.node_failover_uuid
        WHERE node_failovers.status = $1 AND server_failovers.status IN ($2, $3, $4)
        ORDER BY server_failovers.created",
    )
    .bind(RUNNING)
    .bind(QUEUED)
    .bind(REHOMING)
    .bind(RESTORING)
    .fetch_all(state.database.write())
    .await?;

    rows.iter()
        .map(|row| {
            Ok(ServerFailover {
                uuid: row.try_get("uuid")?,
                node_failover_uuid: row.try_get("node_failover_uuid")?,
                mode: row.try_get("mode")?,
                server_uuid: row.try_get("server_uuid")?,
                source_node_uuid: row.try_get("source_node_uuid")?,
                destination_node_uuid: row.try_get("destination_node_uuid")?,
                backup_uuid: row.try_get("backup_uuid")?,
                status: row.try_get("status")?,
                minutes: row.try_get("minutes")?,
            })
        })
        .collect()
}

async fn set_status(
    state: &crate::State,
    row: &ServerFailover,
    status: &str,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE server_failovers
        SET status = $2, error = $3, updated = NOW(),
            finished = CASE WHEN $2 IN ($4, $5, $6) THEN NOW() ELSE NULL END
        WHERE uuid = $1",
    )
    .bind(row.uuid)
    .bind(status)
    .bind(error)
    .bind(DONE)
    .bind(FAILED)
    .bind(CANCELLED)
    .execute(state.database.write())
    .await?;

    Ok(())
}

async fn log_activity(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    event: &str,
    data: serde_json::Value,
) {
    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid,
            user_uuid: None,
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
        tracing::warn!(server = %server_uuid, "failed to log a failover: {err:#}");
    }
}

async fn fail(state: &crate::State, row: &ServerFailover, error: &str) -> Result<(), sqlx::Error> {
    tracing::warn!(server = %row.server_uuid, "failover of a server failed: {error}");
    set_status(state, row, FAILED, Some(error)).await?;
    log_activity(
        state,
        row.server_uuid,
        "server:failover.failed",
        serde_json::json!({ "error": error, "drill": row.mode == MODE_DRILL }),
    )
    .await;
    crate::notifications::failover_failed(state, row.server_uuid, row.source_node_uuid).await;

    Ok(())
}

/// Whether the node answered its newest check.
async fn answers(state: &crate::State, node_uuid: uuid::Uuid) -> bool {
    matches!(
        crate::node_telemetry::latest(&state.database, node_uuid).await,
        Ok(Some(telemetry)) if telemetry.reachable
    )
}

/// The newest successful backup of the server that another node can restore.
async fn newest_off_node_backup(
    state: &crate::State,
    server_uuid: uuid::Uuid,
) -> Result<(Option<ServerBackup>, Vec<uuid::Uuid>), anyhow::Error> {
    let mut backups = ServerBackup::all_by_server_uuid(&state.database, server_uuid)
        .await?
        .into_iter()
        .filter(|backup| backup.deleting.is_none())
        .collect::<Vec<_>>();
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.created));

    let mut reachable = Vec::new();
    let mut newest = None;
    for backup in backups {
        let repository = match &backup.backup_configuration {
            Some(configuration) => configuration
                .fetch_cached(&state.database)
                .await
                .ok()
                .and_then(|configuration| configuration.backup_configs.restic)
                .map(|restic| restic.repository),
            None => None,
        };
        if !off_node(backup.disk, repository.as_deref()) {
            continue;
        }

        reachable.push(backup.uuid);
        if newest.is_none()
            && backup.successful
            && backup.completed.is_some()
            && backup.kind == crate::models::server_backup::ServerBackupKind::Server
        {
            newest = Some(backup);
        }
    }

    Ok((newest, reachable))
}

/// `queued` → `rehoming`.
async fn rehome_server(state: &crate::State, row: &ServerFailover) -> Result<(), anyhow::Error> {
    let Some(server) = Server::by_uuid_optional(&state.database, row.server_uuid).await? else {
        set_status(state, row, CANCELLED, Some("the server was deleted")).await?;
        return Ok(());
    };
    if server.node.uuid != row.source_node_uuid {
        fail(state, row, "the server is no longer on this node").await?;
        return Ok(());
    }

    let (Some(backup), reachable_backups) = newest_off_node_backup(state, server.uuid).await?
    else {
        fail(state, row, NO_BACKUP).await?;
        return Ok(());
    };

    let source = server.node.fetch_cached(&state.database).await?;

    // a node that still answers (a drill, a failover by hand) must not run it a second time
    if (row.mode == MODE_DRILL || answers(state, source.uuid).await)
        && let Ok(client) = source.api_client(&state.database).await
        && let Err(err) = client
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Kill,
                    wait_seconds: None,
                },
            )
            .await
    {
        tracing::warn!(server = %server.uuid, "failed to stop a server before its failover: {err:?}");
    }

    let nodes = Node::by_location_uuids_most_eligible(
        &state.database,
        &[source.location.uuid],
        crate::server_transfers::limits(&server),
        false,
        SUSPENSION_PENALTY,
        RANDOMNESS,
    )
    .await?
    .into_iter()
    .filter(|node| node.uuid != source.uuid && node.role == source.role);

    let mut destination = None;
    for node in nodes {
        if !answers(state, node.uuid).await {
            continue;
        }
        if let Some(picked) =
            crate::server_transfers::pick_allocations(state, &server, &node).await?
        {
            destination = Some((node, picked));
            break;
        }
    }
    let Some((node, picked)) = destination else {
        fail(state, row, NO_NODE).await?;
        return Ok(());
    };

    server
        .rehome(
            state,
            RehomeOptions {
                node_uuid: node.uuid,
                allocation_uuid: picked.primary,
                allocation_uuids: picked.additional,
                variables: picked.variables,
                backup_uuids: reachable_backups,
            },
        )
        .await?;

    sqlx::query(
        "UPDATE server_failovers
        SET status = $2, destination_node_uuid = $3, backup_uuid = $4, error = NULL, updated = NOW()
        WHERE uuid = $1",
    )
    .bind(row.uuid)
    .bind(REHOMING)
    .bind(node.uuid)
    .bind(backup.uuid)
    .execute(state.database.write())
    .await?;

    if let Err(err) = create_on(state, &node, server.uuid).await {
        // tried again on the next run (`rehoming` without the server on the node)
        tracing::warn!(server = %server.uuid, node = %node.uuid, "failed to create a failed-over server: {err:#}");
    }

    tracing::info!(server = %server.uuid, from = %source.uuid, to = %node.uuid, "failing a server over");

    Ok(())
}

/// Has the node's Wings create the server, without its install script.
async fn create_on(
    state: &crate::State,
    node: &Node,
    server_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    match node
        .api_client(&state.database)
        .await?
        .post_servers(&wings_api::servers::post::RequestBody {
            uuid: server_uuid,
            start_on_completion: false,
            skip_scripts: true,
        })
        .await
    {
        Ok(_) => Ok(()),
        // it already has it (an earlier run's request went through)
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::CONFLICT =>
        {
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!("{err:?}")),
    }
}

/// `rehoming` → `restoring`: once the new node's Wings has the server, the backup is restored.
async fn restore_server(state: &crate::State, row: &ServerFailover) -> Result<(), anyhow::Error> {
    let Some(server) = Server::by_uuid_optional(&state.database, row.server_uuid).await? else {
        set_status(state, row, CANCELLED, Some("the server was deleted")).await?;
        return Ok(());
    };
    let (Some(node_uuid), Some(backup_uuid)) = (row.destination_node_uuid, row.backup_uuid) else {
        fail(state, row, "the new node or the backup was deleted").await?;
        return Ok(());
    };
    if row.minutes >= REHOME_TIMEOUT_MINUTES {
        fail(
            state,
            row,
            "the new node did not take the server within 10 minutes",
        )
        .await?;
        return Ok(());
    }

    let node = Node::by_uuid_optional(&state.database, node_uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the new node was deleted"))?;
    let client = node.api_client(&state.database).await?;

    match client.get_servers_server(server.uuid).await {
        Ok(_) => {}
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            create_on(state, &node, server.uuid).await?;
            return Ok(());
        }
        Err(err) => return Err(anyhow::anyhow!("{err:?}")),
    }

    let Some(backup) = ServerBackup::by_uuid_optional(&state.database, backup_uuid).await? else {
        fail(state, row, "the backup was deleted").await?;
        return Ok(());
    };

    let mut server = server;
    let mut transaction = state.database.write().begin().await?;
    if !server
        .try_set_status(&mut *transaction, None, Some(ServerStatus::RestoringBackup))
        .await?
    {
        // installing (Wings reports the skipped install shortly) or already restoring
        transaction.rollback().await?;
        return Ok(());
    }

    let server_uuid = server.uuid;
    if let Err(err) = backup
        .restore(
            state,
            &mut transaction,
            server,
            ServerBackupRestoreOptions {
                truncate_directory: true,
                restore_startup: false,
            },
        )
        .await
    {
        transaction.rollback().await?;
        // Wings may not be ready yet: tried again on the next run until the timeout
        tracing::warn!(server = %server_uuid, "failed to start a failover restore: {err:#}");
        return Ok(());
    }
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server_uuid).await;

    set_status(state, row, RESTORING, None).await?;

    Ok(())
}

/// `restoring` → `done` or `failed`.
async fn check_restore(state: &crate::State, row: &ServerFailover) -> Result<(), anyhow::Error> {
    let Some(server) = Server::by_uuid_optional(&state.database, row.server_uuid).await? else {
        set_status(state, row, CANCELLED, Some("the server was deleted")).await?;
        return Ok(());
    };

    match restore_outcome(server.status, row.minutes) {
        RestoreOutcome::Wait => {}
        RestoreOutcome::Failed(error) => fail(state, row, error).await?,
        RestoreOutcome::Done => {
            set_status(state, row, DONE, None).await?;
            log_activity(
                state,
                server.uuid,
                "server:failover",
                serde_json::json!({
                    "from": row.source_node_uuid,
                    "to": row.destination_node_uuid,
                    "backup": row.backup_uuid,
                    "drill": row.mode == MODE_DRILL,
                }),
            )
            .await;
            tracing::info!(server = %server.uuid, "a server was failed over");
            crate::notifications::failover_done(
                state,
                &server,
                row.backup_uuid,
                row.mode == MODE_DRILL,
            )
            .await;
        }
    }

    Ok(())
}

/// Starts an automatic failover when one is due, or in the notify mode (`notify`) reports the node
/// to the admins instead.
async fn auto_start(state: &crate::State, after: i32, notify: bool) -> Result<(), anyhow::Error> {
    // a report does not keep an automatic failover from starting once the mode is automatic
    let rows = sqlx::query(
        "SELECT nodes.uuid, node_telemetry.consecutive_outages, node_telemetry.reachable,
            (SELECT COUNT(*) FROM servers WHERE servers.node_uuid = nodes.uuid) AS servers,
            EXISTS (
                SELECT 1 FROM node_failovers
                WHERE node_failovers.node_uuid = nodes.uuid
                    AND node_failovers.created > COALESCE(node_telemetry.last_reachable, 'epoch')
                    AND ($1 OR node_failovers.mode <> $2)
            ) AS failed_over
        FROM nodes
        JOIN node_telemetry ON node_telemetry.node_uuid = nodes.uuid",
    )
    .bind(notify)
    .bind(MODE_NOTIFY)
    .fetch_all(state.database.write())
    .await?;

    let mut nodes = Vec::with_capacity(rows.len());
    for row in &rows {
        nodes.push(NodeHealth {
            node_uuid: row.try_get("uuid")?,
            outages: row.try_get("consecutive_outages")?,
            reachable: row.try_get("reachable")?,
            servers: row.try_get("servers")?,
            failed_over: row.try_get("failed_over")?,
        });
    }

    let running: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM node_failovers WHERE status = $1)")
            .bind(RUNNING)
            .fetch_one(state.database.write())
            .await?;

    // a failover by hand elsewhere does not keep another down node from being reported
    if let Some(node_uuid) = auto_pick(&nodes, after, running && !notify) {
        if notify {
            return report_down(state, node_uuid).await;
        }

        match create(state, node_uuid, MODE_AUTO, None, None).await {
            Ok(_) | Err(FailoverError::Running) | Err(FailoverError::NoServers) => {}
            Err(err) => return Err(anyhow::anyhow!(err.message())),
        }
    }

    Ok(())
}

/// Removes the old node's copy of servers failed over, once that node answers.
async fn clean_sources(state: &crate::State) -> Result<(), anyhow::Error> {
    let rows: Vec<(uuid::Uuid, uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
        "SELECT server_failovers.uuid, server_failovers.server_uuid,
            server_failovers.source_node_uuid
        FROM server_failovers
        JOIN node_telemetry ON node_telemetry.node_uuid = server_failovers.source_node_uuid
        WHERE server_failovers.status = $1
            AND server_failovers.source_cleaned IS NULL
            AND node_telemetry.reachable
        ORDER BY server_failovers.finished
        LIMIT 20",
    )
    .bind(DONE)
    .fetch_all(state.database.write())
    .await?;

    for (uuid, server_uuid, source_uuid) in rows {
        let Some(source) = Node::by_uuid_optional(&state.database, source_uuid).await? else {
            continue;
        };
        let client = source.api_client(&state.database).await?;

        let note = match Server::by_uuid_optional(&state.database, server_uuid).await? {
            // moved back since: the copy there is the live one
            Some(server) if server.node.uuid == source_uuid => {
                Some("the server is on this node again, nothing was removed")
            }
            Some(server) => {
                let remote = server.into_remote_api_object(state).await?;
                match client.post_system_orphans(&remote).await {
                    Ok(_) => None,
                    Err(wings_api::client::ApiHttpError::Http(status, _))
                        if status == reqwest::StatusCode::NOT_FOUND =>
                    {
                        Some(
                            "the node runs Wings without the fork: remove the server's files there by hand",
                        )
                    }
                    Err(err) => {
                        tracing::warn!(server = %server_uuid, node = %source_uuid, "failed to remove a failed-over server's copy: {err:?}");
                        continue;
                    }
                }
            }
            // deleted since: only a node that never restarted still knows it
            None => match client.delete_servers_server(server_uuid).await {
                Ok(_) => None,
                Err(_) => Some("the server was deleted: remove its files there by hand"),
            },
        };

        sqlx::query(
            "UPDATE server_failovers SET source_cleaned = NOW(),
                error = COALESCE($2, error)
            WHERE uuid = $1",
        )
        .bind(uuid)
        .bind(note)
        .execute(state.database.write())
        .await?;

        tracing::info!(server = %server_uuid, node = %source_uuid, "removed a failed-over server's old copy");
    }

    Ok(())
}

/// One server of a dry run: where a failover would put it and from which backup, or why it would
/// fail.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct AdminApiFailoverPlan {
    pub server_uuid: uuid::Uuid,
    pub server_name: String,
    pub destination_node_uuid: Option<uuid::Uuid>,
    pub destination_node_name: Option<String>,
    pub backup_uuid: Option<uuid::Uuid>,
    pub backup_created: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

const NO_BACKUP: &str =
    "no finished backup on storage other nodes reach (S3, or restic off the nodes)";
const NO_NODE: &str = "no other node of the location answers and has room and free ports";

/// What a failover of `node` (or a drill of `only_server`) would do now, without doing anything:
/// each server's newest off-node backup and the node it would go to ([`plan_destinations`]). The
/// real failover picks with placement's randomness and checks the ports when it moves a server, so
/// it may pick another node of the same location.
pub async fn plan(
    state: &crate::State,
    node: &Node,
    only_server: Option<uuid::Uuid>,
) -> Result<Vec<AdminApiFailoverPlan>, FailoverError> {
    let servers: Vec<(uuid::Uuid, String, i64, i64)> = sqlx::query_as(
        "SELECT uuid, name, memory, disk FROM servers
        WHERE node_uuid = $1 AND ($2::uuid IS NULL OR uuid = $2)
            AND limbo_state IS DISTINCT FROM 'parked'
            AND NOT EXISTS (SELECT 1 FROM devboxes WHERE devboxes.server_uuid = servers.uuid)
        ORDER BY name",
    )
    .bind(node.uuid)
    .bind(only_server)
    .fetch_all(state.database.read())
    .await?;
    if servers.is_empty() {
        return Err(if only_server.is_some() {
            FailoverError::NotOnNode
        } else {
            FailoverError::NoServers
        });
    }

    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"
        WITH server_usage AS (
            SELECT node_uuid,
                COALESCE(SUM((memory + memory_overhead) * CASE WHEN suspended THEN $4 ELSE 1.0 END), 0)::BIGINT AS used_memory,
                COALESCE(SUM(disk * CASE WHEN suspended THEN $4 ELSE 1.0 END), 0)::BIGINT AS used_disk
            FROM servers
            WHERE servers.limbo_state IS DISTINCT FROM 'parked'
            GROUP BY node_uuid
        )
        SELECT nodes.uuid, nodes.name, nodes.memory, nodes.disk,
            COALESCE(u.used_memory, 0) AS used_memory, COALESCE(u.used_disk, 0) AS used_disk,
            {} AS pressure, COALESCE(telemetry.reachable, FALSE) AS answers
        FROM nodes
        LEFT JOIN server_usage u ON u.node_uuid = nodes.uuid
        LEFT JOIN node_telemetry telemetry ON telemetry.node_uuid = nodes.uuid
        WHERE nodes.location_uuid = $1 AND nodes.uuid <> $2 AND nodes.role = $3
            AND nodes.deployment_enabled AND NOT nodes.maintenance_enabled
            AND NOT nodes.deployment_paused
        ORDER BY nodes.name
        "#,
        crate::node_telemetry::PRESSURE_SQL,
    )))
    .bind(node.location.uuid)
    .bind(node.uuid)
    .bind(node.role.as_str())
    .bind(SUSPENSION_PENALTY)
    .fetch_all(state.database.read())
    .await?;

    let mut names = Vec::with_capacity(rows.len());
    let mut nodes = Vec::with_capacity(rows.len());
    for row in &rows {
        names.push(row.try_get::<String, _>("name")?);
        nodes.push(PlanNode {
            uuid: row.try_get("uuid")?,
            memory: row.try_get("memory")?,
            disk: row.try_get("disk")?,
            used_memory: row.try_get("used_memory")?,
            used_disk: row.try_get("used_disk")?,
            pressure: row.try_get("pressure")?,
            answers: row.try_get("answers")?,
        });
    }

    let mut planned = Vec::with_capacity(servers.len());
    let mut sizes = Vec::new();
    for (uuid, name, memory, disk) in servers {
        let (backup, _) = newest_off_node_backup(state, uuid).await?;
        if backup.is_some() {
            sizes.push((memory, disk));
        }

        planned.push(AdminApiFailoverPlan {
            server_uuid: uuid,
            server_name: name,
            destination_node_uuid: None,
            destination_node_name: None,
            backup_uuid: backup.as_ref().map(|backup| backup.uuid),
            backup_created: backup.map(|backup| backup.created.and_utc()),
            error: None,
        });
    }

    let mut destinations = plan_destinations(&sizes, &mut nodes).into_iter();
    for entry in &mut planned {
        if entry.backup_uuid.is_none() {
            entry.error = Some(NO_BACKUP.into());
            continue;
        }

        match destinations.next().flatten() {
            Some(index) => {
                entry.destination_node_uuid = Some(nodes[index].uuid);
                entry.destination_node_name = Some(names[index].clone());
            }
            None => entry.error = Some(NO_NODE.into()),
        }
    }

    Ok(planned)
}

/// The notify mode found `node` down: the admins get what a failover would do, as a `notify` row
/// on the node's Failover tab, and nothing moves.
async fn report_down(state: &crate::State, node_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let Some(node) = Node::by_uuid_optional(&state.database, node_uuid).await? else {
        return Ok(());
    };
    let planned = match plan(state, &node, None).await {
        Ok(planned) => planned,
        Err(FailoverError::NoServers) => return Ok(()),
        Err(err) => return Err(anyhow::anyhow!(err.message())),
    };

    let mut transaction = state.database.write().begin().await?;
    let failover_uuid: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO node_failovers (node_uuid, mode, status, finished)
        VALUES ($1, $2, $3, NOW())
        RETURNING uuid",
    )
    .bind(node.uuid)
    .bind(MODE_NOTIFY)
    .bind(NOTIFIED)
    .fetch_one(&mut *transaction)
    .await?;

    for entry in &planned {
        sqlx::query(
            "INSERT INTO server_failovers (node_failover_uuid, server_uuid, server_name,
                source_node_uuid, destination_node_uuid, backup_uuid, status, error, finished)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())",
        )
        .bind(failover_uuid)
        .bind(entry.server_uuid)
        .bind(&entry.server_name)
        .bind(node.uuid)
        .bind(entry.destination_node_uuid)
        .bind(entry.backup_uuid)
        .bind(PLANNED)
        .bind(&entry.error)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;

    notify_admins_node_down(state, &node, failover_uuid, &planned).await;

    Ok(())
}

async fn notify_admins_node_down(
    state: &crate::State,
    node: &Node,
    failover_uuid: uuid::Uuid,
    planned: &[AdminApiFailoverPlan],
) {
    let restorable = planned
        .iter()
        .filter(|entry| entry.destination_node_uuid.is_some())
        .count();
    tracing::warn!(
        node = %node.name,
        servers = planned.len(),
        restorable,
        "a node is down; failover is in the notify mode, nothing was moved"
    );

    if let Err(err) = crate::models::admin_activity::AdminActivity::create(
        state,
        crate::models::admin_activity::CreateAdminActivityOptions {
            user_uuid: None,
            impersonator_uuid: None,
            api_key_uuid: None,
            event: "node:failover.notify".into(),
            ip: None,
            data: serde_json::json!({
                "uuid": node.uuid,
                "name": node.name,
                "failover": failover_uuid,
                "servers": planned.len(),
                "restorable": restorable,
            }),
            created: None,
        },
    )
    .await
    {
        tracing::warn!(node = %node.uuid, "failed to log a down node: {err:#?}");
    }
}

/// Tries a failed server of the node's failovers again: one already on its new node gets its
/// backup restored again ([`restore_server`]), one still on the old node starts over (`queued`).
/// Its failover runs again until the server is done.
pub async fn resume(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    server_failover_uuid: uuid::Uuid,
) -> Result<uuid::Uuid, FailoverError> {
    if state.settings.get().await?.recovery.failover_mode == FailoverMode::Off {
        return Err(FailoverError::Disabled);
    }

    let row: Option<(
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
        Option<uuid::Uuid>,
        Option<uuid::Uuid>,
        String,
    )> = sqlx::query_as(
        "SELECT server_failovers.node_failover_uuid, server_failovers.server_uuid,
            server_failovers.source_node_uuid, server_failovers.destination_node_uuid,
            server_failovers.backup_uuid, server_failovers.status
        FROM server_failovers
        JOIN node_failovers ON node_failovers.uuid = server_failovers.node_failover_uuid
        WHERE server_failovers.uuid = $1 AND node_failovers.node_uuid = $2",
    )
    .bind(server_failover_uuid)
    .bind(node_uuid)
    .fetch_optional(state.database.write())
    .await?;
    let Some((failover_uuid, server_uuid, source_uuid, destination_uuid, backup_uuid, status)) =
        row
    else {
        return Err(FailoverError::NotFound);
    };
    if status != FAILED {
        return Err(FailoverError::NotResumable(
            "only a server whose failover failed can be tried again",
        ));
    }

    let Some(mut server) = Server::by_uuid_optional(&state.database, server_uuid).await? else {
        return Err(FailoverError::NotResumable("the server was deleted"));
    };
    let moved = destination_uuid == Some(server.node.uuid);
    if !moved && server.node.uuid != source_uuid {
        return Err(FailoverError::NotResumable(
            "the server is on another node now",
        ));
    }
    if moved
        && match backup_uuid {
            Some(backup_uuid) => ServerBackup::by_uuid_optional(&state.database, backup_uuid)
                .await?
                .is_none(),
            None => true,
        }
    {
        return Err(FailoverError::NotResumable(
            "the backup it was restoring was deleted",
        ));
    }

    let mut transaction = state.database.write().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('node_failover:' || $1::text))")
        .bind(node_uuid)
        .execute(&mut *transaction)
        .await?;

    let other_running: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM node_failovers WHERE node_uuid = $1 AND status = $2 AND uuid <> $3
        )",
    )
    .bind(node_uuid)
    .bind(RUNNING)
    .bind(failover_uuid)
    .fetch_one(&mut *transaction)
    .await?;
    if other_running {
        return Err(FailoverError::Running);
    }

    if moved {
        // the restore that failed (or never finished) left its status behind
        if server.status.is_some() {
            server.set_status(&mut *transaction, None).await?;
        }

        sqlx::query(
            "UPDATE server_failovers
            SET status = $2, error = NULL, finished = NULL, updated = NOW()
            WHERE uuid = $1",
        )
        .bind(server_failover_uuid)
        .bind(REHOMING)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query(
            "UPDATE server_failovers
            SET status = $2, destination_node_uuid = NULL, backup_uuid = NULL, error = NULL,
                finished = NULL, updated = NOW()
            WHERE uuid = $1",
        )
        .bind(server_failover_uuid)
        .bind(QUEUED)
        .execute(&mut *transaction)
        .await?;
    }

    sqlx::query("UPDATE node_failovers SET status = $2, finished = NULL WHERE uuid = $1")
        .bind(failover_uuid)
        .bind(RUNNING)
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server.uuid).await;

    tracing::info!(server = %server.uuid, moved, "a failed failover was resumed");

    Ok(server.uuid)
}

/// The `settle_failovers` task.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let (mode, after, parallel) = {
        let settings = state.settings.get().await?;
        (
            settings.recovery.failover_mode,
            settings.recovery.failover_after,
            settings.recovery.failover_parallel.max(1) as usize,
        )
    };

    if matches!(mode, FailoverMode::Auto | FailoverMode::Notify)
        && let Err(err) = auto_start(state, after, mode == FailoverMode::Notify).await
    {
        tracing::error!("failed to start an automatic failover: {err:#}");
    }

    let rows = active_rows(state).await?;
    let mut failovers = Vec::new();
    let mut busy = std::collections::HashMap::<uuid::Uuid, usize>::new();

    for row in &rows {
        if !failovers.contains(&row.node_failover_uuid) {
            failovers.push(row.node_failover_uuid);
        }

        let result = match row.status.as_str() {
            REHOMING => restore_server(state, row).await,
            RESTORING => check_restore(state, row).await,
            _ => continue,
        };
        *busy.entry(row.node_failover_uuid).or_default() += 1;

        if let Err(err) = result {
            tracing::warn!(server = %row.server_uuid, "failover step failed, retrying: {err:#}");
        }
    }

    for row in rows.iter().filter(|row| row.status == QUEUED) {
        let running = busy.entry(row.node_failover_uuid).or_default();
        if *running >= parallel {
            continue;
        }
        *running += 1;

        if let Err(err) = rehome_server(state, row).await {
            tracing::warn!(server = %row.server_uuid, "failed to fail a server over: {err:#}");
            fail(state, row, &format!("{err:#}")).await?;
        }
    }

    for failover_uuid in failovers {
        finish_if_over(state, failover_uuid, DONE).await?;
    }

    if let Err(err) = clean_sources(state).await {
        tracing::warn!("failed to clean up failed-over servers: {err:#}");
    }

    Ok(())
}

#[derive(ToSchema, Serialize)]
pub struct AdminApiServerFailover {
    pub uuid: uuid::Uuid,
    pub server_uuid: uuid::Uuid,
    pub server_name: String,
    pub destination_node_uuid: Option<uuid::Uuid>,
    pub destination_node_name: Option<String>,
    pub backup_created: Option<chrono::DateTime<chrono::Utc>>,
    pub status: String,
    pub error: Option<String>,
    pub finished: Option<chrono::DateTime<chrono::Utc>>,
    pub source_cleaned: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, Serialize)]
pub struct AdminApiNodeFailover {
    pub uuid: uuid::Uuid,
    pub mode: String,
    pub status: String,
    pub started_by: Option<String>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub finished: Option<chrono::DateTime<chrono::Utc>>,
    pub servers: Vec<AdminApiServerFailover>,
}

/// The node's newest failovers with their servers.
pub async fn recent(
    database: &crate::database::Database,
    node_uuid: uuid::Uuid,
) -> Result<Vec<AdminApiNodeFailover>, anyhow::Error> {
    let failovers = sqlx::query(
        "SELECT node_failovers.uuid, node_failovers.mode, node_failovers.status,
            users.username AS started_by, node_failovers.created, node_failovers.finished
        FROM node_failovers
        LEFT JOIN users ON users.uuid = node_failovers.started_by
        WHERE node_failovers.node_uuid = $1
        ORDER BY node_failovers.created DESC
        LIMIT 10",
    )
    .bind(node_uuid)
    .fetch_all(database.read())
    .await?;

    let uuids = failovers
        .iter()
        .map(|row| row.try_get::<uuid::Uuid, _>("uuid"))
        .collect::<Result<Vec<_>, _>>()?;

    let servers = sqlx::query(
        "SELECT server_failovers.uuid, server_failovers.node_failover_uuid,
            server_failovers.server_uuid, server_failovers.server_name,
            server_failovers.destination_node_uuid, nodes.name AS destination_node_name,
            server_backups.created AS backup_created, server_failovers.status,
            server_failovers.error, server_failovers.finished, server_failovers.source_cleaned
        FROM server_failovers
        LEFT JOIN nodes ON nodes.uuid = server_failovers.destination_node_uuid
        LEFT JOIN server_backups ON server_backups.uuid = server_failovers.backup_uuid
        WHERE server_failovers.node_failover_uuid = ANY($1)
        ORDER BY server_failovers.server_name",
    )
    .bind(&uuids)
    .fetch_all(database.read())
    .await?;

    let at = |value: Option<chrono::NaiveDateTime>| value.map(|at| at.and_utc());

    let mut result = Vec::with_capacity(failovers.len());
    for row in &failovers {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let mut list = Vec::new();
        for server in servers.iter().filter(|server| {
            server
                .try_get::<uuid::Uuid, _>("node_failover_uuid")
                .is_ok_and(|parent| parent == uuid)
        }) {
            list.push(AdminApiServerFailover {
                uuid: server.try_get("uuid")?,
                server_uuid: server.try_get("server_uuid")?,
                server_name: server.try_get("server_name")?,
                destination_node_uuid: server.try_get("destination_node_uuid")?,
                destination_node_name: server.try_get("destination_node_name")?,
                backup_created: at(server.try_get("backup_created")?),
                status: server.try_get("status")?,
                error: server.try_get("error")?,
                finished: at(server.try_get("finished")?),
                source_cleaned: at(server.try_get("source_cleaned")?),
            });
        }

        result.push(AdminApiNodeFailover {
            uuid,
            mode: row.try_get("mode")?,
            status: row.try_get("status")?,
            started_by: row.try_get("started_by")?,
            created: row
                .try_get::<chrono::NaiveDateTime, _>("created")?
                .and_utc(),
            finished: at(row.try_get("finished")?),
            servers: list,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(outages: i32, reachable: bool, servers: i64) -> NodeHealth {
        NodeHealth {
            node_uuid: uuid::Uuid::new_v4(),
            outages,
            reachable,
            servers,
            failed_over: false,
        }
    }

    #[test]
    fn backups_on_the_node_cannot_be_restored_elsewhere() {
        assert!(off_node(BackupDisk::S3, None));
        assert!(off_node(BackupDisk::ProxmoxBackupServer, None));
        assert!(off_node(
            BackupDisk::Restic,
            Some("sftp:backup@192.0.2.10:/srv/restic")
        ));
        assert!(!off_node(BackupDisk::Restic, Some("/var/lib/restic")));
        assert!(!off_node(BackupDisk::Restic, None));
        assert!(!off_node(BackupDisk::Local, None));
        assert!(!off_node(BackupDisk::Zfs, None));
    }

    #[test]
    fn a_node_gone_long_enough_is_picked() {
        let down = node(12, false, 5);
        let nodes = vec![down.clone(), node(0, true, 3), node(0, true, 0)];

        assert_eq!(auto_pick(&nodes, 10, false), Some(down.node_uuid));
        assert_eq!(auto_pick(&nodes, 15, false), None);
        assert_eq!(auto_pick(&nodes, 10, true), None);
    }

    #[test]
    fn nothing_is_picked_when_most_nodes_look_down() {
        let nodes = vec![
            node(12, false, 5),
            node(11, false, 3),
            node(10, false, 2),
            node(0, true, 1),
        ];
        assert_eq!(auto_pick(&nodes, 10, false), None);

        // half of the others answering is enough
        let half = vec![node(12, false, 5), node(11, false, 3), node(0, true, 1)];
        assert_eq!(auto_pick(&half, 10, false), Some(half[0].node_uuid));

        let alone = vec![node(12, false, 5)];
        assert_eq!(auto_pick(&alone, 10, false), None);
    }

    #[test]
    fn empty_or_already_failed_over_nodes_are_left_alone() {
        let mut failed_over = node(30, false, 5);
        failed_over.failed_over = true;
        let nodes = vec![node(20, false, 0), failed_over, node(0, true, 1)];

        assert_eq!(auto_pick(&nodes, 10, false), None);
    }

    fn plan_node(n: u128, memory: i64, used_memory: i64, pressure: Option<f32>) -> PlanNode {
        PlanNode {
            uuid: uuid::Uuid::from_u128(n),
            memory,
            disk: 0,
            used_memory,
            used_disk: 0,
            pressure,
            answers: true,
        }
    }

    #[test]
    fn a_plan_spreads_servers_by_room_and_load() {
        let mut nodes = vec![
            plan_node(1, 1000, 0, Some(10.0)),
            plan_node(2, 1000, 400, None),
        ];

        // the first goes to the emptier node, which then is the fuller one
        assert_eq!(
            plan_destinations(&[(500, 0), (500, 0), (300, 0), (300, 0)], &mut nodes),
            vec![Some(0), Some(1), Some(0), None]
        );
        assert_eq!((nodes[0].used_memory, nodes[1].used_memory), (800, 900));
    }

    #[test]
    fn a_plan_skips_silent_busy_or_full_nodes() {
        let mut silent = plan_node(1, 0, 0, None);
        silent.answers = false;
        let mut nodes = vec![
            silent,
            plan_node(2, 0, 0, Some(90.0)),
            plan_node(3, 0, 0, Some(30.0)),
        ];
        assert_eq!(plan_destinations(&[(100, 0)], &mut nodes), vec![Some(2)]);

        let mut full_disk = plan_node(4, 0, 0, None);
        full_disk.disk = 100;
        full_disk.used_disk = 90;
        assert_eq!(
            plan_destinations(&[(1, 20), (1, 5)], &mut [full_disk]),
            vec![None, Some(0)]
        );

        assert_eq!(plan_destinations(&[(1, 1)], &mut []), vec![None]);
    }

    #[test]
    fn a_restore_ends_with_the_server_status() {
        assert_eq!(restore_outcome(None, 5), RestoreOutcome::Done);
        assert!(matches!(
            restore_outcome(Some(ServerStatus::BackupRestoreFailed), 5),
            RestoreOutcome::Failed(_)
        ));
        assert_eq!(
            restore_outcome(Some(ServerStatus::RestoringBackup), 30),
            RestoreOutcome::Wait
        );
        assert!(matches!(
            restore_outcome(Some(ServerStatus::RestoringBackup), 6 * 60),
            RestoreOutcome::Failed(_)
        ));
    }
}
