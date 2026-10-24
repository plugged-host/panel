//! plugged.host: the devbox reconcile, every minute (docs/features/devboxes.md). A devbox can't
//! stop itself (Wings would restart it), so this:
//!
//! - stops devboxes nobody is connected to (idle minutes), that ran max hours, whose people lost
//!   access (membership, permission, an abuse finding) or whose game server's disk is full;
//! - starts a shared devbox over when someone who could sign in at its start lost access;
//! - finishes stops Wings has not carried out yet (asked again, killed after a minute);
//! - removes devboxes whose game server is gone or moved to another node (failover moves servers
//!   without asking), personal ones on a server whose shared devbox is on, and those unused for
//!   the cleanup days, which frees their disk and port.
//!
//! It also asks Wings what really runs (one server listing per node that has a devbox): a devbox
//! container running without a devbox that may run, a row or a panel server is killed. So a stop
//! that never reached Wings, or a row removed by hand, can't leave a shell on someone's files.

use super::{
    Context, Devbox, DevboxError,
    power::{self, WingsState},
};
use crate::models::{ByUuid, node::Node, server::Server};
use std::collections::{HashMap, HashSet};

/// Minutes Wings may report a booted devbox stopped before the panel stops it too.
const STOPPED_MINUTES: i64 = 3;

struct Listed {
    state: WingsState,
    egg: uuid::Uuid,
    disk: u64,
    node: uuid::Uuid,
}

/// What Wings runs on every node that has a devbox server, by server, and the nodes that
/// answered.
struct Listing {
    servers: HashMap<uuid::Uuid, Listed>,
    answered: HashSet<uuid::Uuid>,
}

impl Listing {
    /// Wings' state of a server: `None` when its node didn't answer (the stop asks Wings itself),
    /// missing when the node answered without it.
    fn known(&self, server: &Server) -> Option<WingsState> {
        match self.servers.get(&server.uuid) {
            Some(listed) => Some(listed.state),
            None if self.answered.contains(&server.node.uuid) => Some(WingsState::Missing),
            None => None,
        }
    }
}

async fn listing(state: &crate::State, context: &Context) -> Result<Listing, anyhow::Error> {
    let nodes: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT servers.node_uuid FROM servers
        WHERE servers.uuid IN (SELECT server_uuid FROM devboxes)
            OR ($1::uuid IS NOT NULL AND servers.egg_uuid = $1)",
    )
    .bind(context.devboxes.egg_uuid)
    .fetch_all(state.database.read())
    .await?;

    let mut listing = Listing {
        servers: HashMap::new(),
        answered: HashSet::new(),
    };
    for node_uuid in nodes {
        let Some(node) = Node::by_uuid_optional(&state.database, node_uuid).await? else {
            continue;
        };
        let servers = match node.api_client(&state.database).await?.get_servers().await {
            Ok(servers) => servers,
            Err(err) => {
                tracing::debug!(node = %node.uuid, "devbox reconcile: Wings did not answer: {err:?}");
                continue;
            }
        };

        listing.answered.insert(node.uuid);
        for server in servers {
            listing.servers.insert(
                server.configuration.uuid,
                Listed {
                    state: server.state.into(),
                    egg: server.configuration.egg.id,
                    disk: server.utilization.disk_bytes,
                    node: node.uuid,
                },
            );
        }
    }

    Ok(listing)
}

pub async fn run(state: &crate::State) -> Result<(), anyhow::Error> {
    let context = Context::load(state).await?;
    let devboxes = Devbox::all(&state.database).await?;
    if devboxes.is_empty() && context.devboxes.egg_uuid.is_none() {
        return Ok(());
    }
    let listing = listing(state, &context).await?;

    for devbox in &devboxes {
        let result = if context.ready() {
            reconcile(state, &context, devbox, &listing).await
        } else {
            // switched off (or abuse protection is): stop whatever still runs, nothing else
            wind_down(state, devbox, &listing).await
        };

        match result {
            Ok(()) => {}
            // known and retried next minute: Wings down, a devbox kept for an abuse review
            Err(err @ (DevboxError::NodeDown | DevboxError::Abuse)) => {
                tracing::debug!(devbox = %devbox.uuid, "devbox reconcile: not now: {}", err.message());
            }
            Err(err) => {
                tracing::warn!(devbox = %devbox.uuid, "devbox reconcile failed: {}", err.message());
            }
        }
    }

    kill_strays(state, &context, &listing).await;

    Ok(())
}

async fn server_of(state: &crate::State, devbox: &Devbox) -> Result<Option<Server>, DevboxError> {
    Ok(match devbox.server_uuid {
        Some(uuid) => Server::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    })
}

async fn wind_down(
    state: &crate::State,
    devbox: &Devbox,
    listing: &Listing,
) -> Result<(), DevboxError> {
    if devbox.token_hash.is_none() {
        return Ok(());
    }
    if devbox.stopping.is_none() {
        return power::stop(state, devbox).await.map(|_| ());
    }

    let known = server_of(state, devbox)
        .await?
        .and_then(|server| listing.known(&server));
    power::settle(state, devbox, known).await.map(|_| ())
}

async fn reconcile(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    listing: &Listing,
) -> Result<(), DevboxError> {
    let now = chrono::Utc::now().naive_utc();
    let game = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?;
    let server = server_of(state, devbox).await?;

    let remove = match &game {
        None => Some("its game server is gone"),
        Some(game)
            if server
                .as_ref()
                .is_some_and(|server| server.node.uuid != game.node.uuid) =>
        {
            Some("its game server moved to another node")
        }
        Some(game) => match devbox.user_uuid {
            Some(user_uuid) if !user_exists(state, user_uuid).await? => Some("its person is gone"),
            Some(_)
                if Devbox::shared_of(&state.database, game.uuid)
                    .await?
                    .is_some() =>
            {
                Some("the server's shared devbox is on")
            }
            _ if devbox.token_hash.is_none()
                && devbox.last_used.unwrap_or(devbox.created)
                    < now - chrono::Duration::days(context.devboxes.cleanup_days.max(1) as i64) =>
            {
                Some("unused for the cleanup days")
            }
            _ => None,
        },
    };
    // one under abuse review is kept (stopped below) until an admin closes its findings: they,
    // the evidence, would go with its server
    if let Some(reason) = remove
        && !super::blocks::under_review(&state.database, devbox).await?
    {
        tracing::info!(devbox = %devbox.uuid, "devbox reconcile: remove ({reason})");
        return super::provision::destroy(state, context, devbox).await;
    }

    if devbox.is_stopping() {
        let known = server.as_ref().and_then(|server| listing.known(server));
        return power::settle(state, devbox, known).await.map(|_| ());
    }
    let (Some(game), true) = (game, devbox.token_hash.is_some()) else {
        return Ok(()); // stopped
    };
    let Some(server) = server else {
        return power::stop(state, devbox).await.map(|_| ()); // its server is gone
    };

    let people = super::people(state, context, devbox, &game).await?;
    let listed = listing.servers.get(&server.uuid);
    let game_disk = listing.servers.get(&game.uuid).map(|listed| listed.disk);
    let idle_since = devbox.last_used.or(devbox.started);
    let minutes_ago = |minutes: i64| now - chrono::Duration::minutes(minutes);

    let stop = if people.is_empty() {
        Some("nobody may use it any more")
    } else if game.suspended {
        Some("its game server is suspended")
    } else if server.suspended {
        Some("it is suspended")
    } else if super::blocks::under_review(&state.database, devbox).await? {
        Some("an abuse finding on it")
    } else if devbox.started.is_some_and(|started| {
        started < now - chrono::Duration::hours(context.devboxes.max_hours.max(1) as i64)
    }) {
        Some("it ran the most hours")
    } else if devbox.connections == 0
        && idle_since
            .is_some_and(|since| since < minutes_ago(context.devboxes.idle_minutes.max(1) as i64))
    {
        Some("idle")
    } else if devbox.booted.is_none()
        && devbox
            .started
            .is_some_and(|started| started < minutes_ago(super::BOOT_MINUTES))
    {
        // started but never reported back (the image pull failed, the node is down)
        Some("it never came up")
    } else if game_disk.is_some() && power::game_disk_full(state, &game, game_disk).await {
        Some("its game server's disk is full")
    } else if listed.is_some_and(|listed| {
        matches!(listed.state, WingsState::Offline)
            && devbox
                .last_seen
                .or(devbox.booted)
                .is_some_and(|seen| seen < minutes_ago(STOPPED_MINUTES))
    }) {
        // stopped behind the panel's back (a node reboot, Wings' crash detection gave up)
        Some("Wings has it stopped")
    } else {
        None
    };
    if let Some(reason) = stop {
        tracing::info!(devbox = %devbox.uuid, "devbox reconcile: stop ({reason})");
        return power::stop(state, devbox).await.map(|_| ());
    }

    // someone who could sign in at the start lost access: a shared devbox starts over, since they
    // shared its Linux user and could have left something running
    if devbox.is_shared() && lost_someone(&devbox.people, &people) {
        tracing::info!(devbox = %devbox.uuid, "devbox reconcile: restart (someone lost access)");
        return power::restart(state, context, devbox).await;
    }

    Ok(())
}

fn lost_someone(at_start: &[uuid::Uuid], now: &[super::Person]) -> bool {
    at_start
        .iter()
        .any(|uuid| !now.iter().any(|person| person.uuid == *uuid))
}

async fn user_exists(state: &crate::State, user_uuid: uuid::Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE uuid = $1)")
        .bind(user_uuid)
        .fetch_one(state.database.read())
        .await
}

/// Devbox containers Wings runs that nothing in the panel lets run: no panel server, no devbox
/// row, a devbox that is stopped, or another node than the panel says. Killed, not stopped:
/// nobody may be in them.
async fn kill_strays(state: &crate::State, context: &Context, listing: &Listing) {
    let Some(egg_uuid) = context.devboxes.egg_uuid else {
        return;
    };

    for (uuid, listed) in &listing.servers {
        if listed.egg != egg_uuid
            || !matches!(listed.state, WingsState::Running | WingsState::Starting)
        {
            continue;
        }

        // read now, not with the run's list: a devbox made or started since then is no stray
        let (Ok(server), Ok(devbox)) = (
            Server::by_uuid_optional(&state.database, *uuid).await,
            Devbox::by_server_uuid(&state.database, *uuid).await,
        ) else {
            continue;
        };
        let why = match (&server, &devbox) {
            (None, _) => "no server in the panel",
            (Some(_), None) => "no devbox row",
            (Some(_), Some(devbox)) if devbox.token_hash.is_none() => "its devbox is stopped",
            (Some(server), Some(_)) if server.node.uuid != listed.node => {
                "it runs on another node than the panel says"
            }
            _ => continue,
        };
        tracing::warn!(server = %uuid, node = %listed.node, "devbox reconcile: kill a devbox container ({why})");

        let killed = async {
            Node::by_uuid_optional(&state.database, listed.node)
                .await?
                .ok_or_else(|| anyhow::anyhow!("node gone"))?
                .api_client(&state.database)
                .await?
                .post_servers_server_power(
                    *uuid,
                    &wings_api::servers_server_power::post::RequestBody {
                        action: wings_api::ServerPowerAction::Kill,
                        wait_seconds: None,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!("{err:?}"))?;

            Ok::<_, anyhow::Error>(())
        };
        if let Err(err) = killed.await {
            tracing::warn!(server = %uuid, "failed to kill a devbox container: {err:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_devbox_restarts_when_someone_lost_access() {
        let alice = uuid::Uuid::from_u128(1);
        let bob = uuid::Uuid::from_u128(2);
        let person = |uuid| super::super::Person {
            uuid,
            username: String::new(),
        };

        assert!(
            !lost_someone(&[alice], &[person(alice), person(bob)]),
            "someone new"
        );
        assert!(lost_someone(&[alice, bob], &[person(alice)]));
        assert!(!lost_someone(&[], &[person(alice)]));
    }
}
