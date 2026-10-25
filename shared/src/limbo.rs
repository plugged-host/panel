//! plugged.host: limbo, start gate 10 (docs/features/limbo.md): idle servers are stopped, then
//! parked off their node until someone restores them.
//!
//! The idle clock is `servers.last_activity`: the newest server activity a person caused (a create
//! handler on `ServerActivity` keeps it, since the activity log itself is pruned). The
//! `settle_limbo` task, once a minute:
//!
//! 1. A running server idle for `stop_minutes` is stopped (`stopped`).
//! 2. A server that is off and idle for `park_minutes` gets a final backup (`parking`), locked, on
//!    storage off its node (S3, restic off the nodes); a server whose backups stay on its node is
//!    never parked. Once the backup is done, Wings deletes the server (`parked`): its row,
//!    allocations and backups stay, the node's disk is freed, and the node's room no longer
//!    counts it.
//! 3. A start of a parked server is held (gate 10) and goes to its limbo page. A restore costs
//!    tokens (fewer within `cost_window_hours` of parking; admins and a panel without tokens pay
//!    nothing), refunded when it fails. The server goes back to its node when that node answers
//!    and has room, else to a location picked on the page ([`Server::rehome`]). Wings creates it
//!    (`restoring`), the final backup is restored (`unpacking`), and it is an ordinary server
//!    again.
//!
//! 4. A server parked for `delete_after_days` (0: never) is deleted ([`purge_due`], same task);
//!    its backups go to the deleted servers' hold like any deleted server's. The limbo page says
//!    when, and a restore is refused once the time has passed.
//!
//! At most `max_per_run` stops start per run and `max_per_run` parks run at once; after 3 failed
//! parks in an hour and none that worked, parking waits for the hour to pass. A server whose park
//! failed is tried again after a day. Activity clears `stopped`; a start during `parking` cancels
//! it (the backup stays an ordinary, unlocked backup).
//!
//! A server can also be parked **on a node**: the Pelican panel parked idle servers by moving them
//! to a node it kept for them, and an import keeps them there ([`NODE_ROLE`], the node's role). Such
//! a server is `parked` without a final backup; its files are on that node ([`way`]). Its restore is
//! a move off the node to a game node (`server_transfers::limbo_request`): home (`server_limbo.
//! home_node_uuid`, on its old ports there when they are free, [`home_allocations`]) or the location
//! picked, retried on another node of the location like a player's move. The task follows the
//! move ([`move_outcome`]); a move that fails leaves the server parked where it was, its tokens
//! refunded, and nothing is ever deleted from that node. Nothing new is parked on such a node, and
//! its servers are never stopped or parked again.

use crate::{
    economy::{Currency, change},
    models::{
        ByUuid, CreatableModel, EventEmittingModel,
        node::Node,
        node_allocation::NodeAllocation,
        server::{Server, ServerEvent, ServerStatus, rehome::RehomeOptions},
        server_activity::{CreateServerActivityOptions, ServerActivity},
        server_allocation::ServerAllocation,
        server_backup::{CreateServerBackupOptions, ServerBackup, ServerBackupRestoreOptions},
        user::User,
    },
    server_transfers::{Picked, TransferError, TransferRequest},
    settings::limbo::AppSettingsLimbo,
    start_gates::{Decision, StartGate},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

/// The ledger type of a restore and of its refund.
pub const TYPE_RESTORE: &str = "limbo";
pub const TYPE_REFUND: &str = "limbo_refund";

/// How long the final backup may take before the park is given up.
const PARK_TIMEOUT_HOURS: i64 = 6;
/// How long Wings may take to create a restored server.
const CREATE_TIMEOUT_MINUTES: i64 = 15;
/// Hours before a server whose park failed is tried again.
const RETRY_HOURS: i64 = 24;
/// Failed parks in the last hour (with none that worked) that pause parking.
const BREAKER_FAILURES: i64 = 3;

/// The role of a node that keeps parked servers' files (`nodes.role`, see the module docs). It is
/// never picked for players' servers (those go to `game` nodes), and limbo parks nothing there.
pub const NODE_ROLE: &str = "limbo";

/// How a parked server comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Its files are in its final backup: Wings creates it, the backup is restored into it.
    Unpack,
    /// Its files are on the limbo node it is parked on: it is moved off that node.
    Move,
}

/// How a parked server on a node of `node_role` comes back. A server parked on a limbo node has
/// no final backup; one that has a final backup comes back from it wherever it is.
pub fn way(node_role: &str, final_backup: bool) -> Way {
    if node_role == NODE_ROLE && !final_backup {
        Way::Move
    } else {
        Way::Unpack
    }
}

/// Where a restore that moves a server off a limbo node stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    Wait,
    /// The server is on its new node: an ordinary server again.
    Done,
    /// The move gave up; the server is still on the limbo node and is parked again.
    Failed(String),
}

/// What the task does with a restore by move, from the status and error of the move it asked
/// for (`None` until that exists), whether the server is still on the limbo node and the minutes
/// since the restore began.
pub fn move_outcome(
    request: Option<(&str, Option<&str>)>,
    on_limbo_node: bool,
    minutes: i64,
) -> MoveOutcome {
    use crate::server_transfers::{CANCELLED, CANCELLING, QUEUED, RUNNING};

    match request {
        None if minutes >= CREATE_TIMEOUT_MINUTES => {
            MoveOutcome::Failed("the move off the limbo node did not start".into())
        }
        None => MoveOutcome::Wait,
        Some((QUEUED | RUNNING | CANCELLING, _)) => MoveOutcome::Wait,
        // the move is over and the files left the limbo node
        Some(_) if !on_limbo_node => MoveOutcome::Done,
        Some((CANCELLED, _)) => MoveOutcome::Failed("the move was cancelled".into()),
        Some((_, error)) => MoveOutcome::Failed(
            error
                .unwrap_or("the copy to the new node failed")
                .to_string(),
        ),
    }
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LimboState {
    /// Stopped for being idle; still on its node.
    Stopped,
    /// Off, its final backup running.
    Parking,
    /// Deleted from Wings; the row and the backup wait for a restore. Or kept as it is on a
    /// `limbo` node (NODE_ROLE), where a restore moves it from.
    Parked,
    /// Being created on a node again.
    Restoring,
    /// Its final backup being restored.
    Unpacking,
}

impl LimboState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Parking => "parking",
            Self::Parked => "parked",
            Self::Restoring => "restoring",
            Self::Unpacking => "unpacking",
        }
    }

    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value? {
            "stopped" => Some(Self::Stopped),
            "parking" => Some(Self::Parking),
            "parked" => Some(Self::Parked),
            "restoring" => Some(Self::Restoring),
            "unpacking" => Some(Self::Unpacking),
            _ => None,
        }
    }

    /// The server's files are (or are about to be) away from its node: everything but its page,
    /// its limbo page, a start and deleting it waits.
    pub fn away(self) -> bool {
        !matches!(self, Self::Stopped)
    }
}

/// The column, for `Server::base_columns`.
pub fn base_columns(prefix: &str) -> [(&'static str, compact_str::CompactString); 1] {
    [(
        "servers.limbo_state",
        compact_str::format_compact!("{prefix}limbo_state"),
    )]
}

/// Tokens a restore costs: the early price within the window after parking, the late one after.
pub fn cost(
    settings: &AppSettingsLimbo,
    parked: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> i64 {
    let late = parked.is_some_and(|parked| {
        now - parked > chrono::Duration::hours(settings.cost_window_hours as i64)
    });

    (if late {
        settings.cost_late
    } else {
        settings.cost_early
    })
    .max(0) as i64
}

/// When a server parked at `parked` is deleted (`delete_after_days`), or `None` when it is not.
pub fn deletes_at(
    settings: &AppSettingsLimbo,
    parked: Option<chrono::NaiveDateTime>,
) -> Option<chrono::NaiveDateTime> {
    if settings.delete_after_days <= 0 {
        return None;
    }

    parked.map(|parked| parked + chrono::Duration::days(settings.delete_after_days as i64))
}

/// Parked servers deleted per run at most.
const PURGE_BATCH: i64 = 20;

/// Deletes servers parked for longer than `delete_after_days` (run by `settle`, also with limbo
/// off). Only `parked` ones: a restore under way stops the clock. A server whose node cannot
/// delete it is tried again next run.
pub async fn purge_due(state: &crate::State) -> Result<usize, anyhow::Error> {
    use crate::models::DeletableModel;

    let days = state.settings.get().await?.limbo.delete_after_days;
    if days <= 0 {
        return Ok(0);
    }

    let due: Vec<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT servers.uuid FROM servers
        JOIN server_limbo ON server_limbo.server_uuid = servers.uuid
        WHERE servers.limbo_state = 'parked' AND server_limbo.parked IS NOT NULL
            AND server_limbo.parked <= NOW() - make_interval(days => $1)
        ORDER BY server_limbo.parked
        LIMIT $2
        "#,
    )
    .bind(days)
    .bind(PURGE_BATCH)
    .fetch_all(state.database.write())
    .await?;

    let mut deleted = 0;
    for server_uuid in due {
        let server = match Server::by_uuid_optional(&state.database, server_uuid).await? {
            Some(server) if server.limbo == Some(LimboState::Parked) => server,
            _ => continue,
        };

        match server
            .delete(
                state,
                crate::models::server::DeleteServerOptions { force: false },
            )
            .await
        {
            Ok(()) => {
                tracing::info!(server = %server_uuid, name = %server.name, "deleted a server parked for longer than limbo keeps it");
                deleted += 1;
            }
            Err(err) => {
                tracing::warn!(server = %server_uuid, "failed to delete a parked server: {err:#}");
            }
        }
    }

    Ok(deleted)
}

/// What to do with an idle candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Stop,
    Park,
    Nothing,
}

/// A candidate's fate from its idle minutes and whether Wings says it is running.
pub fn action(settings: &AppSettingsLimbo, idle_minutes: i64, running: bool) -> Action {
    if running {
        if idle_minutes >= settings.stop_minutes as i64 {
            Action::Stop
        } else {
            Action::Nothing
        }
    } else if idle_minutes >= settings.park_minutes as i64 {
        Action::Park
    } else {
        Action::Nothing
    }
}

/// Keeps `servers.last_activity`: every server activity a person caused (not the panel's own,
/// which has no user) restarts the idle clock and clears `stopped`.
pub fn register() {
    ServerActivity::register_create_handler(
        crate::models::ListenerPriority::Normal,
        |options, _query, _state, transaction| {
            Box::pin(async move {
                if options.user_uuid.is_none() {
                    return Ok(());
                }

                sqlx::query(
                    "UPDATE servers SET last_activity = NOW(),
                        limbo_state = CASE WHEN limbo_state = 'stopped' THEN NULL ELSE limbo_state END
                    WHERE uuid = $1
                        AND (last_activity < NOW() - INTERVAL '1 minute' OR limbo_state = 'stopped')",
                )
                .bind(options.server_uuid)
                .execute(&mut **transaction)
                .await?;

                Ok(())
            })
        },
    );

    // a server parked on a limbo node that an admin moved off it by hand (Admin › Servers ›
    // Transfer) is back: its files are on the new node
    let _ = Server::register_event_handler(|state, event| async move {
        if let ServerEvent::TransferCompleted {
            server,
            destination_node,
            successful: true,
        } = &*event
            && server.limbo == Some(LimboState::Parked)
            && server.node.fetch_cached(&state.database).await?.role == NODE_ROLE
        {
            let details = details(&state.database, server.uuid).await?;
            if way(NODE_ROLE, details.backup_uuid.is_some()) == Way::Move {
                finish_restore(&state, server, &details, destination_node.uuid).await?;
            }
        }

        Ok(())
    });
}

/// Whether limbo is creating the server on a node (the install Wings reports then is the skipped
/// one).
pub async fn creating(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM servers WHERE uuid = $1 AND limbo_state IN ('restoring', 'unpacking')
        )",
    )
    .bind(server_uuid)
    .fetch_one(database.write())
    .await
}

async fn set_state(
    executor: impl sqlx::PgExecutor<'_>,
    server_uuid: uuid::Uuid,
    state: Option<LimboState>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH changed AS (
            UPDATE servers SET limbo_state = $2 WHERE uuid = $1 RETURNING uuid
        )
        INSERT INTO server_limbo (server_uuid, since, updated)
        SELECT uuid, NOW(), NOW() FROM changed
        ON CONFLICT (server_uuid) DO UPDATE SET since = NOW(), updated = NOW()",
    )
    .bind(server_uuid)
    .bind(state.map(LimboState::as_str))
    .execute(executor)
    .await?;

    Ok(())
}

async fn log(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    user_uuid: Option<uuid::Uuid>,
    event: &'static str,
    data: serde_json::Value,
) {
    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid,
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
        tracing::warn!(server = %server_uuid, "failed to log {event}: {err:#?}");
    }
}

async fn lock_backup(
    database: &crate::database::Database,
    backup_uuid: uuid::Uuid,
    locked: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE server_backups SET locked = $2 WHERE uuid = $1")
        .bind(backup_uuid)
        .bind(locked)
        .execute(database.write())
        .await?;
    ServerBackup::invalidate_cached(database, backup_uuid).await;

    Ok(())
}

/// Start gate 10 (see the module docs).
pub struct LimboGate;

#[async_trait::async_trait]
impl StartGate for LimboGate {
    fn name(&self) -> &'static str {
        "limbo"
    }

    async fn check(
        &self,
        state: &crate::State,
        server: &Server,
        _user: &User,
        _action: wings_api::ServerPowerAction,
    ) -> Result<Decision, anyhow::Error> {
        match server.limbo {
            None | Some(LimboState::Stopped) => Ok(Decision::Pass),
            // the owner is back before the server left: it stays, the backup is an ordinary one
            Some(LimboState::Parking) if cancel_park(state, server.uuid, None).await? => {
                Ok(Decision::Pass)
            }
            // parked meanwhile (the park was already past its backup)
            Some(_) => Ok(Decision::Hold {
                url: format!("/server/{:08x}/limbo", server.uuid_short),
            }),
        }
    }
}

/// Ends a park before the server left its node: back to an ordinary server, its backup unlocked.
/// `error` records why it did not go through (a failure), `None` for a cancel. False when the
/// server is no longer `parking` (the park claimed it to delete it from Wings).
async fn cancel_park(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    error: Option<&str>,
) -> Result<bool, anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    let cancelled = sqlx::query(
        "UPDATE servers SET limbo_state = NULL WHERE uuid = $1 AND limbo_state = 'parking'",
    )
    .bind(server_uuid)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        == 1;
    if !cancelled {
        return Ok(false);
    }
    let backup_uuid: Option<uuid::Uuid> = sqlx::query_scalar(
        "UPDATE server_limbo SET error = $2, failed = CASE WHEN $2 IS NULL THEN failed ELSE NOW() END,
            since = NOW(), updated = NOW()
        WHERE server_uuid = $1 RETURNING backup_uuid",
    )
    .bind(server_uuid)
    .bind(error)
    .fetch_optional(&mut *transaction)
    .await?
    .flatten();
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server_uuid).await;

    if let Some(backup_uuid) = backup_uuid {
        lock_backup(&state.database, backup_uuid, false).await?;
    }
    if let Some(error) = error {
        tracing::warn!(server = %server_uuid, "a server could not be parked: {error}");
    }

    Ok(true)
}

/// One server's limbo details.
#[derive(Debug, Clone, Default)]
struct Details {
    since: Option<chrono::NaiveDateTime>,
    backup_uuid: Option<uuid::Uuid>,
    parked: Option<chrono::NaiveDateTime>,
    paid: i64,
    payer_uuid: Option<uuid::Uuid>,
    error: Option<String>,
    /// Parked on a limbo node: the node it came from, when it is still there.
    home_node_uuid: Option<uuid::Uuid>,
}

async fn details(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Details, sqlx::Error> {
    let row = sqlx::query(
        "SELECT since, backup_uuid, parked, paid, payer_uuid, error, home_node_uuid
        FROM server_limbo
        WHERE server_uuid = $1",
    )
    .bind(server_uuid)
    .fetch_optional(database.write())
    .await?;

    match row {
        Some(row) => Ok(Details {
            since: row.try_get("since")?,
            backup_uuid: row.try_get("backup_uuid")?,
            parked: row.try_get("parked")?,
            paid: row.try_get::<i32, _>("paid")? as i64,
            payer_uuid: row.try_get("payer_uuid")?,
            error: row.try_get("error")?,
            home_node_uuid: row.try_get("home_node_uuid")?,
        }),
        None => Ok(Details::default()),
    }
}

/// `parking` → `parked` once the final backup is done: Wings deletes the server.
async fn advance_park(state: &crate::State, server: &Server) -> Result<(), anyhow::Error> {
    let details = details(&state.database, server.uuid).await?;
    let minutes = details
        .since
        .map(|since| (chrono::Utc::now().naive_utc() - since).num_minutes())
        .unwrap_or_default();

    let backup = match details.backup_uuid {
        Some(uuid) => ServerBackup::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    };
    let Some(backup) = backup else {
        cancel_park(state, server.uuid, Some("the final backup was deleted")).await?;
        return Ok(());
    };

    if backup.completed.is_none() {
        if minutes >= PARK_TIMEOUT_HOURS * 60 {
            cancel_park(
                state,
                server.uuid,
                Some("the final backup did not finish within 6 hours"),
            )
            .await?;
            return Ok(());
        }
        return Ok(());
    }
    if !backup.successful {
        cancel_park(state, server.uuid, Some("the final backup failed")).await?;
        return Ok(());
    }

    // claimed first, so a start pressed now is held instead of cancelling a park that already
    // deletes the server; `server_limbo.parked` stays empty until Wings did, so no restore begins
    let claimed = sqlx::query(
        "UPDATE servers SET limbo_state = 'parked' WHERE uuid = $1 AND limbo_state = 'parking'",
    )
    .bind(server.uuid)
    .execute(state.database.write())
    .await?
    .rows_affected()
        == 1;
    Server::invalidate_cached(&state.database, server.uuid).await;
    if !claimed {
        return Ok(());
    }

    let node = server.node.fetch_cached(&state.database).await?;
    match node
        .api_client(&state.database)
        .await?
        .delete_servers_server(server.uuid)
        .await
    {
        Ok(_) => {}
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND => {}
        // back to `parking`, tried again on the next run
        Err(err) => {
            sqlx::query(
                "UPDATE servers SET limbo_state = 'parking' WHERE uuid = $1 AND limbo_state = 'parked'",
            )
            .bind(server.uuid)
            .execute(state.database.write())
            .await?;
            Server::invalidate_cached(&state.database, server.uuid).await;
            return Err(anyhow::anyhow!("{err:?}"));
        }
    }

    sqlx::query(
        "UPDATE server_limbo SET parked = NOW(), since = NOW(), updated = NOW(), error = NULL,
            failed = NULL
        WHERE server_uuid = $1",
    )
    .bind(server.uuid)
    .execute(state.database.write())
    .await?;

    log(
        state,
        server.uuid,
        None,
        "server:limbo.park",
        serde_json::json!({ "backup": backup.uuid, "node": node.uuid }),
    )
    .await;
    tracing::info!(server = %server.uuid, node = %node.uuid, "parked an idle server");

    Ok(())
}

/// A restore that did not work: the server is parked again and whoever paid gets the tokens
/// back. A restore from the final backup removes the half-made copy from the node it was being
/// restored on; a server parked on a limbo node keeps its files there, so nothing is removed.
async fn fail_restore(
    state: &crate::State,
    server: &Server,
    error: &str,
) -> Result<(), anyhow::Error> {
    let details = details(&state.database, server.uuid).await?;

    if let Ok(node) = server.node.fetch_cached(&state.database).await
        && way(&node.role, details.backup_uuid.is_some()) == Way::Unpack
        && let Ok(client) = node.api_client(&state.database).await
        && let Err(err) = client.delete_servers_server(server.uuid).await
    {
        tracing::warn!(server = %server.uuid, "failed to remove a failed restore's copy: {err:?}");
    }

    let mut transaction = state.database.write().begin().await?;
    set_state(&mut *transaction, server.uuid, Some(LimboState::Parked)).await?;
    sqlx::query(
        "UPDATE server_limbo SET paid = 0, payer_uuid = NULL, error = $2, failed = NOW()
        WHERE server_uuid = $1",
    )
    .bind(server.uuid)
    .bind(error)
    .execute(&mut *transaction)
    .await?;
    if details.paid > 0
        && let Some(payer) = details.payer_uuid
    {
        change(
            &mut transaction,
            Currency::Tokens,
            payer,
            details.paid,
            TYPE_REFUND,
            &server.name,
            serde_json::json!({ "server": server.uuid }),
        )
        .await?;
    }
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server.uuid).await;

    log(
        state,
        server.uuid,
        None,
        "server:limbo.restore-failed",
        serde_json::json!({ "error": error, "refunded": details.paid }),
    )
    .await;
    tracing::warn!(server = %server.uuid, "a limbo restore failed: {error}");

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
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::CONFLICT =>
        {
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!("{err:?}")),
    }
}

/// The end of a restore: an ordinary server again on `node_uuid`, its idle clock restarted, its
/// final backup (if any) unlocked.
async fn finish_restore(
    state: &crate::State,
    server: &Server,
    details: &Details,
    node_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    set_state(&mut *transaction, server.uuid, None).await?;
    sqlx::query("UPDATE servers SET last_activity = NOW() WHERE uuid = $1")
        .bind(server.uuid)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE server_limbo SET parked = NULL, paid = 0, payer_uuid = NULL, error = NULL,
            failed = NULL, home_node_uuid = NULL, home_ports = '{}'
        WHERE server_uuid = $1",
    )
    .bind(server.uuid)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server.uuid).await;
    if let Some(backup_uuid) = details.backup_uuid {
        lock_backup(&state.database, backup_uuid, false).await?;
    }

    log(
        state,
        server.uuid,
        None,
        "server:limbo.restored",
        serde_json::json!({ "node": node_uuid }),
    )
    .await;
    tracing::info!(server = %server.uuid, node = %node_uuid, "restored a parked server");

    Ok(())
}

/// The move a restore of a server parked on a limbo node asked for: `Some` when the restore is a
/// move (with the request, `None` until it exists), `None` when it unpacks the final backup.
async fn restore_move(
    state: &crate::State,
    server: &Server,
    details: &Details,
) -> Result<Option<Option<TransferRequest>>, anyhow::Error> {
    let request = TransferRequest::latest(&state.database, server.uuid)
        .await?
        .filter(|request| {
            request.origin == crate::server_transfers::ORIGIN_LIMBO
                && details.since.is_none_or(|since| request.created >= since)
        });
    if request.is_some() {
        return Ok(Some(request));
    }

    let node = server.node.fetch_cached(&state.database).await?;
    Ok((way(&node.role, details.backup_uuid.is_some()) == Way::Move).then_some(None))
}

/// `restoring` by a move off a limbo node → done, or parked again.
async fn advance_move(
    state: &crate::State,
    server: &Server,
    details: &Details,
    request: Option<TransferRequest>,
    minutes: i64,
) -> Result<(), anyhow::Error> {
    let node = server.node.fetch_cached(&state.database).await?;
    let outcome = move_outcome(
        request
            .as_ref()
            .map(|request| (request.status.as_str(), request.error.as_deref())),
        node.role == NODE_ROLE,
        minutes,
    );

    match outcome {
        MoveOutcome::Wait => Ok(()),
        MoveOutcome::Done => finish_restore(state, server, details, node.uuid).await,
        MoveOutcome::Failed(error) => fail_restore(state, server, &error).await,
    }
}

/// `restoring` → `unpacking` → done, or (parked on a limbo node) `restoring` by a move → done.
async fn advance_restore(state: &crate::State, server: Server) -> Result<(), anyhow::Error> {
    let details = details(&state.database, server.uuid).await?;
    let minutes = details
        .since
        .map(|since| (chrono::Utc::now().naive_utc() - since).num_minutes())
        .unwrap_or_default();

    if server.limbo == Some(LimboState::Restoring)
        && let Some(request) = restore_move(state, &server, &details).await?
    {
        return advance_move(state, &server, &details, request, minutes).await;
    }

    if server.limbo == Some(LimboState::Unpacking) {
        return match crate::node_failover::restore_outcome(server.status, minutes) {
            crate::node_failover::RestoreOutcome::Wait => Ok(()),
            crate::node_failover::RestoreOutcome::Failed(error) => {
                fail_restore(state, &server, error).await
            }
            crate::node_failover::RestoreOutcome::Done => {
                finish_restore(state, &server, &details, server.node.uuid).await
            }
        };
    }

    if minutes >= CREATE_TIMEOUT_MINUTES {
        return fail_restore(
            state,
            &server,
            "the node did not take the server within 15 minutes",
        )
        .await;
    }

    let Some(backup) = (match details.backup_uuid {
        Some(uuid) => ServerBackup::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    }) else {
        return fail_restore(state, &server, "the final backup was deleted").await;
    };

    let node = server.node.fetch_cached(&state.database).await?;
    match node
        .api_client(&state.database)
        .await?
        .get_servers_server(server.uuid)
        .await
    {
        Ok(_) => {}
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            return create_on(state, &node, server.uuid).await;
        }
        Err(err) => return Err(anyhow::anyhow!("{err:?}")),
    }

    let server_uuid = server.uuid;
    let mut server = server;
    let mut transaction = state.database.write().begin().await?;
    if !server
        .try_set_status(&mut *transaction, None, Some(ServerStatus::RestoringBackup))
        .await?
    {
        // Wings still reports the skipped install
        transaction.rollback().await?;
        return Ok(());
    }
    set_state(&mut *transaction, server_uuid, Some(LimboState::Unpacking)).await?;

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
        tracing::warn!(server = %server_uuid, "failed to start a limbo restore: {err:#}");
        return Ok(());
    }
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server_uuid).await;

    Ok(())
}

/// Servers limbo is moving along.
async fn in_flight(state: &crate::State) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT uuid FROM servers WHERE limbo_state IN ('parking', 'restoring', 'unpacking')",
    )
    .fetch_all(state.database.read())
    .await
}

/// Parking is paused: several parks failed in the last hour and none worked.
async fn breaker_open(state: &crate::State) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COUNT(*) FILTER (WHERE failed > NOW() - INTERVAL '1 hour' AND parked IS NULL)
                AS failed,
            COUNT(*) FILTER (WHERE parked > NOW() - INTERVAL '1 hour') AS parked
        FROM server_limbo",
    )
    .fetch_one(state.database.read())
    .await?;

    Ok(row.try_get::<i64, _>("failed")? >= BREAKER_FAILURES
        && row.try_get::<i64, _>("parked")? == 0)
}

/// An idle server the task may stop or park.
struct Candidate {
    uuid: uuid::Uuid,
    node_uuid: uuid::Uuid,
    idle_minutes: i64,
}

async fn candidates(
    state: &crate::State,
    settings: &AppSettingsLimbo,
    exempt_members: bool,
) -> Result<Vec<Candidate>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT servers.uuid, servers.node_uuid,
            (EXTRACT(EPOCH FROM NOW() - servers.last_activity) / 60)::BIGINT AS idle_minutes
        FROM servers
        JOIN users ON users.uuid = servers.owner_uuid
        LEFT JOIN server_limbo ON server_limbo.server_uuid = servers.uuid
        WHERE NOT servers.suspended
            AND servers.status IS NULL
            AND servers.destination_node_uuid IS NULL
            AND (servers.limbo_state IS NULL OR servers.limbo_state = 'stopped')
            AND NOT users.admin
            AND NOT EXISTS (SELECT 1 FROM devboxes WHERE devboxes.server_uuid = servers.uuid)
            AND NOT EXISTS (
                SELECT 1 FROM nodes WHERE nodes.uuid = servers.node_uuid AND nodes.role = $4
            )
            AND servers.last_activity < NOW() - make_interval(mins => $1)
            AND (server_limbo.failed IS NULL OR server_limbo.failed < NOW() - make_interval(hours => $2))
            AND NOT ($3 AND EXISTS (
                SELECT 1 FROM user_memberships
                WHERE user_memberships.user_uuid = users.uuid
                    AND (user_memberships.plus_since IS NOT NULL
                        OR user_memberships.plusplus_until > NOW())
            ))
        ORDER BY servers.last_activity
        LIMIT 500",
    )
    .bind(settings.stop_minutes.min(settings.park_minutes))
    .bind(RETRY_HOURS as i32)
    .bind(exempt_members)
    .bind(NODE_ROLE)
    .fetch_all(state.database.read())
    .await?;

    rows.iter()
        .map(|row| {
            Ok(Candidate {
                uuid: row.try_get("uuid")?,
                node_uuid: row.try_get("node_uuid")?,
                idle_minutes: row.try_get("idle_minutes")?,
            })
        })
        .collect()
}

/// Stops a running idle server.
async fn stop(
    state: &crate::State,
    node: &Node,
    server_uuid: uuid::Uuid,
    idle_minutes: i64,
) -> Result<(), anyhow::Error> {
    node.api_client(&state.database)
        .await?
        .post_servers_server_power(
            server_uuid,
            &wings_api::servers_server_power::post::RequestBody {
                action: wings_api::ServerPowerAction::Stop,
                wait_seconds: None,
            },
        )
        .await?;

    set_state(
        state.database.write(),
        server_uuid,
        Some(LimboState::Stopped),
    )
    .await?;
    Server::invalidate_cached(&state.database, server_uuid).await;
    log(
        state,
        server_uuid,
        None,
        "server:limbo.stop",
        serde_json::json!({ "idle_minutes": idle_minutes }),
    )
    .await;

    Ok(())
}

/// Starts parking an idle server that is off: its final backup, locked. False when its backups
/// stay on its node (it cannot be parked).
async fn park(
    state: &crate::State,
    server: &Server,
    idle_minutes: i64,
) -> Result<bool, anyhow::Error> {
    let Some(configuration) = server.backup_configuration(&state.database).await else {
        return Ok(false);
    };
    let repository = configuration
        .backup_configs
        .restic
        .as_ref()
        .map(|restic| restic.repository.clone());
    if !crate::node_failover::off_node(configuration.backup_disk, repository.as_deref())
        || configuration.maintenance_enabled
    {
        return Ok(false);
    }

    let metadata = ServerBackup::generate_metadata(state, server).await?;
    let backup = ServerBackup::create(
        state,
        CreateServerBackupOptions {
            server,
            name: ServerBackup::default_name(),
            backup_group_uuid: None,
            system_backup_policy_uuid: None,
            database_instance: None,
            backup_configuration: Some(configuration),
            ignored_files: Vec::new(),
            metadata,
        },
    )
    .await?;
    lock_backup(&state.database, backup.uuid, true).await?;

    let mut transaction = state.database.write().begin().await?;
    set_state(&mut *transaction, server.uuid, Some(LimboState::Parking)).await?;
    sqlx::query("UPDATE server_limbo SET backup_uuid = $2, error = NULL WHERE server_uuid = $1")
        .bind(server.uuid)
        .bind(backup.uuid)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server.uuid).await;

    log(
        state,
        server.uuid,
        None,
        "server:limbo.parking",
        serde_json::json!({ "idle_minutes": idle_minutes, "backup": backup.uuid }),
    )
    .await;

    Ok(true)
}

/// The `settle_limbo` task (see the module docs). Moves along what is under way even with limbo
/// off; starts nothing new then.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    for uuid in in_flight(state).await? {
        let Some(server) = Server::by_uuid_optional(&state.database, uuid).await? else {
            continue;
        };
        let result = match server.limbo {
            Some(LimboState::Parking) => advance_park(state, &server).await,
            Some(LimboState::Restoring | LimboState::Unpacking) => {
                advance_restore(state, server).await
            }
            _ => Ok(()),
        };
        if let Err(err) = result {
            tracing::warn!(server = %uuid, "failed to move a limbo server along: {err:#}");
        }
    }

    if let Err(err) = purge_due(state).await {
        tracing::warn!("failed to delete servers parked too long: {err:#}");
    }

    let settings = state.settings.get().await?;
    let limbo = settings.limbo.clone();
    let exempt_members = limbo.exempt_members && settings.economy.membership_enabled;
    drop(settings);
    if !limbo.enabled {
        return Ok(());
    }

    let parking: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM servers WHERE limbo_state = 'parking'")
            .fetch_one(state.database.read())
            .await?;
    let mut parks = if breaker_open(state).await? {
        0
    } else {
        (limbo.max_per_run as i64 - parking).max(0)
    };
    let mut stops = limbo.max_per_run.max(0) as i64;

    let candidates = candidates(state, &limbo, exempt_members).await?;
    let mut by_node: HashMap<uuid::Uuid, Vec<Candidate>> = HashMap::new();
    for candidate in candidates {
        by_node
            .entry(candidate.node_uuid)
            .or_default()
            .push(candidate);
    }

    for (node_uuid, candidates) in by_node {
        if parks == 0 && stops == 0 {
            break;
        }

        let Some(node) = Node::by_uuid_optional(&state.database, node_uuid).await? else {
            continue;
        };
        // a node that does not answer tells nothing about its servers
        let Ok(usage) = node.fetch_server_resources(&state.database).await else {
            continue;
        };

        for candidate in candidates {
            let running = usage
                .get(&candidate.uuid)
                .is_some_and(|usage| !matches!(usage.state, wings_api::ServerState::Offline));

            match action(&limbo, candidate.idle_minutes, running) {
                Action::Stop if stops > 0 => {
                    stops -= 1;
                    if let Err(err) =
                        stop(state, &node, candidate.uuid, candidate.idle_minutes).await
                    {
                        tracing::warn!(server = %candidate.uuid, "failed to stop an idle server: {err:#}");
                    }
                }
                Action::Park if parks > 0 => {
                    let Some(server) =
                        Server::by_uuid_optional(&state.database, candidate.uuid).await?
                    else {
                        continue;
                    };
                    match park(state, &server, candidate.idle_minutes).await {
                        Ok(true) => parks -= 1,
                        Ok(false) => {}
                        Err(err) => {
                            parks -= 1;
                            tracing::warn!(server = %candidate.uuid, "failed to start parking an idle server: {err:#}");
                            sqlx::query(
                                "INSERT INTO server_limbo (server_uuid, error, failed, updated)
                                VALUES ($1, $2, NOW(), NOW())
                                ON CONFLICT (server_uuid) DO UPDATE SET error = EXCLUDED.error,
                                    failed = NOW(), updated = NOW()",
                            )
                            .bind(candidate.uuid)
                            .bind(format!("{err:#}"))
                            .execute(state.database.write())
                            .await?;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum LimboError {
    /// The server is not parked.
    NotParked,
    /// Its node is full or does not answer: a location has to be picked.
    HomeUnavailable,
    /// The location is not open to the account or has no room.
    LocationUnavailable,
    /// Not enough tokens.
    Short,
    /// The limbo node that has the server's files does not answer.
    FilesUnreachable,
    /// It was parked for longer than limbo keeps servers: it is being deleted.
    Expired,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for LimboError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for LimboError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl From<sqlx::Error> for LimboError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl LimboError {
    pub fn message(&self) -> String {
        match self {
            Self::NotParked => "the server is not parked".into(),
            Self::HomeUnavailable => {
                "the server's node has no room right now, pick a location".into()
            }
            Self::LocationUnavailable => "that location cannot take the server right now".into(),
            Self::Short => "not enough tokens".into(),
            Self::FilesUnreachable => {
                "the node that keeps the server's files does not answer right now, try again later"
                    .into()
            }
            Self::Expired => "the server was parked for too long and is being deleted".into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotParked
            | Self::HomeUnavailable
            | Self::LocationUnavailable
            | Self::Short
            | Self::FilesUnreachable
            | Self::Expired => 409,
            Self::Other(_) => 500,
        }
    }
}

/// Whether the node answers (its newest sample).
async fn answers(state: &crate::State, node_uuid: uuid::Uuid) -> bool {
    matches!(
        crate::node_telemetry::latest(&state.database, node_uuid).await,
        Ok(Some(telemetry)) if telemetry.reachable
    )
}

/// Whether the server fits back on its own node.
async fn home_available(state: &crate::State, server: &Server) -> Result<bool, anyhow::Error> {
    let node = server.node.fetch_cached(&state.database).await?;
    if !answers(state, node.uuid).await {
        return Ok(false);
    }

    let eligible = Node::by_location_uuids_most_eligible(
        &state.database,
        &[node.location.uuid],
        crate::server_transfers::limits(server),
        false,
        crate::server_creation::SUSPENSION_PENALTY,
        0.0,
    )
    .await?;

    Ok(eligible.iter().any(|eligible| eligible.uuid == node.uuid))
}

/// Whether the owner may bring the server back in the location: one they may create servers in,
/// with room.
async fn location_open(
    state: &crate::State,
    server: &Server,
    location_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    Ok(crate::server_creation::locations(state, &server.owner)
        .await?
        .into_iter()
        .any(|location| location.uuid == location_uuid && location.unlocked && location.available))
}

/// Whether a game node of the location answers and has room for the server.
async fn location_has_room(
    state: &crate::State,
    server: &Server,
    location_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    let nodes = Node::by_location_uuids_most_eligible(
        &state.database,
        &[location_uuid],
        crate::server_transfers::limits(server),
        false,
        crate::server_creation::SUSPENSION_PENALTY,
        0.0,
    )
    .await?;

    for node in nodes {
        if node.role == crate::server_creation::GAME_ROLE && answers(state, node.uuid).await {
            return Ok(true);
        }
    }

    Ok(false)
}

/// The home of a server parked on a limbo node: the node it came from, when that still exists,
/// and whether a game node of its location has room for the server now.
async fn move_home(
    state: &crate::State,
    server: &Server,
    details: &Details,
) -> Result<Option<(Node, bool)>, anyhow::Error> {
    let Some(home_uuid) = details.home_node_uuid else {
        return Ok(None);
    };
    let Some(home) = Node::by_uuid_optional(&state.database, home_uuid).await? else {
        return Ok(None);
    };
    let room = location_has_room(state, server, home.location.uuid).await?;

    Ok(Some((home, room)))
}

/// The old ports of a server coming back from a limbo node to its home node, when `node` is that
/// node, has them all free on one address and the server has as many allocations as it had there:
/// its address is then the one it had before it was parked, as with the Pelican panel's restore.
/// The egg variables that held one of its ports on the limbo node follow. Asked by
/// `server_transfers::pick_allocations` before anything else.
pub(crate) async fn home_allocations(
    state: &crate::State,
    server: &Server,
    node: &Node,
) -> Result<Option<Picked>, anyhow::Error> {
    if server.limbo != Some(LimboState::Restoring) {
        return Ok(None);
    }

    let ports: Option<Vec<i32>> = sqlx::query_scalar(
        "SELECT home_ports FROM server_limbo WHERE server_uuid = $1 AND home_node_uuid = $2",
    )
    .bind(server.uuid)
    .bind(node.uuid)
    .fetch_optional(state.database.read())
    .await?;
    let Some(ports) = ports.filter(|ports| !ports.is_empty()) else {
        return Ok(None);
    };
    // one old port per allocation it has now, else the move picks them as for any server
    let current = crate::allocations::move_order(
        server,
        ServerAllocation::all_by_server_uuid(&state.database, server.uuid).await?,
    )
    .into_iter()
    .map(|allocation| allocation.allocation.port)
    .collect::<Vec<_>>();
    if current.len() != ports.len() {
        return Ok(None);
    }

    let used = NodeAllocation::used_by_node_any_ip(state, node).await?;
    let Some((_, preserved)) =
        NodeAllocation::get_preserved(&state.database, node.uuid, &ports, &used).await?
    else {
        return Ok(None);
    };
    let mut by_port: HashMap<i32, Vec<uuid::Uuid>> = HashMap::new();
    for (uuid, port) in preserved {
        by_port.entry(port).or_default().push(uuid);
    }
    let Some(kept) = ports
        .iter()
        .map(|port| by_port.get_mut(port).and_then(Vec::pop))
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };

    let variables =
        crate::allocations::followed_variables(&state.database, server, &current, &ports).await?;

    let mut kept = kept.into_iter();
    let primary = if server.allocation.is_some() {
        kept.next()
    } else {
        None
    };

    Ok(Some(Picked {
        primary,
        additional: kept.collect(),
        variables,
    }))
}

/// Charges the restore and claims the server (`restoring`) in one transaction, so that a restore
/// cannot be paid twice.
async fn claim(
    state: &crate::State,
    server: &Server,
    user: &User,
    cost: i64,
) -> Result<(), LimboError> {
    let mut transaction = state.database.write().begin().await?;
    if cost > 0
        && change(
            &mut transaction,
            Currency::Tokens,
            user.uuid,
            -cost,
            TYPE_RESTORE,
            &server.name,
            serde_json::json!({ "server": server.uuid }),
        )
        .await?
        .is_none()
    {
        return Err(LimboError::Short);
    }
    let claimed = sqlx::query(
        "UPDATE servers SET limbo_state = 'restoring' WHERE uuid = $1 AND limbo_state = 'parked'
            AND EXISTS (
                SELECT 1 FROM server_limbo WHERE server_uuid = $1 AND parked IS NOT NULL
            )",
    )
    .bind(server.uuid)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Err(LimboError::NotParked);
    }
    set_state(&mut *transaction, server.uuid, Some(LimboState::Restoring)).await?;
    sqlx::query(
        "UPDATE server_limbo SET paid = $2, payer_uuid = $3, error = NULL WHERE server_uuid = $1",
    )
    .bind(server.uuid)
    .bind(cost as i32)
    .bind(user.uuid)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server.uuid).await;

    Ok(())
}

/// [`restore`] of a server parked on a limbo node: a move off that node to the best game node of
/// its home location (its home node first), or of the location picked (see the module docs).
async fn restore_by_move(
    state: &crate::State,
    server: &Server,
    user: &User,
    location_uuid: Option<uuid::Uuid>,
    admin: bool,
    details: &Details,
    cost: i64,
) -> Result<(), LimboError> {
    // the files are on that node: it has to answer to send them
    let node = server.node.fetch_cached(&state.database).await?;
    if !answers(state, node.uuid).await {
        return Err(LimboError::FilesUnreachable);
    }

    let home = move_home(state, server, details).await?;
    let (destination, prefer) = match location_uuid {
        None => match home {
            Some((home, true)) => (home.location.uuid, Some(home.uuid)),
            _ => return Err(LimboError::HomeUnavailable),
        },
        Some(location_uuid) => {
            if (!admin && !location_open(state, server, location_uuid).await?)
                || !location_has_room(state, server, location_uuid).await?
            {
                return Err(LimboError::LocationUnavailable);
            }
            (location_uuid, home.map(|(home, _)| home.uuid))
        }
    };

    claim(state, server, user, cost).await?;

    let request =
        match crate::server_transfers::limbo_request(state, server, user.uuid, destination, prefer)
            .await
        {
            Ok(request) => request,
            Err(err) => {
                // nothing left the node: parked again where it was, the tokens back
                fail_restore(state, server, &err.message()).await?;

                return Err(match err {
                    TransferError::NoNode(_) if location_uuid.is_none() => {
                        LimboError::HomeUnavailable
                    }
                    TransferError::NoNode(_) => LimboError::LocationUnavailable,
                    err => LimboError::Other(anyhow::anyhow!(err.message())),
                });
            }
        };

    log(
        state,
        server.uuid,
        Some(user.uuid),
        "server:limbo.restore",
        serde_json::json!({
            "node": request.destination_node_uuid,
            "cost": cost,
            "request": request.uuid,
        }),
    )
    .await;

    Ok(())
}

/// Restores a parked server, home or (`location_uuid`) to a node of that location; `user` pays
/// unless `admin`.
pub async fn restore(
    state: &crate::State,
    server: &Server,
    user: &User,
    location_uuid: Option<uuid::Uuid>,
    admin: bool,
) -> Result<(), LimboError> {
    if server.limbo != Some(LimboState::Parked) {
        return Err(LimboError::NotParked);
    }

    let details = details(&state.database, server.uuid).await?;
    let settings = state.settings.get().await?;
    let limbo = settings.limbo.clone();
    let tokens_enabled = settings.economy.tokens_enabled;
    drop(settings);

    let now = chrono::Utc::now().naive_utc();
    if deletes_at(&limbo, details.parked).is_some_and(|at| at <= now) {
        return Err(LimboError::Expired);
    }
    let cost = if admin || !tokens_enabled {
        0
    } else {
        cost(&limbo, details.parked, now)
    };

    let current = server.node.fetch_cached(&state.database).await?;
    if way(&current.role, details.backup_uuid.is_some()) == Way::Move {
        return restore_by_move(state, server, user, location_uuid, admin, &details, cost).await;
    }

    // where it goes: home, or the best node of the location picked
    let destination = match location_uuid {
        None if home_available(state, server).await? => None,
        None => return Err(LimboError::HomeUnavailable),
        Some(location_uuid) => {
            if !admin && !location_open(state, server, location_uuid).await? {
                return Err(LimboError::LocationUnavailable);
            }

            let nodes = Node::by_location_uuids_most_eligible(
                &state.database,
                &[location_uuid],
                crate::server_transfers::limits(server),
                false,
                crate::server_creation::SUSPENSION_PENALTY,
                crate::server_creation::RANDOMNESS,
            )
            .await?
            .into_iter()
            .filter(|node| node.role == current.role);

            let mut picked = None;
            for node in nodes {
                if !answers(state, node.uuid).await {
                    continue;
                }
                if node.uuid == current.uuid {
                    picked = Some(None);
                    break;
                }
                if let Some(allocations) =
                    crate::server_transfers::pick_allocations(state, server, &node).await?
                {
                    picked = Some(Some((node, allocations)));
                    break;
                }
            }

            picked.ok_or(LimboError::LocationUnavailable)?
        }
    };

    claim(state, server, user, cost).await?;

    let node = match destination {
        None => server.node.fetch_cached(&state.database).await?,
        Some((node, picked)) => {
            let backups = details.backup_uuid.into_iter().collect();
            if let Err(err) = server
                .rehome(
                    state,
                    RehomeOptions {
                        node_uuid: node.uuid,
                        allocation_uuid: picked.primary,
                        allocation_uuids: picked.additional,
                        variables: picked.variables,
                        backup_uuids: backups,
                    },
                )
                .await
            {
                fail_restore(
                    state,
                    server,
                    "the server could not be moved to the new node",
                )
                .await?;
                return Err(err.into());
            }
            node
        }
    };

    log(
        state,
        server.uuid,
        Some(user.uuid),
        "server:limbo.restore",
        serde_json::json!({ "node": node.uuid, "cost": cost }),
    )
    .await;

    // the task takes it from here; asking Wings now saves the first minute
    if let Err(err) = create_on(state, &node, server.uuid).await {
        tracing::warn!(server = %server.uuid, node = %node.uuid, "failed to create a restored server: {err:#}");
    }

    Ok(())
}

/// What the limbo page shows.
#[derive(ToSchema, Serialize)]
pub struct ApiLimbo {
    pub state: Option<LimboState>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub parked: Option<chrono::DateTime<chrono::Utc>>,
    /// When the server is deleted unless it is restored first (`delete_after_days`).
    pub delete_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Tokens the restore costs this user.
    pub cost: i64,
    /// The user's tokens; `None` with tokens off.
    pub tokens: Option<i64>,
    /// The server fits back on its node.
    pub home_available: bool,
    pub home_location_uuid: uuid::Uuid,
    pub locations: Vec<crate::server_creation::ApiCreateLocation>,
    /// Why the last park or restore did not work.
    pub error: Option<String>,
}

pub async fn view(
    state: &crate::State,
    server: &Server,
    user: &User,
    admin: bool,
) -> Result<ApiLimbo, anyhow::Error> {
    let details = details(&state.database, server.uuid).await?;
    let settings = state.settings.get().await?;
    let limbo = settings.limbo.clone();
    let tokens_enabled = settings.economy.tokens_enabled;
    drop(settings);

    let last_activity: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT last_activity FROM servers WHERE uuid = $1")
            .bind(server.uuid)
            .fetch_optional(state.database.read())
            .await?;
    let tokens: Option<i64> = if tokens_enabled {
        sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1")
            .bind(user.uuid)
            .fetch_optional(state.database.write())
            .await?
    } else {
        None
    };

    let parked = server.limbo == Some(LimboState::Parked);
    let node = server.node.fetch_cached(&state.database).await?;
    // parked on a limbo node, home is the location of the node it came from
    let (home_available, home_location_uuid) = if !parked {
        (false, node.location.uuid)
    } else if way(&node.role, details.backup_uuid.is_some()) == Way::Move {
        match move_home(state, server, &details).await? {
            Some((home, room)) => (room, home.location.uuid),
            None => (false, node.location.uuid),
        }
    } else {
        (home_available(state, server).await?, node.location.uuid)
    };

    Ok(ApiLimbo {
        state: server.limbo,
        last_activity: last_activity.map(|at| at.and_utc()),
        parked: details.parked.map(|at| at.and_utc()),
        delete_at: if parked {
            deletes_at(&limbo, details.parked).map(|at| at.and_utc())
        } else {
            None
        },
        cost: if admin || !tokens_enabled {
            0
        } else {
            cost(&limbo, details.parked, chrono::Utc::now().naive_utc())
        },
        tokens,
        home_available,
        home_location_uuid,
        locations: if parked {
            crate::server_creation::locations(state, &server.owner).await?
        } else {
            Vec::new()
        },
        error: details.error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            + chrono::Duration::hours(hour as i64)
    }

    #[test]
    fn states_round_trip_and_only_stopped_stays_home() {
        for state in [
            LimboState::Stopped,
            LimboState::Parking,
            LimboState::Parked,
            LimboState::Restoring,
            LimboState::Unpacking,
        ] {
            assert_eq!(LimboState::parse(Some(state.as_str())), Some(state));
            assert_eq!(state.away(), state != LimboState::Stopped);
        }
        assert_eq!(LimboState::parse(None), None);
        assert_eq!(LimboState::parse(Some("limbo")), None);
    }

    #[test]
    fn running_servers_stop_and_stopped_ones_park() {
        let settings = AppSettingsLimbo::default();

        assert_eq!(action(&settings, 48 * 60 - 1, true), Action::Nothing);
        assert_eq!(action(&settings, 48 * 60, true), Action::Stop);
        assert_eq!(action(&settings, 500 * 60, true), Action::Stop);
        assert_eq!(action(&settings, 72 * 60 - 1, false), Action::Nothing);
        assert_eq!(action(&settings, 72 * 60, false), Action::Park);
    }

    #[test]
    fn a_parked_server_is_deleted_after_the_days_set_and_never_with_0() {
        let off = AppSettingsLimbo::default();
        assert_eq!(deletes_at(&off, Some(at(0))), None);

        let week = AppSettingsLimbo {
            delete_after_days: 7,
            ..Default::default()
        };
        assert_eq!(deletes_at(&week, Some(at(0))), Some(at(7 * 24)));
        assert_eq!(deletes_at(&week, None), None);
    }

    #[test]
    fn a_restore_costs_more_after_the_window() {
        let settings = AppSettingsLimbo::default();

        assert_eq!(cost(&settings, Some(at(0)), at(72)), 5);
        assert_eq!(cost(&settings, Some(at(0)), at(73)), 10);
        assert_eq!(cost(&settings, None, at(500)), 5);

        let negative = AppSettingsLimbo {
            cost_early: -3,
            ..Default::default()
        };
        assert_eq!(cost(&negative, Some(at(0)), at(1)), 0);
    }

    #[test]
    fn only_a_server_on_a_limbo_node_without_a_backup_comes_back_by_a_move() {
        assert_eq!(way(NODE_ROLE, false), Way::Move);
        // parked the panel's way somewhere: its final backup has the files
        assert_eq!(way(NODE_ROLE, true), Way::Unpack);
        assert_eq!(way("game", false), Way::Unpack);
        assert_eq!(way("game", true), Way::Unpack);
        assert_eq!(way("internal", false), Way::Unpack);
        assert!(crate::models::node::placement::ROLES.contains(&NODE_ROLE));
    }

    #[test]
    fn a_move_waits_for_its_request_and_never_gives_up_files_it_moved() {
        use crate::server_transfers::{CANCELLED, CANCELLING, COMPLETED, FAILED, QUEUED, RUNNING};

        // not asked for yet: waits, then gives up without having moved anything
        assert_eq!(move_outcome(None, true, 0), MoveOutcome::Wait);
        assert_eq!(
            move_outcome(None, true, CREATE_TIMEOUT_MINUTES - 1),
            MoveOutcome::Wait
        );
        assert!(matches!(
            move_outcome(None, true, CREATE_TIMEOUT_MINUTES),
            MoveOutcome::Failed(_)
        ));

        // under way, however long it takes (the move has its own stall watch)
        for status in [QUEUED, RUNNING, CANCELLING] {
            assert_eq!(
                move_outcome(Some((status, None)), true, 600),
                MoveOutcome::Wait
            );
        }

        assert_eq!(
            move_outcome(Some((COMPLETED, None)), false, 3),
            MoveOutcome::Done
        );
        // over, and the files left the limbo node anyway (moved meanwhile)
        assert_eq!(
            move_outcome(Some((FAILED, Some("x"))), false, 3),
            MoveOutcome::Done
        );

        // over, still on the limbo node: parked again with why
        assert_eq!(
            move_outcome(Some((FAILED, Some("no node can take the server"))), true, 3),
            MoveOutcome::Failed("no node can take the server".into())
        );
        assert_eq!(
            move_outcome(Some((FAILED, None)), true, 3),
            MoveOutcome::Failed("the copy to the new node failed".into())
        );
        assert_eq!(
            move_outcome(Some((CANCELLED, None)), true, 3),
            MoveOutcome::Failed("the move was cancelled".into())
        );
        assert!(matches!(
            move_outcome(Some((COMPLETED, None)), true, 3),
            MoveOutcome::Failed(_)
        ));
    }
}
