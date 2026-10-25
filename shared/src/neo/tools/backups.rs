//! plugged.host: neo's tools for a server's backups (docs/features/neo.md › Tools). Server-file
//! backups only, with the checks of the backup routes.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, support};
use crate::models::{
    ByUuid, CreatableModel, DeletableModel, UpdatableModel,
    server::{Server, ServerStatus},
    server_backup::{
        CreateServerBackupOptions, ServerBackup, ServerBackupDeletionStatus, ServerBackupFilter,
        ServerBackupKind, ServerBackupRestoreOptions, UpdateServerBackupOptions,
        retention::EvictionMode,
    },
};
use serde_json::{Value, json};
use std::borrow::Cow;

const LISTED: i64 = 50;

/// A refusal the model can act on when a model call says why it would not run (a
/// `DisplayError`, a validation error); anything else is a failure.
fn refusal(err: impl Into<anyhow::Error>) -> ToolError {
    let err = err.into();

    match crate::response::extract_readable_message(&err) {
        Some((message, _)) => ToolError::invalid(format!("The panel refused this: {message}.")),
        None => ToolError::Failed(err),
    }
}

/// The server's backup the model names, never a deleted one or a database backup.
async fn find(context: &ToolContext<'_>, arguments: &Value) -> Result<ServerBackup, ToolError> {
    let server = context.server()?;
    let missing = || ToolError::invalid("No such backup on this server; use list_backups.");
    let uuid = support::string(arguments, "backup", 64, true)?
        .trim()
        .parse::<uuid::Uuid>()
        .map_err(|_| missing())?;

    match ServerBackup::by_server_uuid_uuid(&context.state.database, server.uuid, uuid).await? {
        Some(backup) if backup.deleted.is_none() && backup.kind == ServerBackupKind::Server => {
            Ok(backup)
        }
        _ => Err(missing()),
    }
}

/// A backup's state in a word.
fn backup_state(backup: &ServerBackup) -> &'static str {
    if backup.deleting.is_some() {
        "being deleted"
    } else if backup.completed.is_none() {
        "in progress"
    } else if backup.successful {
        "ok"
    } else {
        "failed"
    }
}

pub struct ListBackupsTool;

#[async_trait::async_trait]
impl Tool for ListBackupsTool {
    fn name(&self) -> &'static str {
        "list_backups"
    }

    fn description(&self) -> Cow<'static, str> {
        "List this server's backups (id, name, size, date, state, locked) and how many more can be made.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["backups.read"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "List backups".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let backups = ServerBackup::by_server_uuid_node_uuid_with_pagination(
            &context.state.database,
            server.uuid,
            server.node.uuid,
            1,
            LISTED,
            None,
            &ServerBackupFilter {
                kind: Some(ServerBackupKind::Server),
                database_instance_uuid: None,
                database_type: None,
            },
        )
        .await?;
        let used = ServerBackup::count_by_server_uuid(&context.state.database, server.uuid).await?;

        let mut lines = vec![format!(
            "limit: {} backups, {used} used (system backups do not count)",
            server.backup_limit
        )];
        let backups: Vec<&ServerBackup> = backups
            .data
            .iter()
            .filter(|backup| backup.deleted.is_none())
            .collect();
        for backup in &backups {
            lines.push(format!(
                "- id {}: \"{}\", {}, {}, {}{}{}",
                backup.uuid,
                backup.name,
                support::human(backup.bytes.max(0) as u64),
                backup.created.format("%Y-%m-%d %H:%M"),
                backup_state(backup),
                if backup.locked { ", locked" } else { "" },
                if backup.system_backup_policy_uuid.is_some() {
                    ", system (cannot be deleted)"
                } else {
                    ""
                }
            ));
        }
        if backups.is_empty() {
            lines.push("No backups yet.".into());
        }

        Ok(ToolResult::ok(support::limit_output(&lines.join("\n")))
            .with_display(format!("{} backups", backups.len())))
    }
}

pub struct CreateBackupTool;

#[async_trait::async_trait]
impl Tool for CreateBackupTool {
    fn name(&self) -> &'static str {
        "create_backup"
    }

    fn description(&self) -> Cow<'static, str> {
        "Start a backup of the server's files (runs in the background). Good before risky changes. When the backup limit is reached, old backups the server's retention allows are removed first; if none may go, an old backup must be deleted first.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "maxLength": 255},
                "locked": {"type": "boolean", "description": "Protect it from deletion (needs the backup update permission)."},
            },
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["backups.create"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        match arguments.get("name").and_then(Value::as_str) {
            Some(name) if !name.trim().is_empty() => {
                format!("Create backup \"{}\"", support::short(name, 60))
            }
            _ => "Create backup".into(),
        }
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let name = support::string(arguments, "name", 255, false)?.trim();
        let name = if name.is_empty() {
            ServerBackup::default_name()
        } else {
            name.into()
        };
        let locked = support::boolean(arguments, "locked", false) && context.can("backups.update");

        let ratelimit = context
            .state
            .settings
            .get_as(|settings| settings.ratelimits.client_servers_backups_create)
            .await?;
        if context
            .state
            .cache
            .ratelimit(
                "client/servers/backups/create",
                ratelimit.hits,
                ratelimit.window_seconds,
                server.uuid.to_string(),
            )
            .await
            .is_err()
        {
            return Err(ToolError::invalid(
                "Backups were started too recently; wait a few minutes and try again.",
            ));
        }

        let lock = context
            .state
            .cache
            .lock(
                format!("servers::{}::backups", server.uuid),
                Some(30),
                Some(5000),
            )
            .await?;

        let limit = server.backup_limit as i64;
        let count =
            ServerBackup::count_by_server_uuid(&context.state.database, server.uuid).await?;
        if count >= limit {
            let outcome = ServerBackup::evict_for_create(
                context.state,
                server.uuid,
                ServerBackupKind::Server,
                EvictionMode::Expendable,
            )
            .await?;

            if count - outcome.evicted as i64 >= limit {
                return Err(ToolError::invalid(if limit <= 0 {
                    "This server cannot have backups.".to_string()
                } else if outcome.failed > 0 {
                    "Old backups could not be deleted to make room; try again later.".to_string()
                } else if outcome.maintenance > 0 {
                    "Old backups cannot be deleted right now (their storage is in maintenance)."
                        .to_string()
                } else {
                    format!(
                        "The backup limit ({limit}) is reached; delete an old backup first (list_backups)."
                    )
                }));
            }
        }

        let metadata = ServerBackup::generate_metadata(context.state, server).await?;
        let mut backup = ServerBackup::create(
            context.state,
            CreateServerBackupOptions {
                server,
                name,
                backup_group_uuid: None,
                system_backup_policy_uuid: None,
                database_instance: None,
                backup_configuration: None,
                ignored_files: Vec::new(),
                metadata,
            },
        )
        .await
        .map_err(refusal)?;
        drop(lock);

        if locked {
            backup
                .update(
                    context.state,
                    UpdateServerBackupOptions {
                        locked: Some(true),
                        ..Default::default()
                    },
                )
                .await?;
        }

        context
            .log(
                "backup.create",
                json!({"uuid": backup.uuid, "name": backup.name, "locked": locked}),
            )
            .await;

        Ok(ToolResult::ok(format!(
            "Backup \"{}\" (id {}) started{}; it finishes in the background.",
            backup.name,
            backup.uuid,
            if locked { ", locked" } else { "" }
        ))
        .with_display("Backup started"))
    }
}

/// Restores a server backup over the server's files, as the restore route does. `false` when the
/// server is busy with something else.
async fn restore(
    state: crate::State,
    mut server: Server,
    backup: ServerBackup,
    truncate: bool,
) -> Result<bool, anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    if !server
        .try_set_status(&mut *transaction, None, Some(ServerStatus::RestoringBackup))
        .await?
    {
        transaction.rollback().await?;
        return Ok(false);
    }

    let server_uuid = server.uuid;
    if let Err(err) = backup
        .restore(
            &state,
            &mut transaction,
            server,
            ServerBackupRestoreOptions {
                truncate_directory: truncate,
                restore_startup: false,
            },
        )
        .await
    {
        transaction.rollback().await?;
        return Err(err);
    }
    transaction.commit().await?;
    Server::invalidate_cached(&state.database, server_uuid).await;

    Ok(true)
}

pub struct RestoreBackupTool;

#[async_trait::async_trait]
impl Tool for RestoreBackupTool {
    fn name(&self) -> &'static str {
        "restore_backup"
    }

    fn description(&self) -> Cow<'static, str> {
        "Restore a backup (id from list_backups) over the server's files. With delete_existing, every current file is removed first. The user always approves this.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "backup": {"type": "string", "description": "Backup id."},
                "delete_existing": {"type": "boolean", "description": "Delete all current files before restoring."},
            },
            "required": ["backup"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Critical
    }

    fn offered_with(&self) -> Vec<&'static str> {
        vec!["backups.restore"]
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["backups.read", "backups.restore"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Restore backup {}{}",
            support::short(support::shown(arguments, "backup"), 40),
            if support::boolean(arguments, "delete_existing", false) {
                " (deleting all current files first)"
            } else {
                ""
            }
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let backup = find(context, arguments).await?;
        let server = context.server()?.clone();
        if backup.deleting.is_some() {
            return Err(ToolError::invalid("That backup is being deleted."));
        }
        if backup.completed.is_none() || !backup.successful {
            return Err(ToolError::invalid(
                "That backup is not complete or failed, so it cannot be restored.",
            ));
        }
        if server.status.is_some() {
            return Err(ToolError::invalid(
                "The server is busy (installing or restoring) and cannot restore right now.",
            ));
        }

        let truncate = support::boolean(arguments, "delete_existing", false);
        let name = backup.name.clone();
        let uuid = backup.uuid;

        // spawned like the route: a restore must not be cut off halfway when the chat goes away
        let started = tokio::spawn(restore(context.state.clone(), server, backup, truncate))
            .await
            .map_err(anyhow::Error::from)?
            .map_err(refusal)?;

        if !started {
            return Err(ToolError::invalid(
                "The server is not in a state to restore a backup right now.",
            ));
        }

        context
            .log(
                "backup.restore",
                json!({"uuid": uuid, "name": name, "truncate_directory": truncate}),
            )
            .await;

        Ok(ToolResult::ok(format!(
            "Restoring \"{name}\" now{}. The server is unavailable until it finishes.",
            if truncate {
                " (existing files deleted first)"
            } else {
                ""
            }
        ))
        .with_display("Restoring backup"))
    }
}

pub struct DeleteBackupTool;

#[async_trait::async_trait]
impl Tool for DeleteBackupTool {
    fn name(&self) -> &'static str {
        "delete_backup"
    }

    fn description(&self) -> Cow<'static, str> {
        "Delete a backup (id from list_backups). Locked and system backups cannot be deleted."
            .into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"backup": {"type": "string"}},
            "required": ["backup"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn offered_with(&self) -> Vec<&'static str> {
        vec!["backups.delete"]
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["backups.read", "backups.delete"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Delete backup {}",
            support::short(support::shown(arguments, "backup"), 40)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let backup = find(context, arguments).await?;
        if backup.system_backup_policy_uuid.is_some() {
            return Err(ToolError::invalid("System backups cannot be deleted."));
        }
        if backup.completed.is_none() {
            return Err(ToolError::invalid("That backup has not finished yet."));
        }
        if backup.locked {
            return Err(ToolError::invalid(
                "That backup is locked; unlock it in the panel first.",
            ));
        }
        if matches!(
            backup.deletion_status(),
            Some(ServerBackupDeletionStatus::Deleting)
        ) {
            return Err(ToolError::invalid("That backup is already being deleted."));
        }

        let name = backup.name.clone();
        let uuid = backup.uuid;
        let state = context.state.clone();
        tokio::spawn(async move { backup.delete(&state, Default::default()).await })
            .await
            .map_err(anyhow::Error::from)?
            .map_err(refusal)?;
        context
            .log("backup.delete", json!({"uuid": uuid, "name": name}))
            .await;

        Ok(ToolResult::ok(format!("Deleted backup \"{name}\".")).with_display("Deleted backup"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_show_display_errors_only() {
        let shown = refusal(anyhow::Error::new(crate::response::DisplayError::new(
            "backup configuration is in maintenance",
        )));
        assert!(matches!(
            shown,
            ToolError::Invalid(message)
                if message == "The panel refused this: backup configuration is in maintenance."
        ));
        assert!(matches!(
            refusal(anyhow::anyhow!("connection refused")),
            ToolError::Failed(_)
        ));
    }

    #[test]
    fn summaries_name_the_backup() {
        assert_eq!(
            RestoreBackupTool.summarize(&json!({"backup": "abc", "delete_existing": true})),
            "Restore backup abc (deleting all current files first)"
        );
        assert_eq!(
            CreateBackupTool.summarize(&json!({"name": "  "})),
            "Create backup"
        );
        assert_eq!(
            CreateBackupTool.summarize(&json!({"name": "before update"})),
            "Create backup \"before update\""
        );
    }
}
