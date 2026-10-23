//! plugged.host: why, since when, by whom and until when a server is suspended, next to the
//! native `servers.suspended` (docs/features/suspension.md).

use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use utoipa::ToSchema;

/// What suspended a server.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuspensionKind {
    /// An admin, from the server's page or the server list.
    Manual,
    /// The abuse checks.
    Abuse,
    /// noprofit.space (the account was banned or deleted there).
    Noprofit,
}

impl SuspensionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Abuse => "abuse",
            Self::Noprofit => "noprofit",
        }
    }

    /// Unknown or missing values read as `Manual`: a server suspended before these columns existed,
    /// or through a path that does not say why, was suspended by an admin.
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("abuse") => Self::Abuse,
            Some("noprofit") => Self::Noprofit,
            _ => Self::Manual,
        }
    }
}

/// The details of a suspended server. `None` on a server that is not suspended.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServerSuspension {
    pub kind: SuspensionKind,
    pub reason: Option<compact_str::CompactString>,
    pub suspended: Option<chrono::NaiveDateTime>,
    pub suspended_by: Option<uuid::Uuid>,
    pub delete_at: Option<chrono::NaiveDateTime>,
}

/// The columns, for `Server::base_columns`.
pub fn base_columns(prefix: &str) -> [(&'static str, compact_str::CompactString); 5] {
    [
        (
            "servers.suspension_kind",
            compact_str::format_compact!("{prefix}suspension_kind"),
        ),
        (
            "servers.suspension_reason",
            compact_str::format_compact!("{prefix}suspension_reason"),
        ),
        (
            "servers.suspended_at",
            compact_str::format_compact!("{prefix}suspended_at"),
        ),
        (
            "servers.suspended_by",
            compact_str::format_compact!("{prefix}suspended_by"),
        ),
        (
            "servers.suspension_delete_at",
            compact_str::format_compact!("{prefix}suspension_delete_at"),
        ),
    ]
}

impl ServerSuspension {
    pub fn map(prefix: &str, row: &PgRow) -> Result<Option<Self>, sqlx::Error> {
        let suspended: bool = row.try_get(format!("{prefix}suspended").as_str())?;
        if !suspended {
            return Ok(None);
        }

        let kind: Option<String> = row.try_get(format!("{prefix}suspension_kind").as_str())?;

        Ok(Some(Self {
            kind: SuspensionKind::parse(kind.as_deref()),
            reason: row.try_get(format!("{prefix}suspension_reason").as_str())?,
            suspended: row.try_get(format!("{prefix}suspended_at").as_str())?,
            suspended_by: row.try_get(format!("{prefix}suspended_by").as_str())?,
            delete_at: row.try_get(format!("{prefix}suspension_delete_at").as_str())?,
        }))
    }

    pub fn into_api_object(self) -> ApiServerSuspension {
        ApiServerSuspension {
            kind: self.kind,
            reason: self.reason,
            suspended: self.suspended.map(|at| at.and_utc()),
            delete_at: self.delete_at.map(|at| at.and_utc()),
        }
    }

    pub fn into_admin_api_object(self) -> AdminApiServerSuspension {
        AdminApiServerSuspension {
            suspended_by: self.suspended_by,
            kind: self.kind,
            reason: self.reason,
            suspended: self.suspended.map(|at| at.and_utc()),
            delete_at: self.delete_at.map(|at| at.and_utc()),
        }
    }
}

/// What the server's owner and subusers see.
#[derive(ToSchema, Serialize, Clone, Debug)]
#[schema(title = "ServerSuspension")]
pub struct ApiServerSuspension {
    pub kind: SuspensionKind,
    pub reason: Option<compact_str::CompactString>,
    pub suspended: Option<chrono::DateTime<chrono::Utc>>,
    /// When the purge deletes the server, unless it is unsuspended first.
    pub delete_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
#[schema(title = "AdminServerSuspension")]
pub struct AdminApiServerSuspension {
    pub kind: SuspensionKind,
    pub reason: Option<compact_str::CompactString>,
    pub suspended: Option<chrono::DateTime<chrono::Utc>>,
    pub suspended_by: Option<uuid::Uuid>,
    pub delete_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A suspension made by the panel itself (the abuse checks, noprofit.space), not through an update.
#[derive(Clone, Debug)]
pub struct Suspend {
    pub kind: SuspensionKind,
    pub reason: Option<String>,
    pub by: Option<uuid::Uuid>,
    pub delete_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Sets the suspension columns of an update (`Server::update`):
///
/// - suspending: the kind (`Manual` unless the caller said), now, by whom, the reason and the
///   deletion time;
/// - unsuspending: all of them cleared;
/// - a server that stays suspended: the reason and deletion time, when the update carries them.
pub fn apply_update<'a>(
    query_builder: &mut crate::models::UpdateQueryBuilder<'a>,
    was_suspended: bool,
    options: &'a super::UpdateServerOptions,
) {
    let suspended = options.suspended.unwrap_or(was_suspended);
    let reason = options
        .suspension_reason
        .as_ref()
        .map(|reason| reason.as_deref());
    let delete_at = options
        .suspension_delete_at
        .map(|delete_at| delete_at.map(|delete_at| delete_at.naive_utc()));

    if suspended && !was_suspended {
        query_builder
            .set(
                "suspension_kind",
                Some(
                    options
                        .suspension_kind
                        .unwrap_or(SuspensionKind::Manual)
                        .as_str(),
                ),
            )
            .set("suspended_at", Some(chrono::Utc::now().naive_utc()))
            .set("suspended_by", Some(options.suspended_by))
            .set("suspension_reason", Some(reason.flatten()))
            .set("suspension_delete_at", Some(delete_at.flatten()));
    } else if !suspended && was_suspended {
        query_builder
            .set("suspension_kind", Some(None::<&str>))
            .set("suspended_at", Some(None::<chrono::NaiveDateTime>))
            .set("suspended_by", Some(None::<uuid::Uuid>))
            .set("suspension_reason", Some(None::<&str>))
            .set("suspension_delete_at", Some(None::<chrono::NaiveDateTime>));
    } else if suspended {
        query_builder
            .set("suspension_reason", reason)
            .set("suspension_delete_at", delete_at);
    }
}

/// The suspension a server has after an update, for `Server::update` to keep the model current.
pub fn after_update(
    current: Option<ServerSuspension>,
    was_suspended: bool,
    options: &super::UpdateServerOptions,
) -> Option<ServerSuspension> {
    let suspended = options.suspended.unwrap_or(was_suspended);

    if !suspended {
        return None;
    }

    if !was_suspended {
        return Some(ServerSuspension {
            kind: options.suspension_kind.unwrap_or(SuspensionKind::Manual),
            reason: options.suspension_reason.clone().flatten(),
            suspended: Some(chrono::Utc::now().naive_utc()),
            suspended_by: options.suspended_by,
            delete_at: options
                .suspension_delete_at
                .flatten()
                .map(|delete_at| delete_at.naive_utc()),
        });
    }

    let mut suspension = current.unwrap_or(ServerSuspension {
        kind: SuspensionKind::Manual,
        reason: None,
        suspended: None,
        suspended_by: None,
        delete_at: None,
    });
    if let Some(reason) = &options.suspension_reason {
        suspension.reason = reason.clone();
    }
    if let Some(delete_at) = options.suspension_delete_at {
        suspension.delete_at = delete_at.map(|delete_at| delete_at.naive_utc());
    }

    Some(suspension)
}

impl super::Server {
    /// Suspends the server for the panel itself (the abuse checks, noprofit.space) and tells its
    /// node. A server that is already suspended keeps its suspension.
    pub async fn suspend(
        &mut self,
        state: &crate::State,
        suspend: Suspend,
    ) -> Result<(), anyhow::Error> {
        if self.suspended {
            return Ok(());
        }

        let delete_at = suspend.delete_at.map(|delete_at| delete_at.naive_utc());
        let suspended_at = sqlx::query_scalar::<_, chrono::NaiveDateTime>(
            r#"
            UPDATE servers
            SET suspended = true, suspension_kind = $2, suspension_reason = $3,
                suspended_at = NOW(), suspended_by = $4, suspension_delete_at = $5
            WHERE uuid = $1
            RETURNING suspended_at
            "#,
        )
        .bind(self.uuid)
        .bind(suspend.kind.as_str())
        .bind(suspend.reason.as_deref())
        .bind(suspend.by)
        .bind(delete_at)
        .fetch_one(state.database.write())
        .await?;

        self.suspended = true;
        self.suspension = Some(ServerSuspension {
            kind: suspend.kind,
            reason: suspend.reason.map(Into::into),
            suspended: Some(suspended_at),
            suspended_by: suspend.by,
            delete_at,
        });

        self.clone().sync(state).await?;

        Ok(())
    }
}

/// Servers whose deletion time passed, deleted per run at most.
const PURGE_BATCH: i64 = 20;

/// Deletes suspended servers whose `suspension_delete_at` has passed (the `purge_suspended_servers`
/// task). Their backups stay. A server whose node cannot delete it is tried again next run.
pub async fn purge_due(state: &crate::State) -> Result<usize, anyhow::Error> {
    use crate::models::{ByUuid, DeletableModel};

    let due: Vec<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT uuid FROM servers
        WHERE suspended AND suspension_delete_at IS NOT NULL AND suspension_delete_at <= NOW()
        ORDER BY suspension_delete_at
        LIMIT $1
        "#,
    )
    .bind(PURGE_BATCH)
    .fetch_all(state.database.read())
    .await?;

    let mut deleted = 0;
    for server_uuid in due {
        let server = match super::Server::by_uuid_optional(&state.database, server_uuid).await? {
            Some(server) if server.suspended => server,
            _ => continue,
        };

        match server
            .delete(state, super::DeleteServerOptions { force: false })
            .await
        {
            Ok(()) => {
                tracing::info!(server = %server_uuid, name = %server.name, "deleted a suspended server whose deletion time passed");
                deleted += 1;

                if server
                    .suspension
                    .as_ref()
                    .is_some_and(|suspension| suspension.kind == SuspensionKind::Abuse)
                {
                    crate::abuse::alert_deleted(state, &server).await;
                }
            }
            Err(err) => {
                tracing::warn!(server = %server_uuid, "failed to delete a suspended server: {err:#}");
            }
        }
    }

    Ok(deleted)
}

/// Suspends every server a user owns for noprofit.space (the account is gone there).
pub async fn suspend_owned_for_noprofit(
    state: &crate::State,
    owner_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    let mut page = 1;
    loop {
        let servers = super::Server::by_owner_uuid_with_pagination(
            &state.database,
            owner_uuid,
            page,
            50,
            None,
        )
        .await?;
        if servers.data.is_empty() {
            break;
        }

        for mut server in servers.data {
            let server_uuid = server.uuid;
            if let Err(err) = server
                .suspend(
                    state,
                    Suspend {
                        kind: SuspensionKind::Noprofit,
                        reason: None,
                        by: None,
                        delete_at: None,
                    },
                )
                .await
            {
                tracing::warn!(server = %server_uuid, "failed to suspend a server of a deleted noprofit.space account: {err:#}");
            }
        }

        page += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for kind in [
            SuspensionKind::Manual,
            SuspensionKind::Abuse,
            SuspensionKind::Noprofit,
        ] {
            assert_eq!(SuspensionKind::parse(Some(kind.as_str())), kind);
        }
    }

    #[test]
    fn unknown_kind_reads_as_manual() {
        assert_eq!(SuspensionKind::parse(None), SuspensionKind::Manual);
        assert_eq!(SuspensionKind::parse(Some("other")), SuspensionKind::Manual);
    }
}
