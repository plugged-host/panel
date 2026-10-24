//! plugged.host: devbox abuse blocks (docs/features/devboxes.md). A critical finding on a devbox
//! takes devboxes away from whoever pays for it: their running ones stop, and they get none until
//! the finding is dismissed or resolved. Kept apart from the finding, which goes with its server
//! (the abuse purge deletes suspended servers), while the block must not.

use super::{Context, Devbox};
use crate::models::{ByUuid, server::Server};
use sqlx::Row;

const LIVE: &str = "abuse_findings.severity = 'critical'
    AND abuse_findings.status IN ('open', 'actioned')";

/// Whether `user_uuid` may not have devboxes: a block not lifted yet, or a live critical finding
/// on a devbox they pay for that no block was made for (recorded while devboxes were off).
pub async fn blocked(state: &crate::State, user_uuid: uuid::Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (
            SELECT 1 FROM devbox_blocks WHERE user_uuid = $1 AND lifted IS NULL
        ) OR EXISTS (
            SELECT 1 FROM abuse_findings
            JOIN devboxes ON devboxes.server_uuid = abuse_findings.server_uuid
            WHERE {LIVE} AND (devboxes.user_uuid = $1 OR (devboxes.user_uuid IS NULL
                AND devboxes.game_server_uuid IN (SELECT uuid FROM servers WHERE owner_uuid = $1)))
        )"
    )))
    .bind(user_uuid)
    .fetch_one(state.database.read())
    .await
}

/// Whether a live critical finding holds the devbox's server: it is kept (stopped) until an admin
/// dismisses or resolves it, since the finding and its evidence would go with the server.
pub async fn under_review(
    database: &crate::database::Database,
    devbox: &Devbox,
) -> Result<bool, sqlx::Error> {
    let Some(server_uuid) = devbox.server_uuid else {
        return Ok(false);
    };

    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM abuse_findings WHERE server_uuid = $1 AND {LIVE})"
    )))
    .bind(server_uuid)
    .fetch_one(database.read())
    .await
}

/// The person a devbox server's findings are about: its person, or the game server's owner for a
/// shared one. `None` for a server that is no devbox.
pub async fn person_of(
    state: &crate::State,
    server_uuid: uuid::Uuid,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let Some(devbox) = Devbox::by_server_uuid(&state.database, server_uuid).await? else {
        return Ok(None);
    };
    if let Some(user_uuid) = devbox.user_uuid {
        return Ok(Some(user_uuid));
    }

    Ok(
        Server::by_uuid_optional(&state.database, devbox.game_server_uuid)
            .await?
            .map(|game| game.owner.uuid),
    )
}

/// After the abuse checks recorded findings on a server: when it is a devbox with a live critical
/// finding, its person is blocked, every devbox they pay for stops, and shared devboxes of others
/// they are in start over (a new token, nothing of theirs left running there).
pub async fn on_findings(state: &crate::State, server_uuid: uuid::Uuid) {
    if let Err(err) = apply_findings(state, server_uuid).await {
        tracing::warn!(server = %server_uuid, "failed to block devboxes for an abuse finding: {err:#}");
    }
}

async fn apply_findings(
    state: &crate::State,
    server_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    let Some(person) = person_of(state, server_uuid).await? else {
        return Ok(());
    };
    let findings = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT uuid, rule FROM abuse_findings WHERE server_uuid = $1 AND {LIVE}"
    )))
    .bind(server_uuid)
    .fetch_all(state.database.read())
    .await?;
    if findings.is_empty() {
        return Ok(());
    }

    let mut added = 0;
    for finding in &findings {
        // a new block, or one lifted before whose finding was opened again
        let inserted: Option<bool> = sqlx::query_scalar(
            "INSERT INTO devbox_blocks (user_uuid, finding_uuid, server_uuid, rule)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_uuid, finding_uuid) DO UPDATE SET lifted = NULL
                WHERE devbox_blocks.lifted IS NOT NULL
            RETURNING true",
        )
        .bind(person)
        .bind(finding.try_get::<uuid::Uuid, _>("uuid")?)
        .bind(server_uuid)
        .bind(finding.try_get::<String, _>("rule")?)
        .fetch_optional(state.database.write())
        .await?;
        added += i32::from(inserted.is_some());
    }

    // every save of a live finding lands here; after the first only the stops are retried
    for devbox in Devbox::running_paid_by(&state.database, person).await? {
        if devbox.stopping.is_none()
            && let Err(err) = super::power::stop(state, &devbox).await
        {
            tracing::warn!(devbox = %devbox.uuid, "failed to stop a blocked devbox: {}", err.message());
        }
    }
    if added == 0 {
        return Ok(());
    }
    tracing::warn!(user = %person, server = %server_uuid, "devboxes taken away for an abuse finding");

    let context = Context::load(state).await?;
    let shared: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT devboxes.uuid FROM devboxes
        JOIN server_subusers ON server_subusers.server_uuid = devboxes.game_server_uuid
        WHERE server_subusers.user_uuid = $1 AND devboxes.user_uuid IS NULL
            AND devboxes.token_hash IS NOT NULL AND devboxes.stopping IS NULL",
    )
    .bind(person)
    .fetch_all(state.database.read())
    .await?;
    for uuid in shared {
        if let Some(devbox) = Devbox::by_uuid(&state.database, uuid).await?
            && let Err(err) = super::power::restart(state, &context, &devbox).await
        {
            tracing::warn!(devbox = %devbox.uuid, "failed to restart a shared devbox: {}", err.message());
        }
    }

    Ok(())
}

/// After an admin closed or reopened a finding: dismissed (a false positive) or resolved lifts
/// its blocks; reopened blocks again whoever it blocked.
pub async fn on_finding_status(
    database: &crate::database::Database,
    finding_uuid: uuid::Uuid,
    status: &str,
) {
    let live = matches!(
        status,
        crate::abuse::STATUS_OPEN | crate::abuse::STATUS_ACTIONED
    );
    let result = sqlx::query(if live {
        "UPDATE devbox_blocks SET lifted = NULL WHERE finding_uuid = $1 AND lifted IS NOT NULL"
    } else {
        "UPDATE devbox_blocks SET lifted = NOW() WHERE finding_uuid = $1 AND lifted IS NULL"
    })
    .bind(finding_uuid)
    .execute(database.write())
    .await;

    if let Err(err) = result {
        tracing::warn!(finding = %finding_uuid, "failed to update devbox blocks: {err:#}");
    }
}
