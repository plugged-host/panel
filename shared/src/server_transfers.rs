//! plugged.host: moving a server to another node, asked for by its owner on the server's Transfer
//! page or by an admin's "Lighten load" on a node (docs/features/transfers.md).
//!
//! The copy is the native transfer (`Server::transfer`): Wings stops the server, streams it and its
//! node-local backups to the new node, and the panel moves the row once the new node confirms.
//! Around it this adds the choice of node and ports, a cooldown, retries on another node of the
//! location, a cancel, queued moves for "Lighten load", and starting the server again when it was
//! running. Each move is a row in `server_transfer_requests`; [`settle`] (the `settle_transfers`
//! task) moves it along once the native transfer is over, and cancels a copy that stopped moving
//! (or whose new node never confirmed it) so that it ends like a failed one ([`watch_copy`]). What
//! happens to a move is also written to the server's activity log (`server:transfer.*`, with the
//! request's uuid), which the Transfer page shows as the move's timeline.

use crate::{
    models::{
        ByUuid, CreatableModel, EventEmittingModel,
        egg_configuration::{EggConfiguration, EggConfigurationTarget},
        location::Location,
        nest_egg_variable::NestEggVariable,
        node::Node,
        node_allocation::NodeAllocation,
        server::{AdminApiServerLimits, Server, ServerEvent, ServerStatus, ServerTransferOptions},
        server_activity::{CreateServerActivityOptions, ServerActivity},
        server_allocation::ServerAllocation,
        server_backup::ServerBackup,
        server_variable::ServerVariable,
        user::User,
    },
    server_creation::{ApiCreateLocation, GAME_ROLE, RANDOMNESS, SUSPENSION_PENALTY},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{LazyLock, Mutex},
};
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
/// plugged.host: limbo's restore of a server parked on a limbo node ([`limbo_request`])
pub const ORIGIN_LIMBO: &str = "limbo";

/// How long the Transfer page shows how the last move ended (and its timeline), in minutes.
pub const OUTCOME_MINUTES: i64 = 10;
/// How long the strip on every server page shows it, in minutes.
pub const STRIP_MINUTES: i64 = 2;
/// Finished moves the Transfer page lists under "Earlier moves".
const HISTORY: i64 = 6;
/// Timeline lines kept per move.
const TIMELINE: i64 = 40;

/// The errors this module writes into a request's `error`, which [`reason_key`] names.
const NO_NODE: &str = "no node can take the server right now";
const COPY_FAILED: &str = "the copy to the new node failed on every node tried";
const MOVED_MEANWHILE: &str = "the server moved or changed meanwhile";

/// Why [`settle`] cancelled an attempt, the `reason` of its `server:transfer.attempt-failed` line
/// (a key of `pages.server.transfer.reasons`): no copy progress for the stall time, or no
/// confirmation from the new node for the finalize time after the last byte.
pub const REASON_STALLED: &str = "stalled";
pub const REASON_UNFINALIZED: &str = "unfinalized";
/// A cancel asked by the owner or an admin, which Wings did not report either.
const REASON_CANCELLED: &str = "cancelled";
/// Seconds Wings gets to report a cancelled copy before [`settle`] releases the server itself.
const ABORT_GRACE_SECONDS: i64 = 120;
/// A copy whose old node does not answer the panel may still be streaming: it gets the stall time
/// plus its size at this rate (the Pelican panel's watcher without its socket does the same).
const SILENT_RATE: u64 = 5 * 1024 * 1024;

/// The copy speed the offline estimate assumes (the Pelican panel's figure before it measured a
/// move), and the seconds it adds for the stop, the check on the new node and the start.
const ESTIMATE_RATE: u64 = 40 * 1024 * 1024;
const ESTIMATE_OVERHEAD: u64 = 45;

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
            reason: self.error.as_deref().and_then(reason_key).map(Into::into),
            error: self.error,
            from_location: None,
            to_location: None,
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

    /// The server's last finished moves, newest first.
    async fn history(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM server_transfer_requests
            WHERE server_transfer_requests.server_uuid = $1
                AND server_transfer_requests.status = ANY($2)
            ORDER BY server_transfer_requests.created DESC
            LIMIT $3"
        )))
        .bind(server_uuid)
        .bind(&[COMPLETED, FAILED, CANCELLED][..])
        .bind(HISTORY)
        .fetch_all(database.read())
        .await?;

        rows.iter().map(Self::map).collect()
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

/// A location as the Transfer page shows it, also one players cannot pick (an admin's move).
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiTransferPlace {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub flag: Option<compact_str::CompactString>,
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
    /// Why it ended, a key of `pages.server.transfer.reasons`; `null` when `error` is something else.
    pub reason: Option<compact_str::CompactString>,
    /// The location of the node it left and the one it goes to (the Transfer page fills these in).
    pub from_location: Option<ApiTransferPlace>,
    pub to_location: Option<ApiTransferPlace>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub finished: Option<chrono::DateTime<chrono::Utc>>,
}

/// Where a move under way is: the Transfer page's progress card and the strip on every server page.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq)]
pub struct ApiTransferProgress {
    /// A key of `pages.server.transfer.state`.
    pub state: compact_str::CompactString,
    /// The step of the stepper it is on: `prepare`, `stop`, `copy`, `verify` or `start`.
    pub step: compact_str::CompactString,
    pub percent: u8,
    /// Bytes of the server (and its backups) copied and in all, as the old node counts them; 0
    /// before it measured them.
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Bytes per second since this attempt began, while copying.
    pub rate: Option<u64>,
    /// Seconds left in the copy at that rate.
    pub eta_seconds: Option<u64>,
}

/// One line of a move's timeline: a key of `pages.server.transfer.events`, the values it names and
/// its tone (`info`, `success`, `warning` or `danger`).
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq)]
pub struct ApiTransferEvent {
    pub at: chrono::DateTime<chrono::Utc>,
    pub key: compact_str::CompactString,
    pub params: BTreeMap<String, String>,
    pub level: compact_str::CompactString,
}

/// The server's backups: what the Transfer page lists (successful, not deleted).
#[derive(ToSchema, Serialize, Clone, Copy, Debug, Default)]
pub struct ApiTransferBackups {
    pub count: i64,
    pub bytes: i64,
}

#[derive(ToSchema, Serialize)]
pub struct ApiTransferOptions {
    /// Players may move their servers (an admin always may).
    pub enabled: bool,
    pub current_location_uuid: uuid::Uuid,
    pub current_location: ApiTransferPlace,
    /// The current location's entry is available when another of its nodes takes servers.
    pub locations: Vec<ApiCreateLocation>,
    /// When the server may move again; `null` when it may now.
    pub cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Why a move cannot start now, a key of `pages.server.transfer.blocked`; `null` when it can.
    pub blocked: Option<compact_str::CompactString>,
    /// Nodes tried per move ("Nodes Tried per Move").
    pub max_attempts: i32,
    /// The server's files on its node, as Wings counts them; `null` while a move runs or when the
    /// node does not answer.
    pub data_bytes: Option<u64>,
    pub backups: ApiTransferBackups,
    /// About how long a move keeps the server offline, in seconds.
    pub estimate_seconds: u64,
    /// The move under way, else the last one.
    pub request: Option<ApiTransferRequest>,
    /// Where the move under way is.
    pub progress: Option<ApiTransferProgress>,
    /// The timeline of `request`, newest first, while it runs and for [`OUTCOME_MINUTES`] after.
    pub events: Vec<ApiTransferEvent>,
    /// The last finished moves, newest first.
    pub history: Vec<ApiTransferRequest>,
}

/// The move the strip on every server page shows: one under way, or one that ended in the last
/// [`STRIP_MINUTES`].
#[derive(ToSchema, Serialize)]
pub struct ApiTransferStatus {
    pub request: ApiTransferRequest,
    pub progress: Option<ApiTransferProgress>,
}

#[derive(Debug)]
pub enum TransferError {
    Disabled,
    Cooldown(chrono::DateTime<chrono::Utc>),
    /// A move of the server is already under way.
    Busy,
    /// Suspended, installing or restoring: the server cannot move now.
    Unavailable,
    /// A backup of the server is being made; Wings would copy it half done.
    BackupRunning,
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
            Self::BackupRunning => "a backup is running, try again once it is done".into(),
            Self::NotMoving => "this server is not being moved".into(),
            Self::AdminMove => "an admin is moving this server, only an admin can cancel it".into(),
            Self::UnknownLocation => "this location is not offered".into(),
            Self::LocationLocked => "your plan does not include this location".into(),
            Self::NoNode(reason) => format!("{NO_NODE}: {reason}"),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled | Self::LocationLocked | Self::AdminMove => 403,
            Self::Cooldown(_) => 429,
            Self::Busy
            | Self::Unavailable
            | Self::BackupRunning
            | Self::NotMoving
            | Self::NoNode(_) => 409,
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

/// `items` with the one whose key is `prefer` moved to the front; the rest keep their order.
pub fn preferred_first<T>(
    mut items: Vec<T>,
    prefer: Option<uuid::Uuid>,
    key: impl Fn(&T) -> uuid::Uuid,
) -> Vec<T> {
    if let Some(prefer) = prefer {
        items.sort_by_key(|item| key(item) != prefer);
    }

    items
}

/// Whether a failed attempt is followed by one on another node.
pub fn retries(attempts: i32, max_attempts: i32) -> bool {
    attempts < max_attempts.max(1)
}

/// Which `pages.server.transfer.reasons` line says why a move ended, for the errors this module
/// writes; `None` for anything else (shown as written).
pub fn reason_key(error: &str) -> Option<&'static str> {
    if error.starts_with(NO_NODE) {
        Some("noNode")
    } else if error == COPY_FAILED {
        Some("copyFailed")
    } else if error == MOVED_MEANWHILE {
        Some("skippedMoved")
    } else {
        None
    }
}

/// About how long a move of `bytes` (files and backups) keeps the server offline, in seconds.
pub fn estimate_seconds(bytes: u64) -> u64 {
    bytes.div_ceil(ESTIMATE_RATE) + ESTIMATE_OVERHEAD
}

/// What keeps the Transfer page from starting a move, see [`blocked`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Blockers {
    pub enabled: bool,
    pub admin: bool,
    pub busy: bool,
    pub suspended: bool,
    pub installing: bool,
    pub restoring: bool,
    pub backup_running: bool,
    pub cooling_down: bool,
}

/// Why a move cannot start now, a key of `pages.server.transfer.blocked`, in the order the Pelican
/// panel asks: an admin is not held by the switch, a suspension or the cooldown.
pub fn blocked(blockers: Blockers) -> Option<&'static str> {
    if !blockers.enabled && !blockers.admin {
        Some("disabled")
    } else if blockers.busy {
        Some("busy")
    } else if blockers.suspended && !blockers.admin {
        Some("suspended")
    } else if blockers.installing {
        Some("installing")
    } else if blockers.restoring {
        Some("restoring")
    } else if blockers.backup_running {
        Some("backupRunning")
    } else if blockers.cooling_down && !blockers.admin {
        Some("cooldown")
    } else {
        None
    }
}

/// Whether a backup of the server is being made (one started in the last day that has not
/// finished; older ones only end when their node resets).
async fn backup_running(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM server_backups
            WHERE server_backups.server_uuid = $1 AND server_backups.completed IS NULL
                AND server_backups.deleted IS NULL
                AND server_backups.created >= NOW() - INTERVAL '1 day'
        )
        "#,
    )
    .bind(server_uuid)
    .fetch_one(database.read())
    .await
}

/// Whether a move that finished at `finished` ended less than `minutes` ago.
pub fn ended_within(
    finished: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
    minutes: i64,
) -> bool {
    finished.is_some_and(|finished| now - finished < chrono::Duration::minutes(minutes))
}

/// What the native transfer of a move under way is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copying {
    /// No native transfer runs: it ended, and [`settle`] has not looked at it yet. `arrived` when
    /// the server is on the new node.
    Ended { arrived: bool },
    /// Wings moves the server: `None` until the old node reports it, then the bytes copied and in
    /// all (0 in all while Wings stops the server).
    Running(Option<(u64, u64)>),
}

/// Where a move under way is, from its status, the attempts so far, its native transfer and the
/// seconds since this attempt began. The bar gives stopping the first 6%, the copy up to 90,
/// verifying on the new node up to 96 and starting the rest, like the Pelican panel's.
pub fn progress_of(
    status: &str,
    attempts: i32,
    copying: Copying,
    elapsed_seconds: i64,
) -> ApiTransferProgress {
    let (mut state, step, percent, done, total) = match (status, copying) {
        (QUEUED, _) => ("queued", "prepare", 0, 0, 0),
        (_, Copying::Ended { arrived: true }) => ("starting", "start", 97, 0, 0),
        (_, Copying::Ended { arrived: false }) => ("checking", "copy", 6, 0, 0),
        (_, Copying::Running(None)) => (
            if attempts > 1 {
                "retrying"
            } else {
                "preparing"
            },
            "prepare",
            2,
            0,
            0,
        ),
        (_, Copying::Running(Some((done, 0)))) => ("stopping", "stop", 4, done, 0),
        (_, Copying::Running(Some((done, total)))) if done < total => (
            "copying",
            "copy",
            (6 + 84 * done / total) as u8,
            done,
            total,
        ),
        (_, Copying::Running(Some((done, total)))) => ("finalizing", "verify", 92, done, total),
    };
    if status == CANCELLING {
        state = "cancelling";
    }

    let rate = (state == "copying" && elapsed_seconds > 0 && done > 0)
        .then(|| done / elapsed_seconds as u64)
        .filter(|rate| *rate > 0);

    ApiTransferProgress {
        state: state.into(),
        step: step.into(),
        percent,
        bytes_done: done.min(total),
        bytes_total: total,
        rate,
        eta_seconds: rate.map(|rate| total.saturating_sub(done).div_ceil(rate)),
    }
}

/// What the old node says about a running copy, at one look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    /// It lists the transfer with its counters.
    Copying {
        archived: u64,
        sent: u64,
        total: u64,
        files: u64,
    },
    /// It answers but does not list the transfer.
    Missing,
    /// It does not answer.
    Unreachable,
}

impl Seen {
    /// Every byte is archived: the new node is checking and unpacking the copy.
    fn handed_off(self) -> bool {
        matches!(self, Self::Copying { archived, total, .. } if total > 0 && archived >= total)
    }
}

/// What the stall watch keeps of a running copy between two looks. It lives in the panel's memory:
/// a restart starts its clocks again, which only gives a copy more time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Watch {
    /// The attempt it watches; a new attempt starts a new watch.
    pub attempt: i32,
    /// The counters last listed: `(archived, sent, total, files)`.
    pub counters: Option<(u64, u64, u64, u64)>,
    /// When the counters last changed, or the watch began.
    pub moved_at: chrono::NaiveDateTime,
    /// When every byte was first seen archived.
    pub handed_off_at: Option<chrono::NaiveDateTime>,
    /// The copy was cancelled through Wings for this reason, at this time.
    pub aborted: Option<(&'static str, chrono::NaiveDateTime)>,
}

/// What the stall watch decides after a look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Wait,
    /// No progress for the stall time (or the old node lost the transfer): cancel it.
    Stalled,
    /// Every byte was sent, and the new node has not confirmed it for the finalize time.
    Unfinalized,
}

impl Verdict {
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::Wait => None,
            Self::Stalled => Some(REASON_STALLED),
            Self::Unfinalized => Some(REASON_UNFINALIZED),
        }
    }
}

/// The watch of attempt `attempt` after the old node was seen as `seen` at `now`, and whether the
/// copy stalled. Like the Pelican panel's watcher: progress is any change of the counters; once
/// every byte is archived only the finalize time counts; while the old node does not answer the
/// copy gets the stall time plus its size at [`SILENT_RATE`].
pub fn watch_copy(
    previous: Option<&Watch>,
    attempt: i32,
    seen: Seen,
    now: chrono::NaiveDateTime,
    stall_minutes: i32,
    finalize_minutes: i32,
) -> (Watch, Verdict) {
    let counters = match seen {
        Seen::Copying {
            archived,
            sent,
            total,
            files,
        } => Some((archived, sent, total, files)),
        _ => None,
    };

    let Some(previous) = previous.filter(|previous| previous.attempt == attempt) else {
        return (
            Watch {
                attempt,
                counters,
                moved_at: now,
                handed_off_at: seen.handed_off().then_some(now),
                aborted: None,
            },
            Verdict::Wait,
        );
    };
    if previous.aborted.is_some() {
        return (*previous, Verdict::Wait);
    }

    let moved = counters.is_some() && counters != previous.counters;
    let watch = Watch {
        attempt,
        counters: counters.or(previous.counters),
        moved_at: if moved { now } else { previous.moved_at },
        handed_off_at: match seen {
            Seen::Copying { .. } if seen.handed_off() => previous.handed_off_at.or(Some(now)),
            Seen::Copying { .. } => None,
            _ => previous.handed_off_at,
        },
        aborted: None,
    };

    let verdict = if let Some(handed_off_at) = watch.handed_off_at {
        if now - handed_off_at > chrono::Duration::minutes(finalize_minutes.max(5) as i64) {
            Verdict::Unfinalized
        } else {
            Verdict::Wait
        }
    } else {
        let mut allowed = chrono::Duration::minutes(stall_minutes.max(2) as i64);
        if seen == Seen::Unreachable {
            let total = watch.counters.map_or(0, |(_, _, total, _)| total);
            allowed += chrono::Duration::seconds(total.div_ceil(SILENT_RATE) as i64);
        }

        if now - watch.moved_at > allowed {
            Verdict::Stalled
        } else {
            Verdict::Wait
        }
    };

    (watch, verdict)
}

/// Whether a copy cancelled at `at` has had its time for Wings to report it.
pub fn abort_overdue(at: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> bool {
    now - at >= chrono::Duration::seconds(ABORT_GRACE_SECONDS)
}

/// The timeline line a `server:transfer.*` activity of a move stands for: a key of
/// `pages.server.transfer.events`, its tone, and the values it names.
pub fn timeline_line(
    event: &str,
    data: &serde_json::Value,
) -> Option<(&'static str, &'static str, BTreeMap<String, String>)> {
    const NONE: &[&str] = &[];

    let (key, level, names): (&'static str, &'static str, &[&str]) = match event {
        "server:transfer.cancel"
            if data
                .get("ended")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false) =>
        {
            ("endedCancelled", "warning", NONE)
        }
        "server:transfer.cancel" => ("cancelling", "info", NONE),
        "server:transfer.attempt"
            if data
                .get("attempt")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1)
                > 1 =>
        {
            ("retrying", "info", &["attempt", "max"])
        }
        "server:transfer.attempt" => ("destination", "info", &["location"]),
        "server:transfer.attempt-failed" => ("attemptFailed", "warning", &["attempt", "reason"]),
        "server:transfer.verified" => ("copied", "success", NONE),
        "server:transfer.port-variable" => ("portVariable", "info", &["name", "port"]),
        "server:transfer.complete" => ("completed", "success", &["address"]),
        "server:transfer.fail" => ("endedFailed", "danger", NONE),
        "server:transfer.cancelled" => ("endedCancelled", "warning", NONE),
        "server:transfer.restart" | "server:transfer.restart-failed" => {
            let moved = data
                .get("moved")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            match (moved, event.ends_with("-failed")) {
                (true, false) => ("started", "success", NONE),
                (true, true) => ("startFailed", "warning", NONE),
                (false, false) => ("restarted", "success", NONE),
                (false, true) => ("restartFailed", "warning", NONE),
            }
        }
        _ => return None,
    };

    let params = names
        .iter()
        .map(|name| {
            let value = match data.get(*name) {
                Some(serde_json::Value::String(value)) => value.clone(),
                None | Some(serde_json::Value::Null) => String::new(),
                Some(value) => value.to_string(),
            };

            (name.to_string(), value)
        })
        .collect();

    Some((key, level, params))
}

/// The address players connect to: the allocation's alias, else its IP, and the port.
fn address(server: &Server) -> Option<String> {
    server.allocation.as_ref().map(|primary| {
        let allocation = &primary.allocation;

        format!(
            "{}:{}",
            allocation
                .ip_alias
                .as_deref()
                .map_or_else(|| allocation.ip.ip().to_string(), str::to_string),
            allocation.port
        )
    })
}

/// Writes a step of a move into the server's activity log, where its timeline comes from.
async fn log(
    state: &crate::State,
    request: &TransferRequest,
    event: &str,
    extra: serde_json::Value,
) {
    let mut data = serde_json::json!({ "request": request.uuid });
    if let (Some(data), serde_json::Value::Object(extra)) = (data.as_object_mut(), extra) {
        data.extend(extra);
    }

    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid: request.server_uuid,
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
        tracing::warn!(server = %request.server_uuid, "failed to log {event}: {err:#?}");
    }
}

/// The end of the last player move that completed.
pub(crate) async fn last_finished(
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

/// What the server's Transfer page shows to a player, or to an admin (`admin`).
pub async fn options(
    state: &crate::State,
    server: &Server,
    admin: bool,
) -> Result<ApiTransferOptions, anyhow::Error> {
    let (enabled, cooldown_hours, max_attempts) = {
        let settings = state.settings.get().await?;
        (
            settings.games.transfer_enabled,
            settings.games.transfer_cooldown_hours,
            settings.games.transfer_attempts,
        )
    };

    let node = server.node.fetch_cached(&state.database).await?;
    let mut locations = crate::server_creation::locations(state, &server.owner).await?;
    // picking the current location moves the server to another of its nodes
    let another_node = another_node(&state.database, &node).await?;
    for location in locations
        .iter_mut()
        .filter(|location| location.uuid == node.location.uuid)
    {
        location.available = another_node;
    }

    let now = chrono::Utc::now().naive_utc();
    let cooldown = cooldown_until(
        last_finished(&state.database, server.uuid).await?,
        cooldown_hours,
        now,
    );

    let latest = TransferRequest::latest(&state.database, server.uuid).await?;
    let active = latest.as_ref().is_some_and(TransferRequest::active);
    let progress = match latest.as_ref().filter(|request| request.active()) {
        Some(request) => Some(progress(state, server, request).await),
        None => None,
    };
    let shown = latest.as_ref().is_some_and(|request| {
        request.active() || ended_within(request.finished, now, OUTCOME_MINUTES)
    });

    let history = TransferRequest::history(&state.database, server.uuid).await?;
    let mut requests = into_api_objects(
        &state.database,
        latest.iter().chain(history.iter()).cloned().collect(),
    )
    .await?
    .into_iter();
    let request = if latest.is_some() {
        requests.next()
    } else {
        None
    };
    let history = requests.collect();

    let events = match (latest.as_ref(), request.as_ref()) {
        (Some(latest), Some(request)) if shown => {
            timeline(&state.database, latest, request.to_location.as_ref()).await?
        }
        _ => Vec::new(),
    };

    let blocked = blocked(Blockers {
        enabled,
        admin,
        busy: active || server.destination_node.is_some(),
        suspended: server.suspended,
        installing: matches!(
            server.status,
            Some(ServerStatus::Installing | ServerStatus::InstallFailed)
        ),
        restoring: matches!(
            server.status,
            Some(ServerStatus::RestoringBackup | ServerStatus::BackupRestoreFailed)
        ),
        backup_running: backup_running(&state.database, server.uuid).await?,
        cooling_down: cooldown.is_some(),
    });

    let (backups, moved_bytes) = backups(&state.database, server.uuid).await?;
    // Wings is busy with the copy during a move, and the page shows its progress instead
    let data_bytes = if active {
        None
    } else {
        usage(state, server).await.map(|usage| usage.disk_bytes)
    };

    Ok(ApiTransferOptions {
        enabled,
        current_location_uuid: node.location.uuid,
        current_location: ApiTransferPlace {
            uuid: node.location.uuid,
            name: node.location.name.clone(),
            flag: node.location.flag.clone(),
        },
        locations,
        cooldown_until: cooldown.map(|until| until.and_utc()),
        blocked: blocked.map(Into::into),
        max_attempts: max_attempts.max(1),
        data_bytes,
        backups,
        estimate_seconds: estimate_seconds(data_bytes.unwrap_or(0) + moved_bytes),
        request,
        progress,
        events,
        history,
    })
}

/// The move the strip on every server page shows, with where it is.
pub async fn status(
    state: &crate::State,
    server: &Server,
) -> Result<Option<ApiTransferStatus>, anyhow::Error> {
    let Some(latest) = TransferRequest::latest(&state.database, server.uuid).await? else {
        return Ok(None);
    };
    if !latest.active()
        && !ended_within(
            latest.finished,
            chrono::Utc::now().naive_utc(),
            STRIP_MINUTES,
        )
    {
        return Ok(None);
    }

    let progress = if latest.active() {
        Some(progress(state, server, &latest).await)
    } else {
        None
    };

    Ok(into_api_objects(&state.database, vec![latest])
        .await?
        .pop()
        .map(|request| ApiTransferStatus { request, progress }))
}

/// Where a move under way is; asks the old node how far its copy got.
async fn progress(
    state: &crate::State,
    server: &Server,
    request: &TransferRequest,
) -> ApiTransferProgress {
    let copying = if server.destination_node.is_some() {
        let transfers = async {
            server
                .node
                .fetch_cached(&state.database)
                .await?
                .api_client(&state.database)
                .await?
                .get_transfers()
                .await
                .map_err(anyhow::Error::from)
        }
        .await;

        Copying::Running(
            transfers
                .ok()
                .and_then(|transfers| transfers.get(&server.uuid).cloned())
                .map(|progress| (progress.archive_bytes_processed, progress.bytes_total)),
        )
    } else {
        Copying::Ended {
            arrived: request.destination_node_uuid == Some(server.node.uuid),
        }
    };
    let elapsed = request.started.map_or(0, |started| {
        (chrono::Utc::now().naive_utc() - started).num_seconds()
    });

    progress_of(&request.status, request.attempts, copying, elapsed)
}

/// Whether another node of `node`'s location than `node` takes players' servers now.
async fn another_node(
    database: &crate::database::Database,
    node: &Node,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM nodes
            WHERE nodes.location_uuid = $1 AND nodes.uuid <> $2 AND nodes.role = $3
                AND nodes.deployment_enabled AND NOT nodes.maintenance_enabled
                AND NOT nodes.deployment_paused
        )
        "#,
    )
    .bind(node.location.uuid)
    .bind(node.uuid)
    .bind(GAME_ROLE)
    .fetch_one(database.read())
    .await
}

/// The moves as the Transfer page shows them, with the locations they left and go to.
async fn into_api_objects(
    database: &crate::database::Database,
    requests: Vec<TransferRequest>,
) -> Result<Vec<ApiTransferRequest>, sqlx::Error> {
    let location_uuids = requests
        .iter()
        .filter_map(|request| request.location_uuid)
        .collect::<Vec<_>>();
    let node_uuids = requests
        .iter()
        .flat_map(|request| [request.source_node_uuid, request.destination_node_uuid])
        .flatten()
        .collect::<Vec<_>>();

    let rows = sqlx::query(
        r#"
        SELECT NULL::UUID AS node_uuid, locations.uuid, locations.name, locations.flag
        FROM locations
        WHERE locations.uuid = ANY($1)
        UNION ALL
        SELECT nodes.uuid AS node_uuid, locations.uuid, locations.name, locations.flag
        FROM nodes
        JOIN locations ON locations.uuid = nodes.location_uuid
        WHERE nodes.uuid = ANY($2)
        "#,
    )
    .bind(&location_uuids)
    .bind(&node_uuids)
    .fetch_all(database.read())
    .await?;

    let mut by_location = HashMap::new();
    let mut by_node = HashMap::new();
    for row in rows {
        let place = ApiTransferPlace {
            uuid: row.try_get("uuid")?,
            name: row.try_get::<String, _>("name")?.into(),
            flag: row.try_get::<Option<String>, _>("flag")?.map(Into::into),
        };
        match row.try_get::<Option<uuid::Uuid>, _>("node_uuid")? {
            Some(node_uuid) => by_node.insert(node_uuid, place),
            None => by_location.insert(place.uuid, place),
        };
    }

    Ok(requests
        .into_iter()
        .map(|request| {
            let from = request
                .source_node_uuid
                .and_then(|uuid| by_node.get(&uuid).cloned());
            let to = request
                .location_uuid
                .and_then(|uuid| by_location.get(&uuid).cloned())
                .or_else(|| {
                    request
                        .destination_node_uuid
                        .and_then(|uuid| by_node.get(&uuid).cloned())
                });

            ApiTransferRequest {
                from_location: from,
                to_location: to,
                ..request.into_api_object()
            }
        })
        .collect())
}

/// A move's timeline, newest first: the steps written to the activity log, then how it began.
async fn timeline(
    database: &crate::database::Database,
    request: &TransferRequest,
    to: Option<&ApiTransferPlace>,
) -> Result<Vec<ApiTransferEvent>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT server_activities.event, server_activities.data, server_activities.created
        FROM server_activities
        WHERE server_activities.server_uuid = $1 AND server_activities.created >= $2
            AND server_activities.event LIKE 'server:transfer.%'
            AND server_activities.data->>'request' = $3
        ORDER BY server_activities.created DESC
        LIMIT $4
        "#,
    )
    .bind(request.server_uuid)
    .bind(request.created - chrono::Duration::minutes(1))
    .bind(request.uuid.to_string())
    .bind(TIMELINE)
    .fetch_all(database.read())
    .await?;

    let mut events = Vec::with_capacity(rows.len() + 1);
    for row in rows {
        let event: String = row.try_get("event")?;
        let data: serde_json::Value = row.try_get("data")?;
        let created: chrono::NaiveDateTime = row.try_get("created")?;

        if let Some((key, level, params)) = timeline_line(&event, &data) {
            events.push(ApiTransferEvent {
                at: created.and_utc(),
                key: key.into(),
                params,
                level: level.into(),
            });
        }
    }

    events.push(ApiTransferEvent {
        at: request.created.and_utc(),
        key: if request.origin == ORIGIN_ADMIN {
            "adopted"
        } else {
            "requested"
        }
        .into(),
        params: BTreeMap::from([(
            "location".to_string(),
            to.map(|place| place.name.to_string()).unwrap_or_default(),
        )]),
        level: "info".into(),
    });

    Ok(events)
}

/// The server's successful backups, and the bytes of those a move copies (shared ones stay).
async fn backups(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<(ApiTransferBackups, u64), sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS count, COALESCE(SUM(server_backups.bytes), 0)::BIGINT AS bytes,
            COALESCE(SUM(server_backups.bytes) FILTER (WHERE NOT server_backups.shared), 0)::BIGINT
                AS moved
        FROM server_backups
        WHERE server_backups.server_uuid = $1 AND server_backups.deleted IS NULL
            AND server_backups.successful
        "#,
    )
    .bind(server_uuid)
    .fetch_one(database.read())
    .await?;

    Ok((
        ApiTransferBackups {
            count: row.try_get("count")?,
            bytes: row.try_get("bytes")?,
        },
        row.try_get::<i64, _>("moved")?.max(0) as u64,
    ))
}

/// What Wings says the server uses; `None` when it does not answer.
async fn usage(state: &crate::State, server: &Server) -> Option<wings_api::ResourceUsage> {
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
}

/// Whether Wings says the server is running (or starting); `false` when it cannot tell.
async fn running(state: &crate::State, server: &Server) -> bool {
    usage(state, server).await.is_some_and(|usage| {
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
    request_preferring(state, server, user, location_uuid, None, admin).await
}

/// [`request`], trying `prefer` (a node of the location) first: the start queue's move names the
/// node it goes to.
pub async fn request_preferring(
    state: &crate::State,
    server: &Server,
    user: &User,
    location_uuid: uuid::Uuid,
    prefer: Option<uuid::Uuid>,
    admin: bool,
) -> Result<ApiTransferRequest, TransferError> {
    request_with(state, server, user, location_uuid, prefer, admin, false).await
}

/// An admin moves the server to the best node of `location_uuid` (Admin › Servers › Transfer, "Best
/// node"), the way a player's move goes: ports kept where free, backups along, retried on another
/// node of the location. `start` starts the server on its new node even when it was off.
pub async fn admin_request(
    state: &crate::State,
    server: &Server,
    admin: &User,
    location_uuid: uuid::Uuid,
    start: bool,
) -> Result<ApiTransferRequest, TransferError> {
    request_with(state, server, admin, location_uuid, None, true, start).await
}

async fn request_with(
    state: &crate::State,
    server: &Server,
    user: &User,
    location_uuid: uuid::Uuid,
    prefer: Option<uuid::Uuid>,
    admin: bool,
    start: bool,
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

    if backup_running(&state.database, server.uuid).await? {
        return Err(TransferError::BackupRunning);
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
    let restart = start || queued || running(state, server).await;
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

    if let Err(err) = start_attempt(state, &mut request, prefer).await {
        request.finish(FAILED, Some(err.message()));
        request.save(&state.database).await?;
        fail(state, &request).await;

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
/// configuration deploys, else ports inside the deployment port ranges (its own numbers where it
/// can, made when missing), else any free ones. Egg variables that held one of its old ports
/// follow it to the new one (docs/features/allocations.md). `None` when the node has no room.
pub(crate) async fn pick_allocations(
    state: &crate::State,
    server: &Server,
    node: &Node,
) -> Result<Option<Picked>, anyhow::Error> {
    // plugged.host: a server coming back from a limbo node to its home node first gets its old
    // ports there (docs/features/limbo.md#parked-on-a-node)
    if let Some(picked) = crate::limbo::home_allocations(state, server, node).await? {
        return Ok(Some(picked));
    }

    // the order the new allocations line up with the old ones in (crate::allocations::carry_over)
    let ports = crate::allocations::move_order(
        server,
        ServerAllocation::all_by_server_uuid(&state.database, server.uuid).await?,
    )
    .into_iter()
    .map(|allocation| allocation.allocation.port)
    .collect::<Vec<_>>();

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
            let variables = variables
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect();

            return following_ports(state, server, &ports, primary, additional, variables)
                .await
                .map(Some);
        }
    }

    match crate::allocations::deploy_for_move(state, node, &ports).await {
        Ok(Some(picked)) => {
            let new = picked.iter().map(|(_, port)| *port).collect::<Vec<_>>();
            let (primary, additional) = split(picked.into_iter().map(|(uuid, _)| uuid).collect());

            return Ok(Some(Picked {
                primary,
                additional,
                variables: crate::allocations::followed_variables(
                    &state.database,
                    server,
                    &ports,
                    &new,
                )
                .await?,
            }));
        }
        Ok(None) => {}
        Err(err) => {
            tracing::warn!(server = %server.uuid, node = %node.uuid, "failed to pick ports in the deployment port ranges: {err:?}");
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

    following_ports(state, server, &ports, primary, additional, HashMap::new())
        .await
        .map(Some)
}

/// [`Picked`] with the egg variables that held one of the `old` ports set to the new port in the
/// same place; `variables` (an egg configuration's) win.
async fn following_ports(
    state: &crate::State,
    server: &Server,
    old: &[i32],
    primary: Option<uuid::Uuid>,
    additional: Vec<uuid::Uuid>,
    mut variables: HashMap<String, String>,
) -> Result<Picked, anyhow::Error> {
    let uuids = primary
        .into_iter()
        .chain(additional.iter().copied())
        .collect::<Vec<_>>();
    let ports = crate::allocations::ports_of(&state.database, &uuids).await?;
    let new = uuids
        .iter()
        .filter_map(|uuid| ports.get(uuid).copied())
        .collect::<Vec<_>>();

    for (name, value) in
        crate::allocations::followed_variables(&state.database, server, old, &new).await?
    {
        variables.entry(name).or_insert(value);
    }

    Ok(Picked {
        primary,
        additional,
        variables,
    })
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

/// Starts the native transfer to the best node of the location the request has not tried yet
/// (`prefer` first when it can take the server).
async fn start_attempt(
    state: &crate::State,
    request: &mut TransferRequest,
    prefer: Option<uuid::Uuid>,
) -> Result<(), TransferError> {
    let server = Server::by_uuid_optional(&state.database, request.server_uuid)
        .await?
        .ok_or(TransferError::NotMoving)?;
    let source = server.node.fetch_cached(&state.database).await?;
    let location_uuid = request.location_uuid.unwrap_or(source.location.uuid);
    // players' servers go to game nodes; an admin's move stays among nodes of the source's role
    // plugged.host: a limbo restore leaves a limbo node for a game node
    let role = if request.origin == ORIGIN_USER || request.origin == ORIGIN_LIMBO {
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
    // the rest keep their order
    let nodes = preferred_first(nodes, prefer, |node| node.uuid);

    let mut reason = None;
    for node in nodes {
        let Some(picked) = pick_allocations(state, &server, &node).await? else {
            reason = Some("not enough free allocations".to_string());
            continue;
        };

        let backups =
            ServerBackup::all_uuids_by_server_uuid_not_shared(&state.database, server.uuid).await?;
        let node_uuid = node.uuid;
        let location = node.location.name.clone();

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
        let max_attempts = state.settings.get().await?.games.transfer_attempts;
        log(
            state,
            request,
            "server:transfer.attempt",
            serde_json::json!({
                "attempt": request.attempts,
                "max": max_attempts.max(request.attempts),
                "location": location,
            }),
        )
        .await;

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

/// A cancelled move: its request, and whether it ended at once (a queued one) or settles once Wings
/// has stopped copying.
#[derive(Clone, Copy, Debug)]
pub struct Cancelled {
    pub request: uuid::Uuid,
    pub ended: bool,
}

/// Cancels the server's move: a queued one at once, a running one through Wings (it settles once
/// Wings has stopped copying). A move an admin started (`admin` false) is refused.
pub async fn cancel(
    state: &crate::State,
    server: &Server,
    admin: bool,
) -> Result<Cancelled, TransferError> {
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
    // plugged.host: a limbo restore's move is limbo's (it fails the restore when cancelled)
    if request.origin != ORIGIN_USER && !admin {
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

    Ok(Cancelled {
        request: request.uuid,
        ended: !request.active(),
    })
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

/// Starts a server again after its move (`moved`: on its new node); a suspended one stays off.
async fn start_again(
    state: &crate::State,
    request: &TransferRequest,
    server: &Server,
    moved: bool,
) {
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

    if let Err(err) = &started {
        tracing::warn!(server = %server.uuid, "failed to start a server after its move: {err:#}");
    }
    log(
        state,
        request,
        if started.is_ok() {
            "server:transfer.restart"
        } else {
            "server:transfer.restart-failed"
        },
        serde_json::json!({ "moved": moved }),
    )
    .await;
}

/// A move that gave up: written to the timeline with why.
async fn fail(state: &crate::State, request: &TransferRequest) {
    log(
        state,
        request,
        "server:transfer.fail",
        serde_json::json!({ "reason": request.error.as_deref().and_then(reason_key) }),
    )
    .await;
}

/// Writes the new ports into the egg variables the egg configuration named and the ones that held
/// one of the server's old ports (docs/features/allocations.md).
async fn write_variables(
    state: &crate::State,
    request: &TransferRequest,
    server: &Server,
) -> Result<(), anyhow::Error> {
    if request.variables.is_empty() {
        return Ok(());
    }

    let egg_variables = NestEggVariable::all_by_egg_uuid(&state.database, server.egg.uuid).await?;
    for (name, value) in &request.variables {
        if let Some(variable) = egg_variables
            .iter()
            .find(|v| v.env_variable == name.as_str())
        {
            ServerVariable::create(&state.database, server.uuid, variable.uuid, value).await?;
            log(
                state,
                request,
                "server:transfer.port-variable",
                serde_json::json!({ "name": name, "port": value }),
            )
            .await;
        }
    }

    Ok(())
}

/// Ends the server's native transfer as failed, the way Wings' own failure report does
/// (`POST /api/remote/servers/{server}/transfer/failure`): the server stays on its node, the
/// allocations it got on the new node are given back, the new node drops its copy, and
/// `TransferCompleted { successful: false }` is emitted. The old node keeps its copy until the panel
/// records a success, so nothing is lost. `false` when no transfer was under way any more.
pub async fn release(state: &crate::State, server: &Server) -> Result<bool, anyhow::Error> {
    let Some(destination) = &server.destination_node else {
        return Ok(false);
    };
    let destination = destination.fetch_cached(&state.database).await?;

    let mut transaction = state.database.write().begin().await?;

    // a success reported meanwhile moved the server: nothing to release then
    let released = sqlx::query(
        "UPDATE servers
        SET destination_allocation_uuid = NULL, destination_node_uuid = NULL
        WHERE servers.uuid = $1 AND servers.node_uuid = $2 AND servers.destination_node_uuid = $3",
    )
    .bind(server.uuid)
    .bind(server.node.uuid)
    .bind(destination.uuid)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        > 0;
    if !released {
        transaction.rollback().await?;
        return Ok(false);
    }

    sqlx::query(
        "DELETE FROM server_allocations
        USING node_allocations
        WHERE node_allocations.uuid = server_allocations.allocation_uuid
            AND server_allocations.server_uuid = $1 AND node_allocations.node_uuid <> $2",
    )
    .bind(server.uuid)
    .bind(server.node.uuid)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Server::invalidate_cached(&state.database, server.uuid).await;

    if let Err(err) = destination
        .api_client(&state.database)
        .await?
        .delete_servers_server(server.uuid)
        .await
    {
        tracing::error!("failed to delete server on destination node: {:?}", err);
    }

    Server::get_event_emitter().emit(
        state.clone(),
        ServerEvent::TransferCompleted {
            server: Box::new(server.clone()),
            destination_node: Box::new(destination),
            successful: false,
        },
    );

    Ok(true)
}

/// The stall watch of every running move, by request (see [`Watch`]).
static WATCHES: LazyLock<Mutex<HashMap<uuid::Uuid, Watch>>> = LazyLock::new(Default::default);

fn watches() -> std::sync::MutexGuard<'static, HashMap<uuid::Uuid, Watch>> {
    WATCHES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Looks at a move whose native transfer still runs. A copy that stalled (or that its new node
/// never confirmed) is cancelled on the old node, like an owner's cancel; when Wings has not
/// reported a cancelled copy within [`ABORT_GRACE_SECONDS`], the server is released here
/// ([`release`]). Returns why the attempt ended when it was released.
async fn watch(
    state: &crate::State,
    request: &TransferRequest,
    server: &Server,
    stall_minutes: i32,
    finalize_minutes: i32,
) -> Option<&'static str> {
    let now = chrono::Utc::now().naive_utc();
    let previous = watches()
        .get(&request.uuid)
        .copied()
        .filter(|watch| watch.attempt == request.attempts);

    let aborted = match previous.and_then(|watch| watch.aborted) {
        Some(aborted) => aborted,
        // an owner's or an admin's cancel: Wings gets its time to report it first
        None if request.status == CANCELLING => {
            let mut watch = previous.unwrap_or(Watch {
                attempt: request.attempts,
                counters: None,
                moved_at: now,
                handed_off_at: None,
                aborted: None,
            });
            watch.aborted = Some((REASON_CANCELLED, now));
            watches().insert(request.uuid, watch);

            return None;
        }
        None => {
            let seen = match async {
                server
                    .node
                    .fetch_cached(&state.database)
                    .await?
                    .api_client(&state.database)
                    .await?
                    .get_transfers()
                    .await
                    .map_err(anyhow::Error::from)
            }
            .await
            {
                Ok(transfers) => transfers
                    .get(&server.uuid)
                    .map_or(Seen::Missing, |progress| Seen::Copying {
                        archived: progress.archive_bytes_processed,
                        sent: progress.network_bytes_processed,
                        total: progress.bytes_total,
                        files: progress.files_processed,
                    }),
                Err(_) => Seen::Unreachable,
            };

            let (mut watch, verdict) = watch_copy(
                previous.as_ref(),
                request.attempts,
                seen,
                now,
                stall_minutes,
                finalize_minutes,
            );
            if let Some(reason) = verdict.reason() {
                tracing::warn!(
                    server = %server.uuid,
                    attempt = request.attempts,
                    reason,
                    "cancelling a move whose copy stalled"
                );
                if let Err(err) = async {
                    server
                        .node
                        .fetch_cached(&state.database)
                        .await?
                        .api_client(&state.database)
                        .await?
                        .delete_servers_server_transfer(server.uuid)
                        .await
                        .map_err(anyhow::Error::from)
                }
                .await
                {
                    tracing::warn!(server = %server.uuid, "failed to cancel a stalled copy on its node: {err:#}");
                }
                watch.aborted = Some((reason, now));
            }
            watches().insert(request.uuid, watch);

            return None;
        }
    };

    let (reason, at) = aborted;
    if !abort_overdue(at, now) {
        return None;
    }

    match release(state, server).await {
        Ok(true) => {
            tracing::warn!(server = %server.uuid, reason, "released a move Wings did not report");
            Some(reason)
        }
        Ok(false) => None,
        Err(err) => {
            tracing::warn!(server = %server.uuid, "failed to release a stalled move: {err:#}");
            None
        }
    }
}

/// The `settle_transfers` task: closes moves whose native transfer is over (done, failed and
/// retried on another node, or cancelled), cancels copies that stalled ([`watch`]), and starts
/// queued ones while their node has a free turn.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let (max_attempts, parallel, stall_minutes, finalize_minutes) = {
        let settings = state.settings.get().await?;
        (
            settings.games.transfer_attempts,
            settings.games.transfer_parallel,
            settings.games.transfer_stall_minutes,
            settings.games.transfer_finalize_minutes,
        )
    };

    let mut watched = HashSet::new();
    for mut request in
        TransferRequest::all_with_status(&state.database, &[RUNNING, CANCELLING]).await?
    {
        watched.insert(request.uuid);
        let Some(mut server) =
            Server::by_uuid_optional(&state.database, request.server_uuid).await?
        else {
            continue;
        };
        if server.destination_node.is_some() {
            if watch(state, &request, &server, stall_minutes, finalize_minutes)
                .await
                .is_none()
            {
                continue;
            }

            // released here: the attempt ends like one Wings reported as failed
            match Server::by_uuid_optional(&state.database, request.server_uuid).await? {
                Some(released) if released.destination_node.is_none() => server = released,
                _ => continue,
            }
        }
        // why the panel cancelled this attempt, if it did
        let stalled = watches()
            .remove(&request.uuid)
            .filter(|watch| watch.attempt == request.attempts)
            .and_then(|watch| watch.aborted)
            .map(|(reason, _)| reason)
            .filter(|reason| *reason != REASON_CANCELLED);

        if Some(server.node.uuid) == request.destination_node_uuid {
            log(
                state,
                &request,
                "server:transfer.verified",
                serde_json::json!({}),
            )
            .await;
            if let Err(err) = write_variables(state, &request, &server).await {
                tracing::warn!(server = %server.uuid, "failed to write a moved server's ports: {err:#}");
            }
            request.finish(COMPLETED, None);
            request.save(&state.database).await?;
            tracing::info!(server = %server.uuid, "moved a server");
            log(
                state,
                &request,
                "server:transfer.complete",
                serde_json::json!({ "address": address(&server) }),
            )
            .await;
            crate::notifications::transfer_ended(state, &request).await;

            if request.restart {
                start_again(state, &request, &server, true).await;
            }
            continue;
        }

        if request.status == CANCELLING {
            request.finish(CANCELLED, None);
            log(
                state,
                &request,
                "server:transfer.cancelled",
                serde_json::json!({}),
            )
            .await;
        } else {
            log(
                state,
                &request,
                "server:transfer.attempt-failed",
                serde_json::json!({
                    "attempt": request.attempts,
                    "reason": stalled.unwrap_or("copyFailed"),
                }),
            )
            .await;
            if let Some(reason) = stalled {
                tracing::info!(server = %server.uuid, attempt = request.attempts, reason, "a stalled move attempt ended");
            }

            if retries(request.attempts, max_attempts) {
                match start_attempt(state, &mut request, None).await {
                    Ok(()) => continue,
                    Err(err) => request.finish(FAILED, Some(err.message())),
                }
            } else {
                request.finish(FAILED, Some(COPY_FAILED.into()));
            }
            fail(state, &request).await;
            crate::notifications::transfer_ended(state, &request).await;
        }
        request.save(&state.database).await?;

        if request.restart {
            start_again(state, &request, &server, false).await;
        }
    }
    // moves that ended without passing here (deleted servers, ended elsewhere)
    watches().retain(|request, _| watched.contains(request));

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
                request.finish(CANCELLED, Some(MOVED_MEANWHILE.into()));
                request.save(&state.database).await?;
                log(
                    state,
                    &request,
                    "server:transfer.cancelled",
                    serde_json::json!({}),
                )
                .await;
                continue;
            }
        }

        if let Err(err) = start_attempt(state, &mut request, None).await {
            request.finish(FAILED, Some(err.message()));
            request.save(&state.database).await?;
            fail(state, &request).await;
            crate::notifications::transfer_ended(state, &request).await;
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
            AND (servers.limbo_state IS NULL OR servers.limbo_state = 'stopped')
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

/// plugged.host: limbo's restore of a server parked on a limbo node (docs/features/limbo.md
/// #parked-on-a-node): a move off that node to the best game node of `location_uuid`, `prefer`
/// (its home node) first, its old ports there when they are free (`limbo::home_allocations`),
/// retried on another node of the location like a player's move. It stays off; limbo follows the
/// request and ends the restore. `user_uuid` is who asked for it.
pub async fn limbo_request(
    state: &crate::State,
    server: &Server,
    user_uuid: uuid::Uuid,
    location_uuid: uuid::Uuid,
    prefer: Option<uuid::Uuid>,
) -> Result<TransferRequest, TransferError> {
    if server.destination_node.is_some()
        || TransferRequest::latest(&state.database, server.uuid)
            .await?
            .is_some_and(|request| request.active())
    {
        return Err(TransferError::Busy);
    }

    let mut request = insert(
        state,
        server,
        Some(user_uuid),
        ORIGIN_LIMBO,
        location_uuid,
        false,
    )
    .await?;
    if let Err(err) = start_attempt(state, &mut request, prefer).await {
        request.finish(FAILED, Some(err.message()));
        request.save(&state.database).await?;
        fail(state, &request).await;

        return Err(err);
    }

    Ok(request)
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
    fn a_preferred_node_goes_first_and_the_rest_keep_their_order() {
        let uuids = |items: &[u128]| {
            items
                .iter()
                .map(|n| uuid::Uuid::from_u128(*n))
                .collect::<Vec<_>>()
        };
        let order = |items: Vec<uuid::Uuid>| items.iter().map(|u| u.as_u128()).collect::<Vec<_>>();

        let first = |prefer| order(preferred_first(uuids(&[1, 2, 3, 4]), prefer, |u| *u));
        assert_eq!(first(Some(uuid::Uuid::from_u128(3))), vec![3, 1, 2, 4]);
        assert_eq!(first(Some(uuid::Uuid::from_u128(9))), vec![1, 2, 3, 4]);
        assert_eq!(first(None), vec![1, 2, 3, 4]);
    }

    #[test]
    fn attempts_stop_at_the_limit() {
        assert!(retries(1, 2));
        assert!(!retries(2, 2));
        assert!(!retries(1, 0));
    }

    #[test]
    fn reasons_name_the_errors_this_module_writes() {
        assert_eq!(
            reason_key(&TransferError::NoNode("every node is full".into()).message()),
            Some("noNode")
        );
        assert_eq!(reason_key(COPY_FAILED), Some("copyFailed"));
        assert_eq!(reason_key(MOVED_MEANWHILE), Some("skippedMoved"));
        assert_eq!(reason_key("connection refused"), None);
    }

    #[test]
    fn the_estimate_adds_the_stop_and_start_to_the_copy() {
        assert_eq!(estimate_seconds(0), 45);
        assert_eq!(estimate_seconds(1), 46);
        assert_eq!(estimate_seconds(40 * 1024 * 1024), 46);
        assert_eq!(estimate_seconds(4 * 1024 * 1024 * 1024), 45 + 103);
    }

    #[test]
    fn an_outcome_shows_for_a_while() {
        assert!(!ended_within(None, at(12), 10));
        assert!(ended_within(
            Some(at(12) - chrono::Duration::minutes(9)),
            at(12),
            10
        ));
        assert!(!ended_within(
            Some(at(12) - chrono::Duration::minutes(10)),
            at(12),
            10
        ));
        assert!(!ended_within(Some(at(11)), at(12), 2));
    }

    #[test]
    fn progress_follows_the_copy() {
        let progress = |status, attempts, copying| {
            let progress = progress_of(status, attempts, copying, 100);
            (
                progress.state.to_string(),
                progress.step.to_string(),
                progress.percent,
            )
        };
        let expect =
            |state: &str, step: &str, percent: u8| (state.to_string(), step.to_string(), percent);

        assert_eq!(
            progress(QUEUED, 0, Copying::Ended { arrived: false }),
            expect("queued", "prepare", 0)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Running(None)),
            expect("preparing", "prepare", 2)
        );
        assert_eq!(
            progress(RUNNING, 2, Copying::Running(None)),
            expect("retrying", "prepare", 2)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Running(Some((0, 0)))),
            expect("stopping", "stop", 4)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Running(Some((0, 1000)))),
            expect("copying", "copy", 6)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Running(Some((500, 1000)))),
            expect("copying", "copy", 48)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Running(Some((1000, 1000)))),
            expect("finalizing", "verify", 92)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Ended { arrived: true }),
            expect("starting", "start", 97)
        );
        assert_eq!(
            progress(RUNNING, 1, Copying::Ended { arrived: false }),
            expect("checking", "copy", 6)
        );
        assert_eq!(
            progress(CANCELLING, 1, Copying::Running(Some((500, 1000)))),
            expect("cancelling", "copy", 48)
        );
    }

    #[test]
    fn the_speed_counts_from_the_start_of_the_attempt() {
        let copying = progress_of(RUNNING, 1, Copying::Running(Some((500, 1000))), 10);
        assert_eq!(copying.rate, Some(50));
        assert_eq!(copying.eta_seconds, Some(10));
        assert_eq!((copying.bytes_done, copying.bytes_total), (500, 1000));

        let started = progress_of(RUNNING, 1, Copying::Running(Some((0, 1000))), 10);
        assert_eq!((started.rate, started.eta_seconds), (None, None));

        let stopping = progress_of(RUNNING, 1, Copying::Running(Some((0, 0))), 10);
        assert_eq!((stopping.rate, stopping.eta_seconds), (None, None));
    }

    #[test]
    fn activities_become_timeline_lines() {
        let line = |event: &str, data: serde_json::Value| {
            timeline_line(event, &data).map(|(key, level, params)| {
                (
                    key,
                    level,
                    params
                        .into_iter()
                        .map(|(name, value)| format!("{name}={value}"))
                        .collect::<Vec<_>>(),
                )
            })
        };

        assert_eq!(
            line(
                "server:transfer.attempt",
                serde_json::json!({ "attempt": 1, "max": 2, "location": "Paris" })
            ),
            Some(("destination", "info", vec!["location=Paris".to_string()]))
        );
        assert_eq!(
            line(
                "server:transfer.attempt",
                serde_json::json!({ "attempt": 2, "max": 3, "location": "Paris" })
            ),
            Some((
                "retrying",
                "info",
                vec!["attempt=2".to_string(), "max=3".to_string()]
            ))
        );
        assert_eq!(
            line(
                "server:transfer.attempt-failed",
                serde_json::json!({ "attempt": 1, "reason": "copyFailed" })
            ),
            Some((
                "attemptFailed",
                "warning",
                vec!["attempt=1".to_string(), "reason=copyFailed".to_string()]
            ))
        );
        assert_eq!(
            line(
                "server:transfer.complete",
                serde_json::json!({ "address": "node.example.com:25565" })
            ),
            Some((
                "completed",
                "success",
                vec!["address=node.example.com:25565".to_string()]
            ))
        );
        assert_eq!(
            line(
                "server:transfer.fail",
                serde_json::json!({ "reason": null })
            ),
            Some(("endedFailed", "danger", vec![]))
        );
        assert_eq!(
            line("server:transfer.cancel", serde_json::json!({})),
            Some(("cancelling", "info", vec![]))
        );
        assert_eq!(
            line(
                "server:transfer.cancel",
                serde_json::json!({ "ended": true })
            ),
            Some(("endedCancelled", "warning", vec![]))
        );
        assert_eq!(
            line(
                "server:transfer.restart",
                serde_json::json!({ "moved": true })
            ),
            Some(("started", "success", vec![]))
        );
        assert_eq!(
            line(
                "server:transfer.restart-failed",
                serde_json::json!({ "moved": false })
            ),
            Some(("restartFailed", "warning", vec![]))
        );
        assert_eq!(line("server:transfer.request", serde_json::json!({})), None);
    }

    #[test]
    fn blocked_asks_in_the_pelican_panels_order() {
        let open = Blockers {
            enabled: true,
            ..Blockers::default()
        };

        assert_eq!(blocked(open), None);
        assert_eq!(
            blocked(Blockers {
                enabled: false,
                ..open
            }),
            Some("disabled")
        );
        assert_eq!(
            blocked(Blockers {
                enabled: false,
                admin: true,
                ..open
            }),
            None
        );
        assert_eq!(
            blocked(Blockers {
                busy: true,
                cooling_down: true,
                ..open
            }),
            Some("busy")
        );
        assert_eq!(
            blocked(Blockers {
                suspended: true,
                ..open
            }),
            Some("suspended")
        );
        assert_eq!(
            blocked(Blockers {
                suspended: true,
                admin: true,
                ..open
            }),
            None
        );
        assert_eq!(
            blocked(Blockers {
                installing: true,
                ..open
            }),
            Some("installing")
        );
        assert_eq!(
            blocked(Blockers {
                restoring: true,
                ..open
            }),
            Some("restoring")
        );
        assert_eq!(
            blocked(Blockers {
                backup_running: true,
                admin: true,
                ..open
            }),
            Some("backupRunning")
        );
        assert_eq!(
            blocked(Blockers {
                cooling_down: true,
                ..open
            }),
            Some("cooldown")
        );
        assert_eq!(
            blocked(Blockers {
                cooling_down: true,
                admin: true,
                ..open
            }),
            None
        );
    }

    fn copying(archived: u64, total: u64) -> Seen {
        Seen::Copying {
            archived,
            sent: archived,
            total,
            files: archived / 10,
        }
    }

    fn minutes(minutes: i64) -> chrono::NaiveDateTime {
        at(12) + chrono::Duration::minutes(minutes)
    }

    /// Looks at the copy once a minute through `seen` (one entry per minute from 0) and returns the
    /// minute of the first verdict that is not `Wait`.
    fn first_verdict(seen: &[Seen], stall: i32, finalize: i32) -> Option<(usize, Verdict)> {
        let mut watch = None;
        for (minute, seen) in seen.iter().enumerate() {
            let (next, verdict) = watch_copy(
                watch.as_ref(),
                1,
                *seen,
                minutes(minute as i64),
                stall,
                finalize,
            );
            if verdict != Verdict::Wait {
                return Some((minute, verdict));
            }
            watch = Some(next);
        }

        None
    }

    #[test]
    fn a_moving_copy_is_never_stalled() {
        let seen = (0..60)
            .map(|minute| copying(minute * 10, 1000))
            .collect::<Vec<_>>();
        assert_eq!(first_verdict(&seen, 10, 20), None);
    }

    #[test]
    fn a_copy_without_progress_stalls_after_the_stall_time() {
        let mut seen = vec![copying(100, 1000), copying(200, 1000)];
        seen.extend(std::iter::repeat_n(copying(200, 1000), 20));
        // last moved at minute 1, stalled once more than 10 minutes passed
        assert_eq!(first_verdict(&seen, 10, 20), Some((12, Verdict::Stalled)));

        // the stop before the copy counts too: no counters moving
        let stopping = std::iter::repeat_n(copying(0, 0), 20).collect::<Vec<_>>();
        assert_eq!(
            first_verdict(&stopping, 10, 20),
            Some((11, Verdict::Stalled))
        );
    }

    #[test]
    fn a_transfer_the_old_node_lost_stalls_too() {
        let mut seen = vec![copying(100, 1000)];
        seen.extend(std::iter::repeat_n(Seen::Missing, 20));
        assert_eq!(first_verdict(&seen, 10, 20), Some((11, Verdict::Stalled)));

        // listed again with the same counters is no progress
        let mut back = vec![copying(100, 1000), Seen::Missing];
        back.extend(std::iter::repeat_n(copying(100, 1000), 20));
        assert_eq!(first_verdict(&back, 10, 20), Some((11, Verdict::Stalled)));
    }

    #[test]
    fn a_silent_old_node_gets_time_for_the_size() {
        // 3000 MiB at 5 MiB/s is 10 more minutes
        let total = 3000 * 1024 * 1024;
        let mut seen = vec![copying(0, total)];
        seen.extend(std::iter::repeat_n(Seen::Unreachable, 40));
        assert_eq!(first_verdict(&seen, 10, 20), Some((21, Verdict::Stalled)));
    }

    #[test]
    fn a_finished_copy_waits_for_the_finalize_time() {
        let mut seen = vec![copying(500, 1000), copying(1000, 1000)];
        seen.extend(std::iter::repeat_n(copying(1000, 1000), 40));
        // handed off at minute 1: no stall verdict, only the finalize time
        assert_eq!(
            first_verdict(&seen, 10, 20),
            Some((22, Verdict::Unfinalized))
        );
        assert_eq!(Verdict::Unfinalized.reason(), Some(REASON_UNFINALIZED));
        assert_eq!(Verdict::Stalled.reason(), Some(REASON_STALLED));
        assert_eq!(Verdict::Wait.reason(), None);
    }

    #[test]
    fn the_watch_starts_again_for_a_new_attempt_and_stays_aborted() {
        let (watch, _) = watch_copy(None, 1, copying(100, 1000), minutes(0), 10, 20);
        let (next, verdict) = watch_copy(Some(&watch), 2, copying(100, 1000), minutes(30), 10, 20);
        assert_eq!(
            (next.attempt, next.moved_at, verdict),
            (2, minutes(30), Verdict::Wait)
        );

        let aborted = Watch {
            aborted: Some((REASON_STALLED, minutes(0))),
            ..watch
        };
        let (kept, verdict) =
            watch_copy(Some(&aborted), 1, copying(100, 1000), minutes(30), 10, 20);
        assert_eq!((kept, verdict), (aborted, Verdict::Wait));

        // tiny settings are held to the Pelican panel's floors
        let (watch, _) = watch_copy(None, 1, copying(1, 10), minutes(0), 0, 0);
        assert_eq!(
            watch_copy(Some(&watch), 1, copying(1, 10), minutes(2), 0, 0).1,
            Verdict::Wait
        );
        assert_eq!(
            watch_copy(Some(&watch), 1, copying(1, 10), minutes(3), 0, 0).1,
            Verdict::Stalled
        );
    }

    #[test]
    fn wings_gets_two_minutes_to_report_a_cancel() {
        assert!(!abort_overdue(minutes(0), minutes(1)));
        assert!(abort_overdue(minutes(0), minutes(2)));
        assert_eq!(reason_key(REASON_STALLED), None);
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
