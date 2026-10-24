//! plugged.host: starting and stopping devboxes (docs/features/devboxes.md). Every start gets a
//! new bearer token (the devbox's only way to talk to the panel; only its sha256 is kept) and a
//! new code secret (the connect codes, [`super::codes`]); a stop retires both. A devbox never
//! stops itself (Wings restarts a clean exit), so idle and max-runtime stops come from here.
//!
//! [`start`] checks everything again itself (who asks, the membership, the running cap, the game
//! server, an abuse block, the node), whatever the caller checked: it is the one way a devbox gets
//! a token.
//!
//! A stop never fails open: [`stop`] marks the devbox stopping (from then on its token is refused
//! and `ph` ends every session at its next heartbeat) and asks Wings; the token is only retired
//! once Wings reports the devbox offline ([`settle`]). Until then the reconcile asks again every
//! minute, and kills it once a stop took longer than [`KILL_AFTER_SECONDS`].

use super::{Context, Devbox, DevboxError};
use crate::models::{ByUuid, nest_egg_variable::NestEggVariable, server::Server};
use rand::distr::SampleString;

/// Seconds after a stop request before the devbox is killed instead.
pub const KILL_AFTER_SECONDS: i64 = 60;
/// Seconds the start lock is held at most (one per payer, around the running cap).
const LOCK_SECONDS: u64 = 60;

/// What Wings says about a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WingsState {
    Running,
    Starting,
    Stopping,
    Offline,
    /// Wings doesn't have the server.
    Missing,
}

impl From<wings_api::ServerState> for WingsState {
    fn from(state: wings_api::ServerState) -> Self {
        match state {
            wings_api::ServerState::Offline => Self::Offline,
            wings_api::ServerState::Starting => Self::Starting,
            wings_api::ServerState::Stopping => Self::Stopping,
            wings_api::ServerState::Running => Self::Running,
        }
    }
}

/// The server's state on Wings with its disk use; `None` when Wings can't be asked.
pub async fn wings_details(
    state: &crate::State,
    server: &Server,
) -> Option<(WingsState, Option<u64>)> {
    let node = server.node.fetch_cached(&state.database).await.ok()?;
    let client = node.api_client(&state.database).await.ok()?;

    match client.get_servers_server(server.uuid).await {
        Ok(details) => Some((details.state.into(), Some(details.utilization.disk_bytes))),
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            Some((WingsState::Missing, None))
        }
        Err(_) => None,
    }
}

pub async fn wings_state(state: &crate::State, server: &Server) -> Option<WingsState> {
    wings_details(state, server).await.map(|(state, _)| state)
}

/// Whether the game server already uses all the disk it may: the devbox writes into its volume,
/// which Wings only measures for the game server itself. Goes by the latest figure Wings has
/// (`disk_bytes` when the caller has it); unknown passes.
pub async fn game_disk_full(state: &crate::State, game: &Server, disk_bytes: Option<u64>) -> bool {
    if game.disk <= 0 {
        return false; // unlimited
    }

    let disk_bytes = match disk_bytes {
        Some(bytes) => Some(bytes),
        None => wings_details(state, game)
            .await
            .and_then(|(_, bytes)| bytes),
    };

    disk_full(game.disk, disk_bytes)
}

fn disk_full(limit_mib: i64, disk_bytes: Option<u64>) -> bool {
    limit_mib > 0
        && disk_bytes.is_some_and(|bytes| bytes >= (limit_mib as u64).saturating_mul(1024 * 1024))
}

async fn power(state: &crate::State, server: &Server, action: wings_api::ServerPowerAction) {
    if let Err(err) = try_power(state, server, action).await {
        tracing::warn!(server = %server.uuid, "devbox power action {action:?} failed: {err:#}");
    }
}

async fn try_power(
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
        .await
        .map_err(|err| anyhow::anyhow!("{err:?}"))?;

    Ok(())
}

async fn server_of(state: &crate::State, devbox: &Devbox) -> Result<Option<Server>, DevboxError> {
    Ok(match devbox.server_uuid {
        Some(uuid) => Server::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    })
}

/// Starts `devbox` for `actor`, who must get exactly this one (their own, or the server's shared
/// one) and whose payer must have the membership for it.
pub async fn start(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    actor: uuid::Uuid,
) -> Result<(), DevboxError> {
    let game = Server::by_uuid_optional(&state.database, devbox.game_server_uuid)
        .await?
        .ok_or(DevboxError::Missing)?;
    let server = server_of(state, devbox)
        .await?
        .ok_or(DevboxError::Missing)?;

    match super::resolve(state, context, actor, &game).await? {
        Some((_, Some(resolved))) if resolved.uuid == devbox.uuid => {}
        _ => return Err(DevboxError::NotAllowed),
    }
    let payer = devbox.payer(&game);
    if let Some(reason) = super::refusal(state, actor, &game, payer).await? {
        return Err(reason);
    }
    if server.suspended {
        // the devbox's own server: only the abuse checks (or an admin) suspend one
        return Err(DevboxError::Abuse);
    }

    // one start at a time per payer, so two clicks (or two servers) never both pass the cap
    let _lock = state
        .cache
        .lock(
            format!("devboxes::start::{payer}"),
            Some(LOCK_SECONDS),
            Some(20_000),
        )
        .await?;

    let mut devbox = devbox.clone();
    devbox.refresh(&state.database).await?;
    if devbox.is_stopping() {
        return Err(DevboxError::Stopping);
    }
    if devbox.token_hash.is_some() {
        return Ok(()); // started meanwhile
    }
    let cap = context.devboxes.running_per_user;
    if Devbox::running_paid_by(&state.database, payer).await?.len() >= cap.max(0) as usize {
        return Err(DevboxError::TooMany(cap));
    }

    // Wings must have it offline: one the panel thinks is stopped but that still runs is killed,
    // never started twice
    match wings_state(state, &server).await {
        None => return Err(DevboxError::NodeDown),
        Some(WingsState::Missing) => return Err(DevboxError::Missing),
        Some(WingsState::Offline) => {}
        Some(_) => {
            power(state, &server, wings_api::ServerPowerAction::Kill).await;
            return Err(DevboxError::Stopping);
        }
    }

    if game_disk_full(state, &game, None).await {
        return Err(DevboxError::DiskFull);
    }

    let people = super::people(state, context, &devbox, &game).await?;
    if people.is_empty() {
        return Err(DevboxError::NotAllowed);
    }

    launch(state, context, &devbox, &server, &people, false).await
}

/// A running devbox starts over with a new token and a new code secret, everything in it ended:
/// what happens to a shared devbox when someone loses access to it (removed, a permission taken,
/// the server changed hands), since they shared its Linux user and could have left something
/// running. Stopped instead when nobody pays for it or may use it any more.
pub async fn restart(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
) -> Result<(), DevboxError> {
    let mut devbox = devbox.clone();
    devbox.refresh(&state.database).await?;
    if !devbox.is_running() {
        return Ok(());
    }

    let game = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?;
    let server = server_of(state, &devbox).await?;
    let (Some(game), Some(server)) = (game, server) else {
        return stop(state, &devbox).await.map(|_| ());
    };

    let payer = devbox.payer(&game);
    let people = super::people(state, context, &devbox, &game).await?;
    if super::tier(state, context, payer).await?.is_none()
        || people.is_empty()
        || super::blocked_reason(state, &game).await?.is_some()
        || super::blocks::blocked(state, payer).await?
    {
        return stop(state, &devbox).await.map(|_| ());
    }

    launch(state, context, &devbox, &server, &people, true).await
}

/// Asks Wings to stop the devbox. `true` once it is offline (token retired); `false` while that
/// is not confirmed yet, and [`settle`] (the reconcile, the card's poll) carries on from there.
pub async fn stop(state: &crate::State, devbox: &Devbox) -> Result<bool, DevboxError> {
    if devbox.token_hash.is_none() {
        if devbox.stopping.is_some() {
            sqlx::query("UPDATE devboxes SET stopping = NULL WHERE uuid = $1")
                .bind(devbox.uuid)
                .execute(state.database.write())
                .await?;
        }

        return Ok(true);
    }

    let mut devbox = devbox.clone();
    if devbox.stopping.is_none() {
        // the codes go now: nobody signs in to a devbox on its way down
        devbox.stopping = sqlx::query_scalar(
            "UPDATE devboxes SET stopping = COALESCE(stopping, NOW()), connections = 0,
                code_secret = NULL
            WHERE uuid = $1
            RETURNING stopping",
        )
        .bind(devbox.uuid)
        .fetch_optional(state.database.write())
        .await?
        .flatten();
    }

    settle(state, &devbox, None).await
}

/// Carries a stop on: retires the token once Wings says offline (or doesn't have the server),
/// asks for the stop again while it runs, kills it once the stop took longer than
/// [`KILL_AFTER_SECONDS`]. `known` is Wings' state when the caller already has it (the
/// reconcile's node listing).
pub async fn settle(
    state: &crate::State,
    devbox: &Devbox,
    known: Option<WingsState>,
) -> Result<bool, DevboxError> {
    let Some(stopping) = devbox.stopping else {
        return Ok(devbox.token_hash.is_none());
    };

    let server = server_of(state, devbox).await?;
    let wings = match (&server, known) {
        (None, _) => Some(WingsState::Missing),
        (Some(_), Some(known)) => Some(known),
        (Some(server), None) => wings_state(state, server).await,
    };

    match (wings, server) {
        (Some(WingsState::Offline | WingsState::Missing), _) => {
            sqlx::query(
                "UPDATE devboxes SET token_hash = NULL, stopping = NULL, connections = 0,
                    code_secret = NULL
                WHERE uuid = $1",
            )
            .bind(devbox.uuid)
            .execute(state.database.write())
            .await?;

            Ok(true)
        }
        // Wings didn't answer: asked again at the next reconcile
        (None, _) | (_, None) => Ok(false),
        (Some(wings), Some(server)) => {
            let waited = chrono::Utc::now().naive_utc() - stopping;
            if waited >= chrono::Duration::seconds(KILL_AFTER_SECONDS) {
                power(state, &server, wings_api::ServerPowerAction::Kill).await;
            } else if wings != WingsState::Stopping {
                power(state, &server, wings_api::ServerPowerAction::Stop).await;
            }

            Ok(false)
        }
    }
}

/// A new token, code secret and web certificate, written where Wings reads them on the next
/// start (Wings fetches the server's configuration from the panel on every start), then the
/// power action. If Wings refuses, the devbox is stopped again: the new token must not stay
/// valid for a devbox that never got it.
async fn launch(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    server: &Server,
    people: &[super::Person],
    restart: bool,
) -> Result<(), DevboxError> {
    let token = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 64);
    let code_secret = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 64);

    let variables = NestEggVariable::all_by_egg_uuid(&state.database, server.egg.uuid).await?;
    let set = async |env: &str, value: &str, optional: bool| -> Result<(), DevboxError> {
        let Some(variable) = variables
            .iter()
            .find(|variable| variable.env_variable == env)
        else {
            // an egg from before the web version has nothing to switch off
            return if optional && value.is_empty() {
                Ok(())
            } else {
                Err(DevboxError::NotSetUp)
            };
        };

        crate::models::server_variable::ServerVariable::create(
            &state.database,
            server.uuid,
            variable.uuid,
            value,
        )
        .await?;

        Ok(())
    };

    set("DEVBOX_TOKEN", &token, false).await?;
    set("DEVBOX_PANEL_URL", &context.panel_url(), false).await?;

    // VS Code in the browser: a fresh certificate for <slug>.<web domain> every start, and the CA
    // the edge's client certificate comes from. Empty switches ph's web side off.
    let web = if context.web_enabled() {
        Some(super::pki::issue(
            &context.devboxes,
            &devbox.web_host(&context.devboxes),
        )?)
    } else {
        None
    };
    let encode = |pem: &str| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(pem)
    };
    set(
        "DEVBOX_TLS_CERT",
        &web.as_ref()
            .map_or_else(String::new, |web| encode(&web.cert)),
        true,
    )
    .await?;
    set(
        "DEVBOX_TLS_KEY",
        &web.as_ref()
            .map_or_else(String::new, |web| encode(&web.key)),
        true,
    )
    .await?;
    set(
        "DEVBOX_EDGE_CA",
        &web.as_ref().map_or_else(String::new, |web| encode(&web.ca)),
        true,
    )
    .await?;
    if web.is_some()
        && let Err(err) = super::dns::ensure(state, context, devbox).await
    {
        // SSH works without it; "Open in browser" waits for the record
        tracing::warn!(devbox = %devbox.uuid, "failed to set a devbox's DNS record: {err:#}");
    }

    // the image follows the settings (a release tag or a pinned digest), for devboxes made before
    // it changed too
    let image = context.devboxes.image.trim();
    if !image.is_empty() && server.image != image {
        sqlx::query("UPDATE servers SET image = $2 WHERE uuid = $1")
            .bind(server.uuid)
            .bind(image)
            .execute(state.database.write())
            .await?;
    }
    Server::invalidate_cached(&state.database, server.uuid).await;

    let code_secret = state.database.encrypt_base64(code_secret).await?;
    sqlx::query(
        "UPDATE devboxes SET token_hash = $2, code_secret = $3, stopping = NULL,
            boot_status = NULL, boot_detail = NULL, booted = NULL, connections = 0,
            started = CASE WHEN $4 THEN COALESCE(started, NOW()) ELSE NOW() END,
            last_used = NOW(), people = $5
        WHERE uuid = $1",
    )
    .bind(devbox.uuid)
    .bind(super::hash_token(&token))
    .bind(code_secret.as_str())
    // max_hours counts from the first start: a restart for revocation doesn't extend it
    .bind(restart)
    .bind(people.iter().map(|person| person.uuid).collect::<Vec<_>>())
    .execute(state.database.write())
    .await?;

    let action = if restart {
        wings_api::ServerPowerAction::Restart
    } else {
        wings_api::ServerPowerAction::Start
    };
    if let Err(err) = try_power(state, server, action).await {
        if let Some(fresh) = Devbox::by_uuid(&state.database, devbox.uuid).await? {
            stop(state, &fresh).await?;
        }

        return Err(DevboxError::Other(err));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_disk_is_full_at_its_limit() {
        let mib = 1024 * 1024;

        assert!(!disk_full(0, Some(u64::MAX)), "unlimited");
        assert!(!disk_full(100, None), "unknown passes");
        assert!(!disk_full(100, Some(99 * mib)));
        assert!(disk_full(100, Some(100 * mib)));
    }
}
