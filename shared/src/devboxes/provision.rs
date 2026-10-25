//! plugged.host: creating, repairing and removing devboxes (docs/features/devboxes.md).
//!
//! A devbox's server is made on its game server's node, from the devbox egg, owned by the system
//! account, with one free allocation of the node as its SSH port. Its mount of the game server's
//! volume at `/server` is no mount row: the Wings configuration of a devbox server gets it from
//! its `devboxes` row ([`MOUNT_SQL`]), only while both servers are on the same node. Wings binds
//! it only when the node allows its data directory in `allowed_mounts`, which is checked here
//! before anything is made.

use super::{Context, Devbox, DevboxError, Kind};
use crate::models::{
    ByUuid, CreatableModel, DeletableModel,
    egg_configuration::{
        EggConfigAllocationDeploymentPrimaryAllocation, EggConfigAllocationsDeployment,
    },
    nest_egg::NestEgg,
    node::{Node, NodeAllowedSources},
    node_allocation::NodeAllocation,
    server::{
        AdminApiServerLimits, ApiServerFeatureLimits, CreateServerOptions, DeleteServerOptions,
        Server,
    },
    user::{CreateUserOptions, User},
};
use rand::distr::SampleString;
use std::collections::HashMap;

/// Where a devbox server's mount comes from in its Wings configuration: its row's volume source,
/// only while the game server is on the devbox's node and the source is that server's volume.
pub const MOUNT_SQL: &str = "SELECT devboxes.mount_source
    FROM devboxes
    JOIN servers game ON game.uuid = devboxes.game_server_uuid
    JOIN servers devbox ON devbox.uuid = devboxes.server_uuid
    WHERE devboxes.server_uuid = $1
        AND devboxes.mount_source IS NOT NULL
        AND game.node_uuid = devbox.node_uuid
        AND devboxes.mount_source LIKE '%/' || game.uuid::text";
/// Where the game server's files are inside a devbox.
pub const MOUNT_TARGET: &str = "/server";
/// The system account's email: `.invalid` is reserved, so it never belongs to anyone.
const SYSTEM_EMAIL: &str = "devboxes@system.invalid";
/// Wings' defaults (`/var/lib/calagopus-wings`, `{root_directory}/volumes`).
const WINGS_ROOT: &str = "/var/lib/calagopus-wings";
const WINGS_DATA: &str = "{root_directory}/volumes";

/// A new devbox's slug: its browser address, `<slug>.<web domain>`.
fn new_slug() -> String {
    rand::distr::Alphanumeric
        .sample_string(&mut rand::rng(), 16)
        .to_ascii_lowercase()
}

/// The game server's volume on its node, from the node's Wings configuration.
pub fn volume_source(config: &serde_json::Value, game_uuid: uuid::Uuid) -> Option<String> {
    let system = config.get("system");
    let field = |name: &str, default: &str| {
        system
            .and_then(|system| system.get(name))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(default)
            .to_string()
    };

    let root = field("root_directory", WINGS_ROOT);
    let data = field("data", WINGS_DATA).replace("{root_directory}", &root);
    let data = data.trim_end_matches('/');
    if !data.starts_with('/') || data.split('/').any(|part| part == "..") {
        return None;
    }

    Some(format!("{data}/{game_uuid}"))
}

/// The account that owns every devbox server: made the first time (suspended, no password, no
/// sign-in), then kept in the settings.
pub async fn system_user(
    state: &crate::State,
    context: &Context,
) -> Result<uuid::Uuid, DevboxError> {
    if let Some(uuid) = context.devboxes.system_user_uuid
        && User::by_uuid_optional(&state.database, uuid)
            .await?
            .is_some()
    {
        return Ok(uuid);
    }

    let _lock = state
        .cache
        .lock("devboxes::system_user", Some(60), Some(30_000))
        .await?;
    let existing: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT uuid FROM users WHERE email = $1")
            .bind(SYSTEM_EMAIL)
            .fetch_optional(state.database.write())
            .await?;

    let uuid = match existing {
        Some(uuid) => uuid,
        None => {
            let language = state
                .settings
                .get_as(|settings| settings.app.language.clone())
                .await?;
            let username = crate::noprofit::unique_username(&state.database, "devboxes").await?;
            let user = User::create(
                state,
                CreateUserOptions {
                    role_uuid: None,
                    external_id: Some("system:devboxes".into()),
                    username: username.into(),
                    email: SYSTEM_EMAIL.into(),
                    name_first: Some("Devboxes".into()),
                    name_last: None,
                    password: None,
                    admin: false,
                    frozen: false,
                    suspended: true,
                    verify_email: true,
                    send_email: false,
                    language,
                },
            )
            .await?;
            sqlx::query("UPDATE users SET password_login_disabled = TRUE WHERE uuid = $1")
                .bind(user.uuid)
                .execute(state.database.write())
                .await?;
            User::invalidate_cached(&state.database, user.uuid).await;

            user.uuid
        }
    };

    let mut settings = state.settings.get_mut().await?;
    settings.devboxes.system_user_uuid = Some(uuid);
    settings.save().await?;

    Ok(uuid)
}

/// The devbox `user_uuid` works in on `game`, created (not started) when it doesn't exist yet,
/// with its server made again when it is gone or stayed behind when the game server moved.
pub async fn ensure(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
    game: &Server,
) -> Result<Devbox, DevboxError> {
    let (kind, _) = super::resolve(state, context, user_uuid, game)
        .await?
        .ok_or(DevboxError::NotAllowed)?;
    let payer = match kind {
        Kind::Shared => game.owner.uuid,
        Kind::Personal => user_uuid,
    };
    if let Some(reason) = super::refusal(state, user_uuid, game, payer).await? {
        return Err(reason);
    }

    let owner = (kind == Kind::Personal).then_some(user_uuid);
    let _lock = state
        .cache
        .lock(
            format!(
                "devboxes::ensure::{}::{}",
                game.uuid,
                owner.map_or_else(|| "shared".to_string(), |owner| owner.to_string())
            ),
            Some(180),
            Some(90_000),
        )
        .await?;

    let mut devbox = match owner {
        Some(owner) => match Devbox::personal(&state.database, game.uuid, owner).await? {
            Some(devbox) => devbox,
            None => create_row(state, game.uuid, Some(owner)).await?,
        },
        // a shared devbox is only made by its owner switching it on
        None => Devbox::shared_of(&state.database, game.uuid)
            .await?
            .ok_or(DevboxError::Missing)?,
    };

    let server = match devbox.server_uuid {
        Some(uuid) => Server::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    };
    if let Some(server) = &server
        && server.node.uuid == game.node.uuid
    {
        return Ok(devbox);
    }
    if server.is_some() {
        remove_server(state, &devbox).await?;
        devbox.refresh(&state.database).await?;
    }

    create_server(state, context, &devbox, game, payer).await?;
    devbox.refresh(&state.database).await?;

    Ok(devbox)
}

async fn create_row(
    state: &crate::State,
    game_server_uuid: uuid::Uuid,
    user_uuid: Option<uuid::Uuid>,
) -> Result<Devbox, DevboxError> {
    let uuid: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO devboxes (game_server_uuid, user_uuid, slug, shared_rights)
        VALUES ($1, $2, $3, $4)
        RETURNING uuid",
    )
    .bind(game_server_uuid)
    .bind(user_uuid)
    .bind(new_slug())
    .bind(
        user_uuid
            .is_none()
            .then(|| serde_json::to_value(super::SharedRights::default()))
            .transpose()
            .map_err(anyhow::Error::from)?,
    )
    .fetch_one(state.database.write())
    .await?;

    Devbox::by_uuid(&state.database, uuid)
        .await?
        .ok_or(DevboxError::Missing)
}

async fn create_server(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    game: &Server,
    payer: uuid::Uuid,
) -> Result<(), DevboxError> {
    let node = game.node.fetch_cached(&state.database).await?;
    let tier = super::tier(state, context, payer)
        .await?
        .ok_or(DevboxError::NotAllowed)?;
    let limits = tier.limits(&context.devboxes);
    let egg_uuid = context.devboxes.egg_uuid.ok_or(DevboxError::NotSetUp)?;
    let egg = NestEgg::by_uuid_optional(&state.database, egg_uuid)
        .await?
        .ok_or(DevboxError::NotSetUp)?;
    let owner = system_user(state, context).await?;

    // Wings binds the volume only when its data directory is in allowed_mounts
    let config = node
        .fetch_configuration(&state.database)
        .await
        .map_err(|_| DevboxError::NodeDown)?;
    let source = volume_source(&config.0, game.uuid).ok_or(DevboxError::NodeNotReady)?;
    let allowed = NodeAllowedSources::from_config(&config).map_err(anyhow::Error::from)?;
    if allowed.allows_mount(&source, MOUNT_TARGET) != Some(true) {
        return Err(DevboxError::NodeNotReady);
    }

    let image = match context.devboxes.image.trim() {
        "" => egg
            .docker_images
            .values()
            .next()
            .cloned()
            .ok_or(DevboxError::NotSetUp)?,
        image => image.into(),
    };
    let startup = egg
        .startup_commands
        .values()
        .next()
        .cloned()
        .unwrap_or_else(|| "ph serve".into());
    let label = match devbox.user_uuid {
        Some(user_uuid) => {
            sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE uuid = $1")
                .bind(user_uuid)
                .fetch_optional(state.database.read())
                .await?
                .unwrap_or_default()
        }
        None => "shared".to_string(),
    };
    let short = game.uuid.simple().to_string();
    let feature_limits: ApiServerFeatureLimits = serde_json::from_value(serde_json::json!({
        "allocations": 0,
        "databases": 0,
        "backups": 0,
        "schedules": 0,
    }))
    .map_err(anyhow::Error::from)?;

    // one allocation pick at a time per node, so two devboxes never grab the same port
    let _lock = state
        .cache
        .lock(
            format!("devboxes::allocate::{}", node.uuid),
            Some(120),
            Some(60_000),
        )
        .await?;
    let allocation = free_allocation(state, &node).await?;

    let server = Server::create(
        state,
        CreateServerOptions {
            node_uuid: node.uuid,
            owner_uuid: owner,
            egg_uuid: egg.uuid,
            backup_configuration_uuid: None,
            allocation_uuid: Some(allocation),
            allocation_uuids: Vec::new(),
            start_on_completion: false,
            skip_installer: true,
            external_id: Some(format!("devbox:{}", devbox.uuid).into()),
            name: format!("devbox {} {label}", &short[..8])
                .chars()
                .take(180)
                .collect::<String>()
                .into(),
            description: Some(
                format!(
                    "{} devbox for server {} (system server, managed by the panel: do not edit)",
                    context.app_name,
                    &short[..8]
                )
                .chars()
                .take(1024)
                .collect::<String>()
                .into(),
            ),
            limits: AdminApiServerLimits {
                cpu: limits.cpu,
                memory: limits.memory as i64,
                memory_overhead: 0,
                swap: 0,
                disk: limits.disk as i64,
                io_weight: None,
                bandwidth: None,
            },
            pinned_cpus: Vec::new(),
            startup,
            image,
            labels: Default::default(),
            timezone: None,
            trash: None,
            hugepages_passthrough_enabled: false,
            kvm_passthrough_enabled: false,
            feature_limits,
            variables: HashMap::new(),
        },
    )
    .await?;

    sqlx::query(
        "UPDATE devboxes SET server_uuid = $2, node_uuid = $3, mount_source = $4, token_hash = NULL,
            code_secret = NULL, stopping = NULL, boot_status = NULL, boot_detail = NULL,
            connections = 0, version = NULL, host_key = NULL
        WHERE uuid = $1",
    )
    .bind(devbox.uuid)
    .bind(server.uuid)
    .bind(node.uuid)
    .bind(&source)
    .execute(state.database.write())
    .await?;
    tracing::info!(devbox = %devbox.uuid, server = %server.uuid, game = %game.uuid, "devbox created");

    Ok(())
}

/// Any free allocation of the node.
async fn free_allocation(state: &crate::State, node: &Node) -> Result<uuid::Uuid, DevboxError> {
    let deployment = EggConfigAllocationsDeployment {
        dedicated: false,
        primary: Some(EggConfigAllocationDeploymentPrimaryAllocation {
            start_port: 1,
            end_port: u16::MAX,
            assign_to_variable: None,
        }),
        additional: Vec::new(),
    };
    let mut variables = HashMap::new();

    match NodeAllocation::get_from_deployment(state, &deployment, node, &mut variables).await {
        Ok((Some(primary), _)) => Ok(primary),
        Ok((None, _)) | Err(crate::database::DatabaseError::Any(_)) => Err(DevboxError::NoPort),
        Err(err) => Err(err.into()),
    }
}

/// Deletes the devbox's server on Wings first, and only then in the panel, so a devbox Wings
/// still runs never loses the row that lets the panel stop it.
async fn remove_server(state: &crate::State, devbox: &Devbox) -> Result<(), DevboxError> {
    if let Some(uuid) = devbox.server_uuid
        && let Some(server) = Server::by_uuid_optional(&state.database, uuid).await?
    {
        let node = server.node.fetch_cached(&state.database).await?;
        match node
            .api_client(&state.database)
            .await?
            .delete_servers_server(server.uuid)
            .await
        {
            Ok(_) => {}
            Err(wings_api::client::ApiHttpError::Http(status, _))
                if status == reqwest::StatusCode::NOT_FOUND => {}
            Err(_) => return Err(DevboxError::NodeDown),
        }

        server
            .delete(state, DeleteServerOptions { force: true })
            .await?;
    }

    sqlx::query(
        "UPDATE devboxes SET server_uuid = NULL, node_uuid = NULL, mount_source = NULL,
            token_hash = NULL, code_secret = NULL, stopping = NULL, connections = 0
        WHERE uuid = $1",
    )
    .bind(devbox.uuid)
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// Removes a devbox completely: its server (through Wings), its DNS record and the row. Nothing
/// is removed while the abuse checks hold a live critical finding on its server (the finding and
/// its evidence would go with it) or while Wings can't confirm the deletion: the row stays, and
/// the reconcile asks again every minute.
pub async fn destroy(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
) -> Result<(), DevboxError> {
    if super::blocks::under_review(&state.database, devbox).await? {
        return Err(DevboxError::Abuse);
    }

    remove_server(state, devbox).await?;
    if let Err(err) = super::dns::remove(state, context, devbox).await {
        tracing::warn!(devbox = %devbox.uuid, "failed to remove a devbox's DNS record: {err:#}");
    }
    sqlx::query("DELETE FROM devboxes WHERE uuid = $1")
        .bind(devbox.uuid)
        .execute(state.database.write())
        .await?;
    tracing::info!(devbox = %devbox.uuid, game = %devbox.game_server_uuid, "devbox removed");

    Ok(())
}

/// The owner switches the shared devbox on: everyone's personal devbox on the server goes (one
/// that can't go right now is retried by the reconcile).
pub async fn enable_shared(
    state: &crate::State,
    context: &Context,
    game: &Server,
) -> Result<Devbox, DevboxError> {
    let devbox = {
        let _lock = state
            .cache
            .lock(
                format!("devboxes::ensure::{}::shared", game.uuid),
                Some(180),
                Some(90_000),
            )
            .await?;

        match Devbox::shared_of(&state.database, game.uuid).await? {
            Some(devbox) => devbox,
            None => create_row(state, game.uuid, None).await?,
        }
    };

    for personal in Devbox::all_of_game(&state.database, game.uuid).await? {
        if personal.is_shared() {
            continue;
        }
        if let Err(err) = destroy(state, context, &personal).await {
            tracing::warn!(devbox = %personal.uuid, "failed to remove a personal devbox: {}", err.message());
        }
    }

    Ok(devbox)
}

pub async fn disable_shared(
    state: &crate::State,
    context: &Context,
    game: &Server,
) -> Result<(), DevboxError> {
    match Devbox::shared_of(&state.database, game.uuid).await? {
        Some(shared) => destroy(state, context, &shared).await,
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_follows_the_wings_data_directory() {
        let game = uuid::Uuid::nil();

        assert_eq!(
            volume_source(&serde_json::json!({}), game).as_deref(),
            Some("/var/lib/calagopus-wings/volumes/00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(
            volume_source(
                &serde_json::json!({"system": {"root_directory": "/srv/wings", "data": "{root_directory}/data/"}}),
                game
            )
            .as_deref(),
            Some("/srv/wings/data/00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(
            volume_source(&serde_json::json!({"system": {"data": "volumes"}}), game),
            None,
            "a relative directory can't be checked against allowed_mounts"
        );
        assert_eq!(
            volume_source(
                &serde_json::json!({"system": {"data": "/srv/../etc"}}),
                game
            ),
            None
        );
    }

    #[test]
    fn slugs_fit_the_edge() {
        let slug = new_slug();

        assert_eq!(slug.len(), 16);
        assert!(
            slug.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
    }
}
