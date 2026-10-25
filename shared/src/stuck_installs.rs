//! plugged.host: an install that runs far too long, and the ways out the Pelican panel's installing
//! page offered after 30 minutes: install again, or delete the server
//! (docs/features/server-settings.md#stuck-installs).
//!
//! An install Wings never started (it refused the server, or the node lost it) leaves the server
//! `installing` for good: the console only says it installs, and every other page is blocked.
//! When the install began is not stored; as the Pelican panel used `updated_at`, the newest of the
//! server's creation and the last activity a person caused on it (a reinstall or a software switch
//! logs one, `servers.last_activity`) stands for it. Deleting such a server (or one whose install
//! failed) does not wait for its node to agree, as the Pelican panel forced it.

use crate::models::{
    ByUuid,
    server::{Server, ServerStatus},
};

/// Minutes after which an install counts as stuck, as on the Pelican panel.
pub const STUCK_AFTER_MINUTES: i64 = 30;

/// When the server's install began, as near as the panel knows; `None` unless it is installing.
pub async fn installing_since(
    database: &crate::database::Database,
    server: &Server,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, sqlx::Error> {
    if server.status != Some(ServerStatus::Installing) {
        return Ok(None);
    }

    let last_activity: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT last_activity FROM servers WHERE uuid = $1")
            .bind(server.uuid)
            .fetch_optional(database.read())
            .await?;

    Ok(Some(started(server.created, last_activity).and_utc()))
}

fn started(
    created: chrono::NaiveDateTime,
    last_activity: Option<chrono::NaiveDateTime>,
) -> chrono::NaiveDateTime {
    last_activity.map_or(created, |activity| activity.max(created))
}

/// Whether an install that began at `since` is stuck by `now`.
pub fn stuck(since: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> bool {
    now - since >= chrono::Duration::minutes(STUCK_AFTER_MINUTES)
}

/// Whether deleting the server need not wait for its node to agree, as the Pelican panel's
/// installing page forced it: its install failed, or is stuck (Wings often does not know such a
/// server at all, or its node is gone).
pub async fn delete_without_node(
    database: &crate::database::Database,
    server: &Server,
) -> Result<bool, sqlx::Error> {
    if server.status == Some(ServerStatus::InstallFailed) {
        return Ok(true);
    }

    Ok(installing_since(database, server)
        .await?
        .is_some_and(|since| stuck(since, chrono::Utc::now())))
}

#[derive(Debug)]
pub enum RetryError {
    /// Not installing, or not for long enough yet.
    NotStuck,
    Failed(anyhow::Error),
}

impl From<sqlx::Error> for RetryError {
    fn from(err: sqlx::Error) -> Self {
        Self::Failed(err.into())
    }
}

/// Starts a stuck install over: asks Wings to drop whatever it still runs (a failure there is
/// fine, it may not know the server at all), then installs again as a reinstall does, keeping the
/// files. If Wings refuses that too, the server stays `installing`, so deleting it stays possible.
pub async fn retry(state: &crate::State, server: &Server) -> Result<(), RetryError> {
    let Some(since) = installing_since(&state.database, server).await? else {
        return Err(RetryError::NotStuck);
    };
    if !stuck(since, chrono::Utc::now()) {
        return Err(RetryError::NotStuck);
    }

    if let Ok(node) = server.node.fetch_cached(&state.database).await
        && let Ok(client) = node.api_client(&state.database).await
        && let Err(err) = client.post_servers_server_install_abort(server.uuid).await
    {
        tracing::debug!(server = %server.uuid, "no install to abort before the retry: {err:?}");
    }

    if !Server::try_set_status_by_uuid(
        state.database.write(),
        server.uuid,
        Some(ServerStatus::Installing),
        None,
    )
    .await?
    {
        return Err(RetryError::NotStuck);
    }
    Server::invalidate_cached(&state.database, server.uuid).await;

    if let Err(err) = server.install(state, false, None).await {
        Server::try_set_status_by_uuid(
            state.database.write(),
            server.uuid,
            None,
            Some(ServerStatus::Installing),
        )
        .await?;
        Server::invalidate_cached(&state.database, server.uuid).await;

        return Err(RetryError::Failed(err));
    }
    Server::invalidate_cached(&state.database, server.uuid).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{started, stuck};

    fn at(minute: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 25)
            .unwrap()
            .and_hms_opt(12, minute, 0)
            .unwrap()
    }

    #[test]
    fn an_install_starts_at_the_newest_of_creation_and_activity() {
        assert_eq!(started(at(0), None), at(0));
        assert_eq!(started(at(0), Some(at(20))), at(20));
        assert_eq!(started(at(20), Some(at(0))), at(20));
    }

    #[test]
    fn an_install_is_stuck_after_thirty_minutes() {
        assert!(!stuck(at(0).and_utc(), at(29).and_utc()));
        assert!(stuck(at(0).and_utc(), at(30).and_utc()));
    }
}
