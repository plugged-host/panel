//! plugged.host: deleted servers' system backups, kept for their owner a while (docs/features/backups-page.md).
//!
//! When a server is deleted, a delete handler records it in `deleted_servers` with its successful
//! system backups (they outlive the server: `server_backups.server_uuid` is set to null). The
//! owner's Backups page offers the newest of them for download until `hold_until`; the
//! `purge_deleted_servers` task removes the older ones right away and the newest one when the
//! hold ends. The account's live servers' newest backups are listed on the same page.

use crate::models::{
    ByUuid, DeletableModel, ListenerPriority, server::Server, server_backup::ServerBackup,
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// Hooks the hold into every server deletion (an owner's, an admin's, a suspension's purge).
pub fn register() {
    Server::register_delete_handler(
        ListenerPriority::Normal,
        |server, _options, state, transaction| Box::pin(hold(state, server, transaction)),
    );
}

/// Records the server and its system backups, in the deleting transaction: a deletion that fails
/// and is rolled back leaves no record.
async fn hold(
    state: &crate::State,
    server: &Server,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), anyhow::Error> {
    let days = state.settings.get().await?.recovery.deleted_hold_days;
    if days <= 0 {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO deleted_servers (uuid, owner_uuid, name, suspended, backups, hold_until)
        SELECT $1, $2, $3, $4, held.backups, NOW() + make_interval(days => $5)
        FROM (
            SELECT ARRAY(
                SELECT server_backups.uuid
                FROM server_backups
                WHERE server_backups.server_uuid = $1
                    AND server_backups.system_backup_policy_uuid IS NOT NULL
                    AND server_backups.kind = 'SERVER'
                    AND server_backups.successful
                    AND server_backups.deleted IS NULL
                    AND server_backups.deleting IS NULL
                ORDER BY server_backups.created DESC
            ) AS backups
        ) held
        WHERE cardinality(held.backups) > 0
        ON CONFLICT (uuid) DO NOTHING",
    )
    .bind(server.uuid)
    .bind(server.owner.uuid)
    .bind(server.name.as_str())
    .bind(server.suspended)
    .bind(days)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Whether the system backups of a server deleted now are held (and must not be deleted with it).
pub async fn holding(state: &crate::State) -> Result<bool, anyhow::Error> {
    Ok(state.settings.get().await?.recovery.deleted_hold_days > 0)
}

/// The held backups to remove now: all but the newest. `present` is newest first.
fn older_copies(present: &[uuid::Uuid]) -> &[uuid::Uuid] {
    present.get(1..).unwrap_or_default()
}

/// The held backups that still exist and are not being deleted, newest first.
async fn present(
    database: &crate::database::Database,
    backups: &[uuid::Uuid],
) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT server_backups.uuid
        FROM server_backups
        WHERE server_backups.uuid = ANY($1)
            AND server_backups.deleted IS NULL
            AND server_backups.deleting IS NULL
        ORDER BY server_backups.created DESC",
    )
    .bind(backups)
    .fetch_all(database.write())
    .await
}

/// Deletes the given backups; true when every one of them is gone or on its way.
async fn remove(state: &crate::State, server_uuid: uuid::Uuid, backups: &[uuid::Uuid]) -> bool {
    let mut all = true;

    for backup_uuid in backups {
        let backup = match ServerBackup::by_uuid_optional(&state.database, *backup_uuid).await {
            Ok(Some(backup)) => backup,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(server = %server_uuid, backup = %backup_uuid, "failed to load a held backup: {err:#}");
                all = false;
                continue;
            }
        };

        if let Err(err) = backup.delete(state, Default::default()).await {
            tracing::warn!(server = %server_uuid, backup = %backup_uuid, "failed to delete a held backup: {err:#}");
            all = false;
        }
    }

    all
}

/// Removes the older copies of newly held servers and everything of the holds that ended.
pub async fn purge(state: &crate::State) -> Result<(), anyhow::Error> {
    let rows = sqlx::query(
        "SELECT uuid, backups, trimmed IS NOT NULL AS trimmed, hold_until <= NOW() AS ended
        FROM deleted_servers
        WHERE purged IS NULL AND (trimmed IS NULL OR hold_until <= NOW())
        ORDER BY deleted
        LIMIT 100",
    )
    .fetch_all(state.database.write())
    .await?;

    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let backups: Vec<uuid::Uuid> = row.try_get("backups")?;
        let trimmed: bool = row.try_get("trimmed")?;
        let ended: bool = row.try_get("ended")?;

        let present = present(&state.database, &backups).await?;

        if ended {
            if remove(state, uuid, &present).await {
                sqlx::query(
                    "UPDATE deleted_servers SET trimmed = COALESCE(trimmed, NOW()), purged = NOW()
                    WHERE uuid = $1",
                )
                .bind(uuid)
                .execute(state.database.write())
                .await?;
                tracing::info!(server = %uuid, "removed the copies of a deleted server whose hold ended");
            }
        } else if !trimmed && remove(state, uuid, older_copies(&present)).await {
            sqlx::query("UPDATE deleted_servers SET trimmed = NOW() WHERE uuid = $1")
                .bind(uuid)
                .execute(state.database.write())
                .await?;
        }
    }

    Ok(())
}

#[derive(ToSchema, Serialize)]
pub struct ApiAccountBackupCopy {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub bytes: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}

/// One row of the account's Backups page: a server, live or deleted, and its newest copy.
#[derive(ToSchema, Serialize)]
pub struct ApiAccountBackup {
    pub server_uuid: uuid::Uuid,
    /// The short ID the panel shows (`#1a2b3c4d`) and its server URLs use (`/server/1a2b3c4d`).
    pub server_uuid_short: compact_str::CompactString,
    pub server_name: compact_str::CompactString,
    /// The server's egg; null for a deleted server (`deleted_servers` does not record it).
    pub egg_name: Option<compact_str::CompactString>,
    /// The server's node; for a deleted server, the node that took its copy.
    pub node_name: compact_str::CompactString,
    /// When the server was deleted; null for a server that still exists.
    pub deleted: Option<chrono::DateTime<chrono::Utc>>,
    /// Until when a deleted server's copy can be downloaded.
    pub kept_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Null while a live server has no finished backup yet.
    pub backup: Option<ApiAccountBackupCopy>,
}

fn copy_from(row: &sqlx::postgres::PgRow) -> Result<Option<ApiAccountBackupCopy>, sqlx::Error> {
    let Some(uuid) = row.try_get::<Option<uuid::Uuid>, _>("backup_uuid")? else {
        return Ok(None);
    };

    Ok(Some(ApiAccountBackupCopy {
        uuid,
        name: row.try_get("backup_name")?,
        bytes: row.try_get("backup_bytes")?,
        created: row
            .try_get::<chrono::NaiveDateTime, _>("backup_created")?
            .and_utc(),
    }))
}

/// A server's short ID as the panel writes it: `servers.uuid_short` in hex, which is the first
/// 8 hex digits of the server's UUID (`Server::create()`), so a deleted server has one too.
fn short_id(uuid_short: i32) -> compact_str::CompactString {
    compact_str::format_compact!("{uuid_short:08x}")
}

/// The newest finished, successful server backup among those the condition picks.
const NEWEST_COPY: &str =
    "SELECT server_backups.uuid AS backup_uuid, server_backups.name AS backup_name,
        server_backups.bytes AS backup_bytes, server_backups.created AS backup_created,
        server_backups.node_uuid AS backup_node_uuid
    FROM server_backups
    WHERE server_backups.kind = 'SERVER'
        AND server_backups.successful
        AND server_backups.completed IS NOT NULL
        AND server_backups.deleted IS NULL
        AND server_backups.deleting IS NULL";

/// The account's live servers (not suspended), newest first, and its deleted servers still held,
/// most recently deleted first, with their newest copy each.
pub async fn for_owner(
    database: &crate::database::Database,
    owner_uuid: uuid::Uuid,
) -> Result<Vec<ApiAccountBackup>, sqlx::Error> {
    let live = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT servers.uuid, servers.uuid_short, servers.name, nest_eggs.name AS egg_name,
            nodes.name AS node_name, copy.*
        FROM servers
        JOIN nest_eggs ON nest_eggs.uuid = servers.egg_uuid
        JOIN nodes ON nodes.uuid = servers.node_uuid
        LEFT JOIN LATERAL ({NEWEST_COPY}
            AND server_backups.server_uuid = servers.uuid
            ORDER BY server_backups.created DESC LIMIT 1) copy ON TRUE
        WHERE servers.owner_uuid = $1 AND NOT servers.suspended
        ORDER BY servers.created DESC"
    )))
    .bind(owner_uuid)
    .fetch_all(database.read())
    .await?;

    let deleted = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT deleted_servers.uuid, deleted_servers.name, deleted_servers.deleted,
            deleted_servers.hold_until, nodes.name AS node_name, copy.*
        FROM deleted_servers
        JOIN LATERAL ({NEWEST_COPY}
            AND server_backups.uuid = ANY(deleted_servers.backups)
            ORDER BY server_backups.created DESC LIMIT 1) copy ON TRUE
        JOIN nodes ON nodes.uuid = copy.backup_node_uuid
        WHERE deleted_servers.owner_uuid = $1
            AND NOT deleted_servers.suspended
            AND deleted_servers.purged IS NULL
            AND deleted_servers.hold_until > NOW()
        ORDER BY deleted_servers.deleted DESC"
    )))
    .bind(owner_uuid)
    .fetch_all(database.read())
    .await?;

    let mut rows = Vec::with_capacity(live.len() + deleted.len());
    for row in &live {
        rows.push(ApiAccountBackup {
            server_uuid: row.try_get("uuid")?,
            server_uuid_short: short_id(row.try_get("uuid_short")?),
            server_name: row.try_get("name")?,
            egg_name: Some(row.try_get("egg_name")?),
            node_name: row.try_get("node_name")?,
            deleted: None,
            kept_until: None,
            backup: copy_from(row)?,
        });
    }
    for row in &deleted {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        rows.push(ApiAccountBackup {
            server_uuid: uuid,
            server_uuid_short: short_id(uuid.as_fields().0 as i32),
            server_name: row.try_get("name")?,
            egg_name: None,
            node_name: row.try_get("node_name")?,
            deleted: Some(
                row.try_get::<chrono::NaiveDateTime, _>("deleted")?
                    .and_utc(),
            ),
            kept_until: Some(
                row.try_get::<chrono::NaiveDateTime, _>("hold_until")?
                    .and_utc(),
            ),
            backup: copy_from(row)?,
        });
    }

    Ok(rows)
}

/// A finished backup the user may download from the Backups page: one of a live server they own
/// (not suspended), or a held one of a server they deleted.
pub async fn owned_backup(
    database: &crate::database::Database,
    owner_uuid: uuid::Uuid,
    backup_uuid: uuid::Uuid,
) -> Result<Option<ServerBackup>, anyhow::Error> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM server_backups
            WHERE server_backups.uuid = $2
                AND server_backups.kind = 'SERVER'
                AND server_backups.successful
                AND server_backups.completed IS NOT NULL
                AND server_backups.deleted IS NULL
                AND server_backups.deleting IS NULL
                AND (
                    EXISTS (
                        SELECT 1 FROM servers
                        WHERE servers.uuid = server_backups.server_uuid
                            AND servers.owner_uuid = $1
                            AND NOT servers.suspended
                    )
                    OR EXISTS (
                        SELECT 1 FROM deleted_servers
                        WHERE deleted_servers.owner_uuid = $1
                            AND server_backups.uuid = ANY(deleted_servers.backups)
                            AND NOT deleted_servers.suspended
                            AND deleted_servers.purged IS NULL
                            AND deleted_servers.hold_until > NOW()
                    )
                )
        )",
    )
    .bind(owner_uuid)
    .bind(backup_uuid)
    .fetch_one(database.read())
    .await?;

    if !owned {
        return Ok(None);
    }

    Ok(ServerBackup::by_uuid_optional(database, backup_uuid).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_copy_is_kept() {
        let [a, b, c] = [
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        ];

        assert_eq!(older_copies(&[a, b, c]), &[b, c]);
        assert!(older_copies(&[a]).is_empty());
        assert!(older_copies(&[]).is_empty());
    }

    #[test]
    fn a_short_id_is_the_first_eight_hex_digits_of_the_uuid() {
        for uuid in [
            uuid::Uuid::parse_str("1a2b3c4d-0000-4000-8000-000000000000").unwrap(),
            // a negative uuid_short still reads as the UUID's start
            uuid::Uuid::parse_str("f00dcafe-0000-4000-8000-000000000000").unwrap(),
            uuid::Uuid::parse_str("00000001-0000-4000-8000-000000000000").unwrap(),
        ] {
            assert_eq!(
                short_id(uuid.as_fields().0 as i32).as_str(),
                &uuid.to_string()[..8]
            );
        }
    }
}
