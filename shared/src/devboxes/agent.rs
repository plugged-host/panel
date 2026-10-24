//! plugged.host: what a running devbox (`ph`) asks the panel (docs/features/devboxes.md).
//! Authenticated by the bearer token of its current start, which the people in it can read: every
//! answer is limited to that devbox and checks access again, and nothing here can start, extend
//! or reconfigure a devbox.
//!
//! What `ph` may do with the game server: in a personal devbox, what its person holds on the game
//! server themselves; in a shared one, what the owner allowed everyone in it. Starts go through
//! the start gates like the Start button, so a websocket token never carries them.

use super::{Context, Devbox, SharedRights};
use crate::models::{ByUuid, server::Server};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// The longest a boot report's fields may be (the columns).
const VERSION_LENGTH: usize = 40;
const STATUS_LENGTH: usize = 32;
const DETAIL_LENGTH: usize = 255;
const HOST_KEY_LENGTH: usize = 100;
/// Seconds between the "someone works in it" entries on the game server.
const ACTIVE_EVERY: u64 = 3600;

#[derive(ToSchema, Serialize)]
pub struct ApiAccessUser {
    pub username: String,
    /// The public keys on their account, OpenSSH format.
    pub keys: Vec<String>,
}

/// Who may sign in to a devbox.
#[derive(ToSchema, Serialize, Default)]
pub struct ApiAccess {
    pub users: Vec<ApiAccessUser>,
}

fn clip(value: Option<&str>, length: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(length).collect())
}

/// The people who may sign in, with their keys.
pub async fn access(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
) -> Result<ApiAccess, anyhow::Error> {
    let Some(game) = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?
    else {
        return Ok(ApiAccess::default());
    };
    let people = super::people(state, context, devbox, &game).await?;
    let uuids: Vec<uuid::Uuid> = people.iter().map(|person| person.uuid).collect();

    let rows =
        sqlx::query("SELECT user_uuid, public_key FROM user_ssh_keys WHERE user_uuid = ANY($1)")
            .bind(&uuids)
            .fetch_all(state.database.read())
            .await?;
    let mut keys: std::collections::HashMap<uuid::Uuid, Vec<String>> = Default::default();
    for row in rows {
        let bytes: Vec<u8> = row.try_get("public_key")?;
        // a key that can't be read can't sign in either
        if let Ok(key) = russh::keys::PublicKey::from_bytes(&bytes)
            && let Ok(line) = key.to_openssh()
        {
            keys.entry(row.try_get("user_uuid")?)
                .or_default()
                .push(line);
        }
    }

    Ok(ApiAccess {
        users: people
            .into_iter()
            .map(|person| ApiAccessUser {
                keys: keys.remove(&person.uuid).unwrap_or_default(),
                username: person.username,
            })
            .collect(),
    })
}

/// `ph serve` came up: its version, whether `/server` is there, and its host key.
pub async fn boot(
    state: &crate::State,
    devbox: &Devbox,
    version: Option<&str>,
    mount: Option<&str>,
    detail: Option<&str>,
    host_key: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE devboxes SET version = $2, boot_status = $3, boot_detail = $4, host_key = $5,
            booted = NOW(), last_seen = NOW()
        WHERE uuid = $1",
    )
    .bind(devbox.uuid)
    .bind(clip(version, VERSION_LENGTH))
    .bind(clip(mount, STATUS_LENGTH).unwrap_or_else(|| "unknown".to_string()))
    .bind(clip(detail, DETAIL_LENGTH))
    .bind(clip(host_key, HOST_KEY_LENGTH))
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// Every minute or so from `ph serve`: how many sessions are open. While someone works in it, an
/// entry on the game server once an hour: its owner sees it, and limbo (which goes by the
/// server's last activity) never parks the server from under them.
pub async fn heartbeat(
    state: &crate::State,
    devbox: &Devbox,
    connections: i64,
) -> Result<(), sqlx::Error> {
    let connections = connections.clamp(0, 1000) as i32;
    sqlx::query(
        "UPDATE devboxes SET connections = $2, last_seen = NOW(),
            last_used = CASE WHEN $2 > 0 THEN NOW() ELSE last_used END
        WHERE uuid = $1",
    )
    .bind(devbox.uuid)
    .bind(connections)
    .execute(state.database.write())
    .await?;

    if connections > 0 {
        let key = format!("devboxes::active::{}", devbox.uuid);
        if !state.cache.exists(&key).await.unwrap_or(true)
            && state.cache.set(&key, ACTIVE_EVERY, &true).await.is_ok()
            && let Ok(Some(game)) =
                Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await
        {
            super::log_game(
                state,
                game.uuid,
                Some(actor(devbox, &game)),
                "server:devbox.active",
                serde_json::json!({ "shared": devbox.is_shared() }),
            )
            .await;
        }
    }

    Ok(())
}

/// A connect code typed as the SSH password: whether it is `username`'s for this start.
pub async fn authenticate(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    username: &str,
    code: &str,
    ip: &str,
) -> Result<bool, anyhow::Error> {
    if username.is_empty() || code.is_empty() || code.len() > 256 || username.len() > 191 {
        return Ok(false);
    }
    let Some(game) = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?
    else {
        return Ok(false);
    };

    let people = super::people(state, context, devbox, &game).await?;
    let Some(person) = people
        .iter()
        .find(|person| person.username.eq_ignore_ascii_case(username))
    else {
        return Ok(false);
    };
    if !super::codes::check(state, devbox, person.uuid, code).await {
        return Ok(false);
    }

    sqlx::query("UPDATE devboxes SET last_used = NOW() WHERE uuid = $1")
        .bind(devbox.uuid)
        .execute(state.database.write())
        .await?;
    let ip: String = ip.chars().take(64).collect();
    super::log_game(
        state,
        game.uuid,
        Some(person.uuid),
        "server:devbox.connect",
        serde_json::json!({ "ip": ip, "shared": devbox.is_shared() }),
    )
    .await;

    Ok(true)
}

/// Who acts on the game server: the person of a personal devbox, the owner for a shared one.
pub fn actor(devbox: &Devbox, game: &Server) -> uuid::Uuid {
    devbox.user_uuid.unwrap_or(game.owner.uuid)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameAction {
    Status,
    Start,
    Stop,
    Restart,
    Kill,
}

impl GameAction {
    pub fn parse(action: &str) -> Option<Self> {
        Some(match action {
            "start" => Self::Start,
            "stop" => Self::Stop,
            "restart" => Self::Restart,
            "kill" => Self::Kill,
            _ => return None,
        })
    }

    pub fn power(self) -> Option<wings_api::ServerPowerAction> {
        Some(match self {
            Self::Status => return None,
            Self::Start => wings_api::ServerPowerAction::Start,
            Self::Stop => wings_api::ServerPowerAction::Stop,
            Self::Restart => wings_api::ServerPowerAction::Restart,
            Self::Kill => wings_api::ServerPowerAction::Kill,
        })
    }

    fn permission(self) -> Option<&'static str> {
        match self {
            Self::Status => None,
            Self::Start => Some("control.start"),
            Self::Restart => Some("control.restart"),
            Self::Stop | Self::Kill => Some("control.stop"),
        }
    }
}

fn shared_may(rights: &SharedRights, action: GameAction) -> bool {
    match action {
        GameAction::Status => rights.status,
        _ => rights.power,
    }
}

/// Whether `ph` in this devbox may do `action` on the game server.
pub async fn may(
    state: &crate::State,
    devbox: &Devbox,
    game: &Server,
    action: GameAction,
) -> Result<bool, sqlx::Error> {
    if devbox.is_shared() {
        return Ok(shared_may(&devbox.shared_rights, action));
    }
    let Some(permission) = action.permission() else {
        return Ok(true);
    };

    super::holds(&state.database, game, actor(devbox, game), permission).await
}

/// What a shared devbox's websocket token carries.
fn shared_socket_permissions(rights: &SharedRights) -> Vec<&'static str> {
    let mut permissions = Vec::new();
    if rights.logs || rights.console {
        permissions.extend([
            "websocket.connect",
            "meta.calagopus",
            "control.read-console",
        ]);
    }
    if rights.console {
        permissions.push("control.console");
    }
    if rights.power && !permissions.is_empty() {
        permissions.push("control.stop");
    }

    permissions
}

/// What the devbox's websocket token for the game server carries: the console, its lines, stop.
/// Never start or restart.
pub async fn socket_permissions(
    state: &crate::State,
    devbox: &Devbox,
    game: &Server,
) -> Result<Vec<&'static str>, sqlx::Error> {
    if devbox.is_shared() {
        return Ok(shared_socket_permissions(&devbox.shared_rights));
    }

    let actor = actor(devbox, game);
    let mut permissions = vec!["websocket.connect", "meta.calagopus"];
    for permission in ["control.read-console", "control.console", "control.stop"] {
        if super::holds(&state.database, game, actor, permission).await? {
            permissions.push(permission);
        }
    }
    if !permissions.contains(&"control.read-console") {
        return Ok(Vec::new());
    }

    Ok(permissions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_rights_limit_what_ph_may_do() {
        let rights = SharedRights::default();

        assert!(shared_may(&rights, GameAction::Status));
        assert!(!shared_may(&rights, GameAction::Start));
        assert!(!shared_may(&rights, GameAction::Kill));
        assert!(shared_may(
            &SharedRights {
                power: true,
                ..rights
            },
            GameAction::Restart
        ));
    }

    #[test]
    fn shared_tokens_never_carry_starts() {
        let all = SharedRights {
            status: true,
            logs: true,
            power: true,
            console: true,
        };
        let permissions = shared_socket_permissions(&all);

        assert!(permissions.contains(&"control.console"));
        assert!(permissions.contains(&"control.stop"));
        assert!(!permissions.contains(&"control.start"));
        assert!(!permissions.contains(&"control.restart"));

        let none = SharedRights {
            status: true,
            logs: false,
            power: true,
            console: false,
        };
        assert!(shared_socket_permissions(&none).is_empty());
    }

    #[test]
    fn boot_reports_are_cut_to_their_columns() {
        assert_eq!(clip(Some("  "), 10), None);
        assert_eq!(
            clip(Some(&"x".repeat(50)), VERSION_LENGTH).unwrap().len(),
            40
        );
    }
}
