//! plugged.host: the notification inbox (docs/features/notifications.md): what the panel tells a
//! person after the fact, kept until they read or delete it, like the Pelican panel's bell.
//!
//! A notification is a `kind` (the constants below, which name its text in the frontend's
//! `elements.notifications.kinds`) and the `data` the text is drawn with, so it reads in whatever
//! language the person uses when they open it. Kinds under `admin.` are for admins and written in
//! English and German only. [`send`] is the one way in; the functions after it are the senders other
//! modules call with one line where something finished (a move, a failover, a modpack install).
//! [`register`] follows upstream's own events, as the Pelican panel's bell did besides its mails: a
//! finished install, being added to or removed from a server. A finished backup is told to whoever
//! asked for its mail (`backup_notifications`). Sending never fails the caller: a notification that
//! cannot be written is logged.

use crate::models::{
    ByUuid, CreatableModel, DeletableModel, EventEmittingModel, ListenerPriority,
    location::Location,
    server::{Server, ServerEvent},
    server_backup::ServerBackup,
    server_subuser::ServerSubuser,
    user_notification::{CreateUserNotificationOptions, UserNotification},
};
use serde_json::json;

/// A move the owner (or whoever asked) wanted is done: the server is in its new location.
pub const TRANSFER_COMPLETED: &str = "transferCompleted";
/// A move gave up; the server stayed where it was.
pub const TRANSFER_FAILED: &str = "transferFailed";
/// For admins: a move gave up (any move, an admin's "Lighten load" included).
pub const ADMIN_TRANSFER_FAILED: &str = "admin.transferFailed";
/// The account's allowance went down and its servers gave the difference back.
pub const ALLOWANCE_CUT: &str = "allowanceCut";
/// The second membership ran out and servers left the premium locations.
pub const MEMBERSHIP_ENDED: &str = "membershipEnded";
/// A failover (or a drill) restored the server on another node.
pub const FAILOVER_DONE: &str = "failoverDone";
/// A failover moved the server and then could not restore it.
pub const FAILOVER_FAILED: &str = "failoverFailed";
pub const MODPACK_INSTALLED: &str = "modpackInstalled";
pub const MODPACK_FAILED: &str = "modpackFailed";
pub const FRAMEWORK_INSTALLED: &str = "frameworkInstalled";
pub const FRAMEWORK_FAILED: &str = "frameworkFailed";
pub const WORLD_IMPORTED: &str = "worldImported";
pub const WORLD_IMPORT_FAILED: &str = "worldImportFailed";
/// An install or a reinstall ended (upstream's `InstallCompleted`), for the owner.
pub const SERVER_INSTALLED: &str = "serverInstalled";
pub const SERVER_INSTALL_FAILED: &str = "serverInstallFailed";
/// The person was made a subuser of a server, or is not one any more.
pub const SUBUSER_ADDED: &str = "subuserAdded";
pub const SUBUSER_REMOVED: &str = "subuserRemoved";
/// A backup of a server ended, for whoever asked for its mail.
pub const BACKUP_COMPLETED: &str = "backupCompleted";
pub const BACKUP_FAILED: &str = "backupFailed";
/// The partner program (shared/src/partners.rs, docs/features/partners.md): an application was
/// approved (`credits` a month), rejected or a partnership revoked (`reason`).
pub const PARTNER_APPROVED: &str = "partnerApproved";
pub const PARTNER_REJECTED: &str = "partnerRejected";
pub const PARTNER_REVOKED: &str = "partnerRevoked";
/// A month's credits were paid.
pub const PARTNER_PAID: &str = "partnerPaid";
/// The monthly check failed (`problems`); the month is held `until` the grace days end.
pub const PARTNER_CHECK_FAILED: &str = "partnerCheckFailed";
/// The checks kept failing past the grace days.
pub const PARTNER_SUSPENDED: &str = "partnerSuspended";
/// An admin paused the partnership (`reason`), resumed it (or a check lifted a suspension), or
/// changed its credits a month.
pub const PARTNER_PAUSED: &str = "partnerPaused";
pub const PARTNER_RESUMED: &str = "partnerResumed";
pub const PARTNER_CREDITS_CHANGED: &str = "partnerCreditsChanged";
/// For admins: a new partner application, a partnership suspended.
pub const ADMIN_PARTNER_APPLICATION: &str = "admin.partnerApplication";
pub const ADMIN_PARTNER_SUSPENDED: &str = "admin.partnerSuspended";

/// Read notifications are deleted after this many days (the `delete_old_activity` task).
pub const KEEP_READ_DAYS: i64 = 90;
/// Unread ones too, after this many: an inbox nobody opens does not grow for ever.
pub const KEEP_DAYS: i64 = 365;
/// A failed failover tells the owner at most once in this many hours (the Pelican panel's day).
const FAILOVER_FAILED_EVERY_HOURS: i64 = 24;
/// The longest error text a notification keeps.
const MAX_ERROR_CHARS: usize = 300;

/// Writes a notification for `user_uuid`. `server_uuid` links the server it is about.
pub async fn send(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    server_uuid: Option<uuid::Uuid>,
    kind: &str,
    data: serde_json::Value,
) {
    let options = CreateUserNotificationOptions {
        user_uuid,
        server_uuid,
        kind: kind.into(),
        data,
    };

    if let Err(err) = UserNotification::create(state, options).await {
        tracing::warn!(user = %user_uuid, kind, "failed to write a notification: {err:#?}");
    }
}

/// Writes a notification for every admin who holds `permission` (and every full admin).
pub async fn send_admins(
    state: &crate::State,
    permission: &str,
    server_uuid: Option<uuid::Uuid>,
    kind: &str,
    data: serde_json::Value,
) {
    let admins: Vec<uuid::Uuid> = match sqlx::query_scalar(
        "SELECT users.uuid FROM users
        LEFT JOIN roles ON roles.uuid = users.role_uuid
        WHERE NOT users.suspended AND (users.admin OR $1 = ANY(roles.admin_permissions))",
    )
    .bind(permission)
    .fetch_all(state.database.read())
    .await
    {
        Ok(admins) => admins,
        Err(err) => {
            tracing::warn!(kind, "failed to find the admins to notify: {err:#}");
            return;
        }
    };

    for admin in admins {
        send(state, admin, server_uuid, kind, data.clone()).await;
    }
}

/// Who hears that a move ended: the owner and whoever asked for it. An admin's move (a "Lighten
/// load", a membership running out) is the admin's business until it gives the owner a new
/// address, so only its completion goes to the owner.
pub fn transfer_recipients(
    owner: uuid::Uuid,
    requester: Option<uuid::Uuid>,
    admin_origin: bool,
    completed: bool,
) -> Vec<uuid::Uuid> {
    if admin_origin {
        return if completed { vec![owner] } else { Vec::new() };
    }

    let mut recipients = vec![owner];
    if let Some(requester) = requester
        && requester != owner
    {
        recipients.push(requester);
    }

    recipients
}

/// The address players connect to: the primary allocation's alias, or its IP, and port.
fn address(server: &Server) -> Option<String> {
    server.allocation.as_ref().map(|primary| {
        let allocation = &primary.allocation;

        format!(
            "{}:{}",
            allocation
                .ip_alias
                .as_deref()
                .map_or_else(|| allocation.ip.ip().to_string(), str::to_string),
            allocation.port
        )
    })
}

/// At most `MAX_ERROR_CHARS` of an error, on a character boundary.
pub fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_ERROR_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

async fn location_name(state: &crate::State, uuid: Option<uuid::Uuid>) -> Option<String> {
    let uuid = uuid?;

    Location::by_uuid_optional(&state.database, uuid)
        .await
        .ok()
        .flatten()
        .map(|location| location.name.to_string())
}

/// A move ended (`server_transfers::settle`): completed for the owner and whoever asked, failed for
/// them and for the admins.
pub async fn transfer_ended(
    state: &crate::State,
    request: &crate::server_transfers::TransferRequest,
) {
    let completed = request.status == crate::server_transfers::COMPLETED;
    if !completed && request.status != crate::server_transfers::FAILED {
        return;
    }

    let server = match Server::by_uuid_optional(&state.database, request.server_uuid).await {
        Ok(Some(server)) => server,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(server = %request.server_uuid, "failed to load a moved server to notify: {err:#}");
            return;
        }
    };

    let location = match location_name(state, request.location_uuid).await {
        Some(name) => Some(name),
        None if completed => server
            .node
            .fetch_cached(&state.database)
            .await
            .ok()
            .map(|node| node.location.name.to_string()),
        None => None,
    };
    let reason = request
        .error
        .as_deref()
        .and_then(crate::server_transfers::reason_key);

    let data = if completed {
        json!({ "server": server.name, "location": location, "address": address(&server) })
    } else {
        json!({ "server": server.name, "location": location, "reason": reason })
    };
    for user_uuid in transfer_recipients(
        server.owner.uuid,
        request.user_uuid,
        request.origin == crate::server_transfers::ORIGIN_ADMIN,
        completed,
    ) {
        send(
            state,
            user_uuid,
            Some(server.uuid),
            if completed {
                TRANSFER_COMPLETED
            } else {
                TRANSFER_FAILED
            },
            data.clone(),
        )
        .await;
    }

    if !completed {
        send_admins(
            state,
            "servers.transfer",
            Some(server.uuid),
            ADMIN_TRANSFER_FAILED,
            json!({
                "server": server.name,
                "location": location,
                "reason": reason,
                "error": request.error.as_deref().map(clip),
            }),
        )
        .await;
    }
}

/// An allowance cut took resources off the account's servers (`allowance_cuts::settle_user`):
/// `servers` is the list it took from, `[{name, cpu, memory, disk}]` (CPU in %, the rest in MiB).
pub async fn allowance_cut(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    servers: serde_json::Value,
) {
    send(
        state,
        user_uuid,
        None,
        ALLOWANCE_CUT,
        json!({ "servers": servers }),
    )
    .await;
}

/// The second membership ran out and these servers are moving off the premium locations
/// (`economy::settle_lapse`).
pub async fn membership_ended(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    second: bool,
    servers: &[String],
) {
    if servers.is_empty() {
        return;
    }

    let membership = match state.settings.get().await {
        Ok(settings) if second => settings.economy.plusplus_name.to_string(),
        Ok(settings) => settings.economy.plus_name.to_string(),
        Err(err) => {
            tracing::warn!(user = %user_uuid, "failed to read the membership's name: {err:#}");
            return;
        }
    };

    send(
        state,
        user_uuid,
        None,
        MEMBERSHIP_ENDED,
        json!({ "membership": membership, "servers": servers }),
    )
    .await;
}

/// A failover or a drill restored the server on another node (`node_failover::check_restore`).
pub async fn failover_done(
    state: &crate::State,
    server: &Server,
    backup_uuid: Option<uuid::Uuid>,
    drill: bool,
) {
    let backup = match backup_uuid {
        Some(uuid) => ServerBackup::by_uuid_optional(&state.database, uuid)
            .await
            .ok()
            .flatten()
            .map(|backup| backup.created.and_utc()),
        None => None,
    };

    send(
        state,
        server.owner.uuid,
        Some(server.uuid),
        FAILOVER_DONE,
        json!({
            "server": server.name,
            "address": address(server),
            "backup": backup,
            "drill": drill,
        }),
    )
    .await;
}

/// A failover failed (`node_failover::fail`). The owner hears of it only when the server had
/// already moved off `source_node_uuid` (before that nothing changed for them), and at most once a
/// day.
pub async fn failover_failed(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    source_node_uuid: uuid::Uuid,
) {
    let server = match Server::by_uuid_optional(&state.database, server_uuid).await {
        Ok(Some(server)) => server,
        _ => return,
    };
    if server.node.uuid == source_node_uuid {
        return;
    }

    let since = chrono::Utc::now() - chrono::Duration::hours(FAILOVER_FAILED_EVERY_HOURS);
    match UserNotification::sent_since(
        &state.database,
        server.owner.uuid,
        server.uuid,
        FAILOVER_FAILED,
        since,
    )
    .await
    {
        Ok(false) => {}
        Ok(true) => return,
        Err(err) => {
            tracing::warn!(server = %server.uuid, "failed to check earlier failover notifications: {err:#}");
            return;
        }
    }

    send(
        state,
        server.owner.uuid,
        Some(server.uuid),
        FAILOVER_FAILED,
        json!({ "server": server.name }),
    )
    .await;
}

/// A modpack install ended (its route and neo's tool): for the person who started it.
pub async fn modpack_ended(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    server_uuid: uuid::Uuid,
    server_name: &str,
    install: &crate::minecraft::modpacks::Install,
) {
    let data = match &install.error {
        None => json!({
            "server": server_name,
            "name": install.name,
            "files": install.files,
            "skipped": install.skipped.len(),
        }),
        Some(error) => json!({ "server": server_name, "name": install.name, "error": clip(error) }),
    };

    send(
        state,
        user_uuid,
        Some(server_uuid),
        if install.error.is_none() {
            MODPACK_INSTALLED
        } else {
            MODPACK_FAILED
        },
        data,
    )
    .await;
}

/// A FiveM framework install ended (`fivem::frameworks::install`): for the person who started it.
pub async fn framework_ended(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    server: &Server,
    install: &crate::fivem::frameworks::Install,
) {
    let (kind, data) = match &install.error {
        None => (
            FRAMEWORK_INSTALLED,
            json!({ "server": server.name, "recipe": install.recipe, "folder": install.deploy }),
        ),
        Some(error) => (
            FRAMEWORK_FAILED,
            json!({ "server": server.name, "recipe": install.recipe, "error": clip(error) }),
        ),
    };

    send(state, user_uuid, Some(server.uuid), kind, data).await;
}

/// An uploaded world archive was made the loaded world, or could not be (`kept`: the old world's
/// new name; `error`: why it failed): for the person who started it.
pub async fn world_import_ended(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    server: &Server,
    kept: Option<&str>,
    error: Option<&str>,
) {
    let (kind, data) = match error {
        None => (
            WORLD_IMPORTED,
            json!({ "server": server.name, "kept": kept }),
        ),
        Some(error) => (
            WORLD_IMPORT_FAILED,
            json!({ "server": server.name, "error": clip(error) }),
        ),
    };

    send(state, user_uuid, Some(server.uuid), kind, data).await;
}

/// A backup of the server ended (`backup_notifications`, for whoever asked for its mail).
pub async fn backup_ended(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    server: &Server,
    backup_name: &str,
    successful: bool,
) {
    send(
        state,
        user_uuid,
        Some(server.uuid),
        if successful {
            BACKUP_COMPLETED
        } else {
            BACKUP_FAILED
        },
        json!({ "server": server.name, "name": backup_name }),
    )
    .await;
}

/// [`send`] from a model's handler, inside the change's transaction: in a savepoint, so a
/// notification that cannot be written neither fails nor spoils the change it is about.
async fn send_in_transaction(
    state: &crate::State,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    options: CreateUserNotificationOptions,
) {
    let (user_uuid, kind) = (options.user_uuid, options.kind.clone());
    let mut savepoint = match sqlx::Connection::begin(&mut **transaction).await {
        Ok(savepoint) => savepoint,
        Err(err) => {
            tracing::warn!(user = %user_uuid, %kind, "failed to open a savepoint for a notification: {err:#}");
            return;
        }
    };

    let ended = match UserNotification::create_with_transaction(state, options, &mut savepoint)
        .await
    {
        Ok(()) => savepoint.commit().await,
        Err(err) => {
            tracing::warn!(user = %user_uuid, %kind, "failed to write a notification: {err:#?}");
            savepoint.rollback().await
        }
    };
    if let Err(err) = ended {
        tracing::warn!(user = %user_uuid, %kind, "failed to close a notification's savepoint: {err:#}");
    }
}

/// Follows upstream's events, as the Pelican panel's bell did: a finished install or reinstall
/// tells the owner; a subuser added or removed is told so, in the transaction that adds or removes
/// them. (A failover's or a limbo restore's install never reaches the event.)
pub fn register() {
    let _ = Server::register_event_handler(|state, event| async move {
        if let ServerEvent::InstallCompleted { server, successful } = &*event {
            send(
                &state,
                server.owner.uuid,
                Some(server.uuid),
                if *successful {
                    SERVER_INSTALLED
                } else {
                    SERVER_INSTALL_FAILED
                },
                json!({ "server": server.name }),
            )
            .await;
        }

        Ok(())
    });

    ServerSubuser::register_after_create_handler(
        ListenerPriority::Normal,
        |subuser, options, state, transaction| {
            Box::pin(async move {
                send_in_transaction(
                    state,
                    transaction,
                    CreateUserNotificationOptions {
                        user_uuid: subuser.user.uuid,
                        server_uuid: Some(options.server.uuid),
                        kind: SUBUSER_ADDED.into(),
                        data: json!({ "server": options.server.name }),
                    },
                )
                .await;

                Ok(())
            })
        },
    );

    ServerSubuser::register_after_delete_handler(
        ListenerPriority::Normal,
        |subuser, _options, state, transaction| {
            Box::pin(async move {
                let server = match subuser.server.fetch_cached(&state.database).await {
                    Ok(server) => server,
                    Err(err) => {
                        tracing::warn!(server = %subuser.server.uuid, "failed to load a server to tell a removed subuser: {err:#}");
                        return Ok(());
                    }
                };

                // no link: the server is not theirs to open any more
                send_in_transaction(
                    state,
                    transaction,
                    CreateUserNotificationOptions {
                        user_uuid: subuser.user.uuid,
                        server_uuid: None,
                        kind: SUBUSER_REMOVED.into(),
                        data: json!({ "server": server.name }),
                    },
                )
                .await;

                Ok(())
            })
        },
    );
}

/// The `delete_old_activity` task's share: read notifications after [`KEEP_READ_DAYS`], any after
/// [`KEEP_DAYS`].
pub async fn prune(state: &crate::State) -> Result<u64, sqlx::Error> {
    let now = chrono::Utc::now();

    UserNotification::delete_older_than(
        &state.database,
        now - chrono::Duration::days(KEEP_READ_DAYS),
        now - chrono::Duration::days(KEEP_DAYS),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> uuid::Uuid {
        uuid::Uuid::from_u128(n)
    }

    #[test]
    fn a_players_move_tells_the_owner_and_whoever_asked() {
        assert_eq!(
            transfer_recipients(id(1), Some(id(2)), false, true),
            vec![id(1), id(2)]
        );
        assert_eq!(
            transfer_recipients(id(1), Some(id(2)), false, false),
            vec![id(1), id(2)]
        );
        // the owner asked: told once
        assert_eq!(
            transfer_recipients(id(1), Some(id(1)), false, true),
            vec![id(1)]
        );
        assert_eq!(transfer_recipients(id(1), None, false, false), vec![id(1)]);
    }

    #[test]
    fn an_admins_move_tells_the_owner_only_of_the_new_address() {
        assert_eq!(
            transfer_recipients(id(1), Some(id(9)), true, true),
            vec![id(1)]
        );
        assert!(transfer_recipients(id(1), Some(id(9)), true, false).is_empty());
    }

    #[test]
    fn long_errors_are_clipped_on_a_character() {
        assert_eq!(clip("no node"), "no node");

        let long = "é".repeat(MAX_ERROR_CHARS + 5);
        let clipped = clip(&long);
        assert_eq!(clipped.chars().count(), MAX_ERROR_CHARS + 1);
        assert!(clipped.ends_with('…'));

        assert_eq!(clip(&"a".repeat(MAX_ERROR_CHARS)).len(), MAX_ERROR_CHARS);
    }

    #[test]
    fn kinds_fit_their_column_and_admin_ones_are_marked() {
        for kind in [
            TRANSFER_COMPLETED,
            TRANSFER_FAILED,
            ADMIN_TRANSFER_FAILED,
            ALLOWANCE_CUT,
            MEMBERSHIP_ENDED,
            FAILOVER_DONE,
            FAILOVER_FAILED,
            MODPACK_INSTALLED,
            MODPACK_FAILED,
            FRAMEWORK_INSTALLED,
            FRAMEWORK_FAILED,
            WORLD_IMPORTED,
            WORLD_IMPORT_FAILED,
            SERVER_INSTALLED,
            SERVER_INSTALL_FAILED,
            SUBUSER_ADDED,
            SUBUSER_REMOVED,
            BACKUP_COMPLETED,
            BACKUP_FAILED,
            PARTNER_APPROVED,
            PARTNER_REJECTED,
            PARTNER_REVOKED,
            PARTNER_PAID,
            PARTNER_CHECK_FAILED,
            PARTNER_SUSPENDED,
            PARTNER_PAUSED,
            PARTNER_RESUMED,
            PARTNER_CREDITS_CHANGED,
            ADMIN_PARTNER_APPLICATION,
            ADMIN_PARTNER_SUSPENDED,
        ] {
            assert!(!kind.is_empty() && kind.len() <= 64, "{kind}");
            // the frontend reads `kinds.<kind>`: only admin ones have a dot, under `admin`
            assert!(!kind.contains('.') || kind.starts_with("admin."), "{kind}");
        }
        assert!(KEEP_READ_DAYS < KEEP_DAYS);
    }
}
