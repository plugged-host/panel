//! plugged.host: the start queue, start gate 20: a start on a busy node waits its turn
//! (docs/features/start-queue.md).
//!
//! A start (and, by setting, a restart) of a user who is not an admin joins its node's line when
//! the node's newest sample is over the line (memory or CPU, lower on a small node) or others are
//! already waiting there; otherwise it goes on to the next gate. The `release_start_queue` task
//! lets the front of each line go once the node, asked right then, is under the line, at most one
//! start per node every `release_interval` seconds, and everyone who waited `max_wait` whatever
//! the node looks like: a queue without a ceiling is an outage with a progress bar. Members go
//! first (the second membership, then the first), then by arrival.
//!
//! A released start goes straight to Wings, past the other gates, and the server's next start
//! within `pass_minutes` skips the queue. One row per server in `server_start_queue`, reused:
//! `waiting`, then `released` or `left`. Moving the server (`server_transfers::request`) leaves
//! the line.

use crate::{
    models::{
        CreatableModel,
        node::Node,
        server::Server,
        server_activity::{CreateServerActivityOptions, ServerActivity},
        user::User,
    },
    prelude::ByUuid,
    settings::{placement::AppSettingsPlacement, queue::AppSettingsQueue},
    start_gates::{Decision, StartGate},
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

pub const WAITING: &str = "waiting";
pub const RELEASED: &str = "released";
pub const LEFT: &str = "left";

/// Why a start was let go.
pub const REASON_ROOM: &str = "room";
pub const REASON_TIMEOUT: &str = "timeout";
pub const REASON_OFF: &str = "off";

/// The line in percent for a node: the queue's, lowered on a small node like the deployment pause.
pub fn line(
    queue: &AppSettingsQueue,
    placement: &AppSettingsPlacement,
    memory_total: Option<i64>,
    threads: Option<i32>,
) -> f32 {
    crate::models::node::placement::pause_line(
        &AppSettingsPlacement {
            pause_percent: queue.line_percent,
            ..placement.clone()
        },
        memory_total,
        threads,
    )
}

/// Whether memory or CPU use is over the line. Unknown figures are not.
pub fn over_line(
    line: f32,
    cpu_percent: Option<f32>,
    memory_used: Option<i64>,
    memory_total: Option<i64>,
) -> bool {
    let memory = match (memory_used, memory_total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 * 100.0 / total as f32),
        _ => None,
    };

    memory.is_some_and(|memory| memory > line) || cpu_percent.is_some_and(|cpu| cpu > line)
}

/// One server's place.
#[derive(Debug, Clone)]
pub struct Entry {
    pub server_uuid: uuid::Uuid,
    pub user_uuid: Option<uuid::Uuid>,
    pub node_uuid: uuid::Uuid,
    pub action: wings_api::ServerPowerAction,
    pub status: String,
    pub priority: i16,
    pub queued: chrono::NaiveDateTime,
    pub released: Option<chrono::NaiveDateTime>,
    pub reason: Option<String>,
}

const ENTRY_COLUMNS: &str =
    "server_uuid, user_uuid, node_uuid, action, status, priority, queued, released, reason";

fn map_entry(row: &sqlx::postgres::PgRow) -> Result<Entry, sqlx::Error> {
    Ok(Entry {
        server_uuid: row.try_get("server_uuid")?,
        user_uuid: row.try_get("user_uuid")?,
        node_uuid: row.try_get("node_uuid")?,
        action: action_from(row.try_get::<&str, _>("action")?),
        status: row.try_get("status")?,
        priority: row.try_get("priority")?,
        queued: row.try_get("queued")?,
        released: row.try_get("released")?,
        reason: row.try_get("reason")?,
    })
}

fn action_name(action: wings_api::ServerPowerAction) -> &'static str {
    match action {
        wings_api::ServerPowerAction::Restart => "restart",
        _ => "start",
    }
}

fn action_from(name: &str) -> wings_api::ServerPowerAction {
    match name {
        "restart" => wings_api::ServerPowerAction::Restart,
        _ => wings_api::ServerPowerAction::Start,
    }
}

impl Entry {
    pub fn waiting(&self) -> bool {
        self.status == WAITING
    }

    /// Seconds waited so far.
    pub fn waited(&self, now: chrono::NaiveDateTime) -> i64 {
        (now - self.queued).num_seconds().max(0)
    }

    /// Released less than `pass_minutes` ago: the next start skips the queue.
    pub fn passes(&self, pass_minutes: i32, now: chrono::NaiveDateTime) -> bool {
        self.status == RELEASED
            && self.released.is_some_and(|released| {
                now - released < chrono::Duration::minutes(pass_minutes as i64)
            })
    }
}

/// The server's row, in whatever state.
pub async fn entry(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Option<Entry>, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {ENTRY_COLUMNS} FROM server_start_queue WHERE server_uuid = $1"
    )))
    .bind(server_uuid)
    .fetch_optional(database.write())
    .await?;

    row.as_ref().map(map_entry).transpose()
}

/// 1 for the front of the line.
pub async fn position(
    database: &crate::database::Database,
    entry: &Entry,
) -> Result<i64, sqlx::Error> {
    let ahead: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM server_start_queue
        WHERE node_uuid = $1 AND status = 'waiting' AND server_uuid <> $2
            AND (priority > $3 OR (priority = $3 AND (queued, server_uuid) < ($4, $2)))",
    )
    .bind(entry.node_uuid)
    .bind(entry.server_uuid)
    .bind(entry.priority)
    .bind(entry.queued)
    .fetch_one(database.read())
    .await?;

    Ok(ahead + 1)
}

/// The account's place in line: 2 with the second membership, 1 with the first, 0 without (and
/// when memberships are off).
async fn priority(state: &crate::State, user_uuid: uuid::Uuid) -> Result<i16, anyhow::Error> {
    if !state.settings.get().await?.economy.membership_enabled {
        return Ok(0);
    }

    let membership = crate::economy::membership(&state.database, user_uuid).await?;

    Ok(membership.priority(chrono::Utc::now().naive_utc()) as i16)
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

/// Start gate 20 (see the module docs).
pub struct QueueGate;

#[async_trait::async_trait]
impl StartGate for QueueGate {
    fn name(&self) -> &'static str {
        "queue"
    }

    async fn check(
        &self,
        state: &crate::State,
        server: &Server,
        user: &User,
        action: wings_api::ServerPowerAction,
    ) -> Result<Decision, anyhow::Error> {
        let settings = state.settings.get().await?;
        let queue = settings.queue.clone();
        let placement = settings.placement.clone();
        drop(settings);

        if !queue.enabled
            || user.admin
            || (matches!(action, wings_api::ServerPowerAction::Restart) && !queue.include_restart)
            || server.suspended
            || server.status.is_some()
        {
            return Ok(Decision::Pass);
        }

        let url = format!("/server/{:08x}/queue", server.uuid_short);
        let now = chrono::Utc::now().naive_utc();
        let node_uuid = server.node.uuid;

        match entry(&state.database, server.uuid).await? {
            // already in line: the same place, never a second one; the second press may be a restart
            Some(entry) if entry.waiting() => {
                let priority = priority(state, user.uuid).await?;
                sqlx::query(
                    "UPDATE server_start_queue SET action = $2, priority = GREATEST(priority, $3),
                        last_seen = NOW()
                    WHERE server_uuid = $1 AND status = 'waiting'",
                )
                .bind(server.uuid)
                .bind(action_name(action))
                .bind(priority)
                .execute(state.database.write())
                .await?;

                return Ok(Decision::Hold { url });
            }
            Some(entry) if entry.passes(queue.pass_minutes, now) => return Ok(Decision::Pass),
            _ => {}
        }

        if !busy(state, node_uuid, &queue, &placement).await? {
            return Ok(Decision::Pass);
        }

        let priority = priority(state, user.uuid).await?;
        sqlx::query(
            "INSERT INTO server_start_queue (server_uuid, user_uuid, node_uuid, action, status,
                priority, queued, last_seen, released, reason)
            VALUES ($1, $2, $3, $4, 'waiting', $5, NOW(), NOW(), NULL, NULL)
            ON CONFLICT (server_uuid) DO UPDATE SET user_uuid = EXCLUDED.user_uuid,
                node_uuid = EXCLUDED.node_uuid, action = EXCLUDED.action, status = 'waiting',
                priority = EXCLUDED.priority, queued = NOW(), last_seen = NOW(), released = NULL,
                reason = NULL
            WHERE server_start_queue.status <> 'waiting'",
        )
        .bind(server.uuid)
        .bind(user.uuid)
        .bind(node_uuid)
        .bind(action_name(action))
        .bind(priority)
        .execute(state.database.write())
        .await?;

        log(
            state,
            server.uuid,
            Some(user.uuid),
            "server:queue.join",
            serde_json::json!({ "action": action_name(action), "node": node_uuid }),
        )
        .await;

        Ok(Decision::Hold { url })
    }
}

/// Whether a start on the node waits: others already do, or its newest sample (from the last few
/// minutes) is over the line.
async fn busy(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    queue: &AppSettingsQueue,
    placement: &AppSettingsPlacement,
) -> Result<bool, anyhow::Error> {
    let others: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM server_start_queue WHERE node_uuid = $1 AND status = 'waiting')",
    )
    .bind(node_uuid)
    .fetch_one(state.database.read())
    .await?;
    if others {
        return Ok(true);
    }

    let Some(latest) = crate::node_telemetry::latest(&state.database, node_uuid).await? else {
        return Ok(false);
    };
    if !latest.reachable || latest.sampled < chrono::Utc::now() - chrono::Duration::minutes(5) {
        return Ok(false);
    }

    Ok(over_line(
        line(queue, placement, latest.memory_total, latest.cpu_threads),
        latest.cpu_percent,
        latest.memory_used,
        latest.memory_total,
    ))
}

/// Takes the server out of the line without starting it. False when it was not waiting.
pub async fn leave(state: &crate::State, server_uuid: uuid::Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE server_start_queue SET status = 'left', released = NOW(), reason = NULL
        WHERE server_uuid = $1 AND status = 'waiting'",
    )
    .bind(server_uuid)
    .execute(state.database.write())
    .await?
    .rows_affected()
        == 1)
}

/// Lets one start go: claims the row, then sends the power action to Wings. The claim is a
/// conditional update, so two releases of the same start (the task and an admin turning the queue
/// off) send it once.
async fn release(
    state: &crate::State,
    entry: &Entry,
    reason: &'static str,
) -> Result<(), anyhow::Error> {
    let claimed = sqlx::query(
        "UPDATE server_start_queue SET status = 'released', released = NOW(), reason = $2
        WHERE server_uuid = $1 AND status = 'waiting'",
    )
    .bind(entry.server_uuid)
    .bind(reason)
    .execute(state.database.write())
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Ok(());
    }

    let Some(server) = Server::by_uuid_optional(&state.database, entry.server_uuid).await? else {
        return Ok(());
    };

    log(
        state,
        server.uuid,
        entry.user_uuid,
        "server:queue.release",
        serde_json::json!({
            "reason": reason,
            "waited": entry.waited(chrono::Utc::now().naive_utc()),
        }),
    )
    .await;

    if server.suspended || server.status.is_some() {
        return Ok(());
    }

    server
        .node
        .fetch_cached(&state.database)
        .await?
        .api_client(&state.database)
        .await?
        .post_servers_server_power(
            server.uuid,
            &wings_api::servers_server_power::post::RequestBody {
                action: entry.action,
                wait_seconds: None,
            },
        )
        .await?;

    Ok(())
}

/// The `release_start_queue` task: see the module docs.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {ENTRY_COLUMNS} FROM server_start_queue WHERE status = 'waiting'
        ORDER BY node_uuid, priority DESC, queued, server_uuid"
    )))
    .fetch_all(state.database.read())
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let entries = rows.iter().map(map_entry).collect::<Result<Vec<_>, _>>()?;

    let settings = state.settings.get().await?;
    let queue = settings.queue.clone();
    let placement = settings.placement.clone();
    drop(settings);

    let now = chrono::Utc::now().naive_utc();

    let mut by_node: Vec<(uuid::Uuid, Vec<&Entry>)> = Vec::new();
    for entry in &entries {
        match by_node.last_mut() {
            Some((node, line)) if *node == entry.node_uuid => line.push(entry),
            _ => by_node.push((entry.node_uuid, vec![entry])),
        }
    }

    for (node_uuid, line_entries) in by_node {
        if let Err(err) =
            settle_node(state, node_uuid, &line_entries, &queue, &placement, now).await
        {
            tracing::warn!(node = %node_uuid, "failed to release the start queue: {err:#}");
        }
    }

    Ok(())
}

async fn settle_node(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    entries: &[&Entry],
    queue: &AppSettingsQueue,
    placement: &AppSettingsPlacement,
    now: chrono::NaiveDateTime,
) -> Result<(), anyhow::Error> {
    let mut rest = Vec::with_capacity(entries.len());
    for entry in entries {
        let reason = if !queue.enabled {
            Some(REASON_OFF)
        } else if entry.waited(now) >= queue.max_wait as i64 {
            Some(REASON_TIMEOUT)
        } else {
            None
        };

        match reason {
            Some(reason) => {
                if let Err(err) = release(state, entry, reason).await {
                    tracing::warn!(server = %entry.server_uuid, "failed to release a queued start: {err:#}");
                }
            }
            None => rest.push(*entry),
        }
    }

    let Some(front) = rest.first() else {
        return Ok(());
    };

    let last: Option<chrono::NaiveDateTime> = sqlx::query_scalar(
        "SELECT MAX(released) FROM server_start_queue WHERE node_uuid = $1 AND status = 'released'",
    )
    .bind(node_uuid)
    .fetch_one(state.database.read())
    .await?;
    if last
        .is_some_and(|last| now - last < chrono::Duration::seconds(queue.release_interval as i64))
    {
        return Ok(());
    }

    // asked right now: the newest sample may be a minute old, and starts change a node quickly
    let node = Node::by_uuid(&state.database, node_uuid).await?;
    let (sample, _) = crate::node_telemetry::probe(&state.database, &node).await;
    if !sample.reachable
        || over_line(
            line(queue, placement, sample.memory_total, sample.cpu_threads),
            sample.cpu_percent,
            sample.memory_used,
            sample.memory_total,
        )
    {
        return Ok(());
    }

    release(state, front, REASON_ROOM).await
}

/// The queue page's view of a server's place.
#[derive(ToSchema, Serialize)]
pub struct ApiStartQueue {
    /// `waiting`, `released`, `left`, or `none` when the server never queued.
    pub status: String,
    /// 1 for the front; 0 when not waiting.
    pub position: i64,
    pub waited: i64,
    /// Seconds until the start goes out however busy the node is.
    pub remaining: i64,
    /// 2 with the second membership, 1 with the first, 0 without.
    pub priority: i16,
    /// The account can get ahead with a (better) membership.
    pub membership_offer: bool,
    /// The Move Server page is offered.
    pub move_offer: bool,
}

/// The server's place for its queue page; a poll also counts a membership bought while waiting.
pub async fn view(
    state: &crate::State,
    server: &Server,
    user: &User,
    can_move: bool,
) -> Result<ApiStartQueue, anyhow::Error> {
    let settings = state.settings.get().await?;
    let queue = settings.queue.clone();
    let membership_enabled = settings.economy.membership_enabled;
    let transfer_enabled = settings.games.transfer_enabled;
    drop(settings);

    let now = chrono::Utc::now().naive_utc();
    let mut entry = entry(&state.database, server.uuid).await?;

    if let Some(entry) = entry.as_mut().filter(|entry| entry.waiting()) {
        let priority = priority(state, user.uuid).await?;
        if priority > entry.priority {
            sqlx::query(
                "UPDATE server_start_queue SET priority = $2, last_seen = NOW()
                WHERE server_uuid = $1 AND status = 'waiting'",
            )
            .bind(server.uuid)
            .bind(priority)
            .execute(state.database.write())
            .await?;
            entry.priority = priority;
        }
    }

    let Some(entry) = entry else {
        return Ok(ApiStartQueue {
            status: "none".into(),
            position: 0,
            waited: 0,
            remaining: 0,
            priority: 0,
            membership_offer: false,
            move_offer: false,
        });
    };

    let waiting = entry.waiting();
    let position = if waiting {
        position(&state.database, &entry).await?
    } else {
        0
    };
    let waited = entry.waited(entry.released.unwrap_or(now));

    // the page follows a release to the console; one from long ago is not news any more
    let status = if entry.status == RELEASED
        && entry
            .released
            .is_none_or(|released| now - released > chrono::Duration::minutes(2))
    {
        "none".to_string()
    } else {
        entry.status.clone()
    };

    Ok(ApiStartQueue {
        status,
        position,
        waited,
        remaining: if waiting {
            (queue.max_wait as i64 - waited).max(0)
        } else {
            0
        },
        priority: entry.priority,
        membership_offer: waiting && membership_enabled && entry.priority < 2,
        move_offer: waiting
            && can_move
            && transfer_enabled
            && queue.move_offer
            && (!queue.move_requires_membership || entry.priority > 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: u32, second: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(20, minute, second)
            .unwrap()
    }

    fn entry(status: &str, released: Option<chrono::NaiveDateTime>) -> Entry {
        Entry {
            server_uuid: uuid::Uuid::nil(),
            user_uuid: None,
            node_uuid: uuid::Uuid::nil(),
            action: wings_api::ServerPowerAction::Start,
            status: status.into(),
            priority: 0,
            queued: at(0, 0),
            released,
            reason: None,
        }
    }

    #[test]
    fn memory_or_cpu_over_the_line_is_busy_and_unknown_is_not() {
        let gib = 1024 * 1024 * 1024;

        assert!(over_line(85.0, Some(20.0), Some(90 * gib), Some(100 * gib)));
        assert!(over_line(85.0, Some(90.0), Some(10 * gib), Some(100 * gib)));
        assert!(!over_line(
            85.0,
            Some(85.0),
            Some(85 * gib),
            Some(100 * gib)
        ));
        assert!(!over_line(85.0, None, None, None));
        assert!(!over_line(85.0, None, Some(90 * gib), Some(0)));
    }

    #[test]
    fn a_small_node_gets_the_placement_offset_on_the_queue_line() {
        let queue = AppSettingsQueue {
            line_percent: 70,
            ..Default::default()
        };
        let placement = AppSettingsPlacement::default();
        let gib = 1024 * 1024 * 1024;

        assert_eq!(line(&queue, &placement, Some(64 * gib), Some(16)), 70.0);
        assert_eq!(line(&queue, &placement, Some(16 * gib), Some(16)), 60.0);
        assert_eq!(line(&queue, &placement, None, None), 70.0);
    }

    #[test]
    fn a_release_lets_the_next_start_through_for_the_pass_minutes() {
        assert!(entry(RELEASED, Some(at(0, 0))).passes(10, at(9, 59)));
        assert!(!entry(RELEASED, Some(at(0, 0))).passes(10, at(10, 0)));
        assert!(!entry(RELEASED, Some(at(0, 0))).passes(0, at(0, 0)));
        assert!(!entry(LEFT, Some(at(0, 0))).passes(10, at(1, 0)));
        assert!(!entry(WAITING, None).passes(10, at(1, 0)));
    }

    #[test]
    fn waiting_time_never_goes_negative() {
        assert_eq!(entry(WAITING, None).waited(at(2, 5)), 125);
        assert_eq!(entry(WAITING, None).waited(at(0, 0)), 0);
    }

    #[test]
    fn actions_round_trip_and_only_restart_is_kept_apart() {
        for name in ["start", "restart"] {
            assert_eq!(action_name(action_from(name)), name);
        }
        assert_eq!(action_name(action_from("anything")), "start");
        assert_eq!(action_name(wings_api::ServerPowerAction::Kill), "start");
    }
}
