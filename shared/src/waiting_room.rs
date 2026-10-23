//! plugged.host: the start waiting room, start gate 30 (docs/features/ads.md#the-waiting-room).
//!
//! A start (or restart) of a server over the size line, by someone who sees ads, opens a waiting
//! room with ads in front of the console. The start goes to Wings on the click, so the room is the
//! ad time the start is paid with, not a queue: it lets the user through once the server runs and
//! `room_min_seconds` passed, or after `room_max_seconds` whatever the server does. Only when the
//! click's power action did not reach Wings is it sent again on the way out. Leaving a room opens a
//! pass: the same user starts the same server without a room for `room_pass_minutes`.

use crate::{
    ads::Audience,
    models::{
        CreatableModel,
        server::Server,
        server_activity::{CreateServerActivityOptions, ServerActivity},
        user::User,
    },
    start_gates::{Decision, StartGate},
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// A limit over the line: 0 (unlimited) always is, and a line of 0 takes every server.
pub fn over(limit: i64, line: i64) -> bool {
    limit == 0 || limit > line
}

/// Whether a server is big enough for the room.
pub fn server_over(settings: &crate::settings::ads::AppSettingsAds, server: &Server) -> bool {
    over(server.cpu as i64, settings.room_cpu as i64)
        || over(server.memory, settings.room_memory)
        || over(server.disk, settings.room_disk)
}

/// Whether the room may let the user through, `elapsed` seconds after it opened.
pub fn ready(elapsed: i64, running: bool, min_seconds: i32, max_seconds: i32) -> bool {
    elapsed >= max_seconds as i64 || (running && elapsed >= min_seconds as i64)
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

/// One user's room for one server.
struct Room {
    action: wings_api::ServerPowerAction,
    opened: chrono::NaiveDateTime,
    dispatched: bool,
    released: Option<chrono::NaiveDateTime>,
}

async fn room(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
) -> Result<Option<Room>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT action, opened, dispatched, released FROM server_waiting_rooms
        WHERE server_uuid = $1 AND user_uuid = $2",
    )
    .bind(server_uuid)
    .bind(user_uuid)
    .fetch_optional(database.write())
    .await?;

    row.map(|row| {
        Ok(Room {
            action: action_from(row.try_get::<&str, _>("action")?),
            opened: row.try_get("opened")?,
            dispatched: row.try_get("dispatched")?,
            released: row.try_get("released")?,
        })
    })
    .transpose()
}

async fn log(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
    event: &'static str,
    data: serde_json::Value,
) {
    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid,
            user_uuid: Some(user_uuid),
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

async fn power(
    state: &crate::State,
    server: &Server,
    action: wings_api::ServerPowerAction,
) -> Result<(), anyhow::Error> {
    server
        .node
        .fetch_cached(&state.database)
        .await?
        .api_client(&state.database)
        .await?
        .post_servers_server_power(
            server.uuid,
            &wings_api::servers_server_power::post::RequestBody {
                action,
                wait_seconds: None,
            },
        )
        .await?;

    Ok(())
}

/// The server's state on Wings; `None` when Wings does not answer.
async fn wings_state(state: &crate::State, server: &Server) -> Option<wings_api::ServerState> {
    let node = server.node.fetch_cached(&state.database).await.ok()?;
    let client = node.api_client(&state.database).await.ok()?;

    client
        .get_servers_server(server.uuid)
        .await
        .ok()
        .map(|server| server.state)
}

/// Whether there is anything to show in a room: a zone, the native unit, a page-level script or a
/// sponsored banner. With nothing to show a room is only a wait.
async fn has_ads(
    state: &crate::State,
    settings: &crate::settings::ads::AppSettingsAds,
) -> Result<bool, anyhow::Error> {
    let api = crate::ads::ApiAdsSettings::new(settings);
    if !api.zones.is_empty()
        || api.native
        || api.room_script.is_some()
        || api.popunder_script.is_some()
        || api.social_bar_script.is_some()
    {
        return Ok(true);
    }

    Ok(!crate::models::ad_banner::AdBanner::live(&state.database)
        .await?
        .is_empty())
}

/// Start gate 30 (see the module docs).
pub struct RoomGate;

#[async_trait::async_trait]
impl StartGate for RoomGate {
    fn name(&self) -> &'static str {
        "ads"
    }

    async fn check(
        &self,
        state: &crate::State,
        server: &Server,
        user: &User,
        action: wings_api::ServerPowerAction,
    ) -> Result<Decision, anyhow::Error> {
        let settings = state.settings.get().await?;
        let ads = settings.ads.clone();
        drop(settings);

        if !ads.enabled
            || !ads.room_enabled
            || user.admin
            || server.suspended
            || server.status.is_some()
            || !server_over(&ads, server)
            || crate::ads::audience(state, user).await? == Audience::None
            || !has_ads(state, &ads).await?
        {
            return Ok(Decision::Pass);
        }

        // the pass from the last room
        if let Some(room) = room(&state.database, server.uuid, user.uuid).await?
            && room.released.is_some_and(|released| {
                chrono::Utc::now().naive_utc() - released
                    < chrono::Duration::minutes(ads.room_pass_minutes as i64)
            })
        {
            return Ok(Decision::Pass);
        }

        // Start on a running server does nothing and costs no room; a node that does not answer
        // gets no room either
        let current = wings_state(state, server).await;
        if current.is_none()
            || (matches!(action, wings_api::ServerPowerAction::Start)
                && !matches!(current, Some(wings_api::ServerState::Offline)))
        {
            return Ok(Decision::Pass);
        }

        let dispatched = match power(state, server, action).await {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(server = %server.uuid, "the waiting room could not start the server on the click: {err:#}");
                false
            }
        };
        sqlx::query(
            "INSERT INTO server_waiting_rooms (server_uuid, user_uuid, action, opened, dispatched,
                released, blocked)
            VALUES ($1, $2, $3, NOW(), $4, NULL, NULL)
            ON CONFLICT (server_uuid, user_uuid) DO UPDATE SET action = EXCLUDED.action,
                opened = NOW(), dispatched = EXCLUDED.dispatched, released = NULL, blocked = NULL",
        )
        .bind(server.uuid)
        .bind(user.uuid)
        .bind(action_name(action))
        .bind(dispatched)
        .execute(state.database.write())
        .await?;
        log(
            state,
            server.uuid,
            user.uuid,
            "server:waiting-room.open",
            serde_json::json!({ "action": action_name(action), "dispatched": dispatched }),
        )
        .await;

        Ok(Decision::Hold {
            url: format!("/server/{:08x}/waiting", server.uuid_short),
        })
    }
}

/// What the waiting room page shows.
#[derive(ToSchema, Serialize)]
pub struct ApiWaitingRoom {
    /// `waiting`, `released` (for two minutes after), or `none`.
    pub status: &'static str,
    /// Seconds since the room opened.
    pub elapsed: i64,
    pub min_seconds: i32,
    pub max_seconds: i32,
    /// The server runs on Wings.
    pub running: bool,
    /// The room may let the user through now.
    pub ready: bool,
}

pub async fn view(
    state: &crate::State,
    server: &Server,
    user: &User,
) -> Result<ApiWaitingRoom, anyhow::Error> {
    let settings = state.settings.get().await?;
    let (min_seconds, max_seconds) = (settings.ads.room_min_seconds, settings.ads.room_max_seconds);
    drop(settings);

    let now = chrono::Utc::now().naive_utc();
    let Some(room) = room(&state.database, server.uuid, user.uuid).await? else {
        return Ok(ApiWaitingRoom {
            status: "none",
            elapsed: 0,
            min_seconds,
            max_seconds,
            running: false,
            ready: false,
        });
    };
    let elapsed = (now - room.opened).num_seconds().max(0);

    if let Some(released) = room.released {
        return Ok(ApiWaitingRoom {
            status: if now - released < chrono::Duration::minutes(2) {
                "released"
            } else {
                "none"
            },
            elapsed,
            min_seconds,
            max_seconds,
            running: false,
            ready: false,
        });
    }

    let running = matches!(
        wings_state(state, server).await,
        Some(wings_api::ServerState::Running)
    );

    Ok(ApiWaitingRoom {
        status: "waiting",
        elapsed,
        min_seconds,
        max_seconds,
        running,
        ready: ready(elapsed, running, min_seconds, max_seconds),
    })
}

/// Why a room could not be left.
#[derive(Debug)]
pub enum LeaveError {
    NotWaiting,
    NotReady,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for LeaveError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<sqlx::Error> for LeaveError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

/// Lets the user through once the room is over (checked here, on the panel's clock), opens the
/// pass and sends the power action if the click's did not reach Wings. `blocked` is the page's
/// ad-block reading, kept on the activity entry.
pub async fn leave(
    state: &crate::State,
    server: &Server,
    user: &User,
    blocked: Option<bool>,
) -> Result<(), LeaveError> {
    let view = view(state, server, user).await?;
    if view.status != "waiting" {
        return Err(LeaveError::NotWaiting);
    }
    if !view.ready {
        return Err(LeaveError::NotReady);
    }
    let Some(room) = room(&state.database, server.uuid, user.uuid).await? else {
        return Err(LeaveError::NotWaiting);
    };

    let claimed = sqlx::query(
        "UPDATE server_waiting_rooms SET released = NOW(), blocked = $3
        WHERE server_uuid = $1 AND user_uuid = $2 AND released IS NULL",
    )
    .bind(server.uuid)
    .bind(user.uuid)
    .bind(blocked)
    .execute(state.database.write())
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Err(LeaveError::NotWaiting);
    }

    let mut dispatched = if room.dispatched { "click" } else { "none" };
    if !room.dispatched && !server.suspended && server.status.is_none() {
        match power(state, server, room.action).await {
            Ok(()) => dispatched = "release",
            Err(err) => {
                tracing::warn!(server = %server.uuid, "the waiting room could not start the server: {err:#}")
            }
        }
    }
    log(
        state,
        server.uuid,
        user.uuid,
        "server:waiting-room.release",
        serde_json::json!({
            "waited": view.elapsed,
            "dispatched": dispatched,
            "blocked": blocked,
        }),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_zero_takes_every_server_and_unlimited_is_always_over() {
        assert!(over(100, 0));
        assert!(over(0, 2048));
        assert!(over(4096, 2048));
        assert!(!over(2048, 2048));
        assert!(!over(1024, 2048));
    }

    #[test]
    fn the_room_waits_for_the_server_or_the_longest_stay() {
        assert!(!ready(5, true, 15, 30));
        assert!(ready(15, true, 15, 30));
        assert!(!ready(20, false, 15, 30));
        assert!(ready(30, false, 15, 30));
    }

    #[test]
    fn actions_round_trip() {
        for action in [
            wings_api::ServerPowerAction::Start,
            wings_api::ServerPowerAction::Restart,
        ] {
            assert_eq!(
                action_name(action_from(action_name(action))),
                action_name(action)
            );
        }
    }
}
