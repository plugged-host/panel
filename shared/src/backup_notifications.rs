//! plugged.host: the backup mails a person asks for on a server's Settings › Notifications, like
//! the Pelican panel's two switches: one for backups made by hand, one for a schedule's
//! (docs/features/server-settings.md).
//!
//! The choice is per person and server (`server_notification_settings`; a person with both off has
//! no row). When a backup of the server finishes, its owner and the subusers who may read its
//! backups get the `backup_completed` mail if they asked for that kind. System backups (Admin ›
//! System Backup Policies) send none.

use crate::models::{
    ByUuid, EventEmittingModel,
    server::Server,
    server_backup::{ServerBackup, ServerBackupEvent, ServerBackupKind},
    user::User,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

/// The mail template a finished backup sends.
pub const TEMPLATE: &str = "backup_completed";

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    /// Backups someone made on the Backups page.
    pub manual_backups: bool,
    /// Backups a schedule made.
    pub scheduled_backups: bool,
}

impl Preferences {
    /// Whether a finished backup gets a mail: a schedule's or one made by hand.
    pub fn wants(self, scheduled: bool) -> bool {
        if scheduled {
            self.scheduled_backups
        } else {
            self.manual_backups
        }
    }
}

/// Postgres' `undefined_table`: the panel runs before the table's migration did. Reads then answer
/// "nothing asked for", so the settings page and finished backups keep working.
fn missing_table(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(|err| err.code())
        .is_some_and(|code| code == "42P01")
}

/// What `user_uuid` asked for on `server_uuid`.
pub async fn get(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
) -> Result<Preferences, sqlx::Error> {
    let row = match sqlx::query(
        "SELECT manual_backups, scheduled_backups FROM server_notification_settings
        WHERE server_uuid = $1 AND user_uuid = $2",
    )
    .bind(server_uuid)
    .bind(user_uuid)
    .fetch_optional(database.read())
    .await
    {
        Ok(row) => row,
        Err(err) if missing_table(&err) => return Ok(Preferences::default()),
        Err(err) => return Err(err),
    };

    Ok(match row {
        Some(row) => Preferences {
            manual_backups: row.try_get("manual_backups")?,
            scheduled_backups: row.try_get("scheduled_backups")?,
        },
        None => Preferences::default(),
    })
}

/// Stores what `user_uuid` asked for on `server_uuid`; both off removes the row.
pub async fn set(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
    preferences: Preferences,
) -> Result<(), sqlx::Error> {
    if preferences == Preferences::default() {
        sqlx::query(
            "DELETE FROM server_notification_settings WHERE server_uuid = $1 AND user_uuid = $2",
        )
        .bind(server_uuid)
        .bind(user_uuid)
        .execute(database.write())
        .await?;

        return Ok(());
    }

    sqlx::query(
        "INSERT INTO server_notification_settings (server_uuid, user_uuid, manual_backups, scheduled_backups)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (server_uuid, user_uuid) DO UPDATE
            SET manual_backups = EXCLUDED.manual_backups,
                scheduled_backups = EXCLUDED.scheduled_backups",
    )
    .bind(server_uuid)
    .bind(user_uuid)
    .bind(preferences.manual_backups)
    .bind(preferences.scheduled_backups)
    .execute(database.write())
    .await?;

    Ok(())
}

/// Sends the mails when a backup finishes, made by hand or by a schedule.
pub fn register() {
    let _ = ServerBackup::register_event_handler(|state, event| async move {
        if let ServerBackupEvent::CreationCompleted { backup, successful } = &*event {
            notify(&state, backup, *successful).await?;
        }

        Ok(())
    });
}

async fn notify(
    state: &crate::State,
    backup: &ServerBackup,
    successful: bool,
) -> Result<(), anyhow::Error> {
    if backup.kind != ServerBackupKind::Server || backup.system_backup_policy_uuid.is_some() {
        return Ok(());
    }
    let Some(server_uuid) = backup.server.as_ref().map(|server| server.uuid) else {
        return Ok(());
    };

    // a schedule's backup is the one whose creation the schedule logged
    let scheduled: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM server_activities
            WHERE server_uuid = $1 AND event = 'server:backup.create'
                AND data->>'uuid' = $2 AND schedule_uuid IS NOT NULL
        )",
    )
    .bind(server_uuid)
    .bind(backup.uuid.to_string())
    .fetch_one(state.database.read())
    .await?;

    // the owner, and subusers who may still read the server's backups
    let rows = match sqlx::query(
        "SELECT settings.user_uuid, settings.manual_backups, settings.scheduled_backups
        FROM server_notification_settings settings
        JOIN servers ON servers.uuid = settings.server_uuid
        LEFT JOIN server_subusers ON server_subusers.server_uuid = settings.server_uuid
            AND server_subusers.user_uuid = settings.user_uuid
        WHERE settings.server_uuid = $1
            AND (servers.owner_uuid = settings.user_uuid
                OR 'backups.read' = ANY(server_subusers.permissions))",
    )
    .bind(server_uuid)
    .fetch_all(state.database.read())
    .await
    {
        Ok(rows) => rows,
        Err(err) if missing_table(&err) => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let mut recipients = Vec::new();
    for row in rows {
        let preferences = Preferences {
            manual_backups: row.try_get("manual_backups")?,
            scheduled_backups: row.try_get("scheduled_backups")?,
        };
        if preferences.wants(scheduled) {
            recipients.push(row.try_get::<uuid::Uuid, _>("user_uuid")?);
        }
    }
    if recipients.is_empty() {
        return Ok(());
    }

    let Some(server) = Server::by_uuid_optional(&state.database, server_uuid).await? else {
        return Ok(());
    };
    let backups_link = {
        let settings = state.settings.get().await?;
        format!(
            "{}/server/{:08x}/backups",
            settings.app.url, server.uuid_short
        )
    };

    for user_uuid in recipients {
        let Some(user) = User::by_uuid_optional(&state.database, user_uuid).await? else {
            continue;
        };

        state
            .mail
            .send_template(
                state,
                TEMPLATE,
                &user,
                minijinja::context! {
                    user => minijinja::value::Serde(&user),
                    server => minijinja::value::Serde(&server),
                    backup_name => backup.name.as_str(),
                    successful => successful,
                    backups_link => backups_link.as_str(),
                },
            )
            .await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_switch_covers_its_own_kind_of_backup() {
        let manual = Preferences {
            manual_backups: true,
            scheduled_backups: false,
        };
        assert!(manual.wants(false));
        assert!(!manual.wants(true));

        let scheduled = Preferences {
            manual_backups: false,
            scheduled_backups: true,
        };
        assert!(scheduled.wants(true));
        assert!(!scheduled.wants(false));

        assert!(!Preferences::default().wants(false));
        assert!(!Preferences::default().wants(true));
    }
}
