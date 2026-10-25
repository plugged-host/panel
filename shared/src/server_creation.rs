//! plugged.host: players creating their own servers (docs/features/create-server.md).
//!
//! The create page asks [`options`] what it may offer (the games of the games settings, the public
//! locations, the plans, the account's allowance; with a size, which locations have room for it)
//! and sends the choice (a plan, or its own CPU, memory and disk) to [`create`], which checks it
//! against the same rules and then goes the native way: the most eligible node of the location
//! (`Node::by_location_uuids_most_eligible`, which skips paused nodes and ranks by load), the
//! allocations its egg configuration deploys (`NodeAllocation::get_from_deployment`), else ports
//! inside the deployment port ranges, made when missing (`allocations::deploy`, two in a row for a
//! game with a second port), and `Server::create`. An account creates one server at a time.

use crate::{
    allowance::{Allowance, Refusal, Resources},
    models::{
        ByUuid, CreatableModel,
        egg_configuration::{EggConfiguration, EggConfigurationTarget},
        location::Location,
        nest_egg::NestEgg,
        nest_egg_variable::NestEggVariable,
        node::Node,
        node_allocation::NodeAllocation,
        plan::Plan,
        server::{AdminApiServerLimits, ApiServerFeatureLimits, CreateServerOptions, Server},
        user::User,
    },
    settings::games::Game,
};
use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

/// The egg feature that makes players accept a EULA (Minecraft's).
pub const EULA_FEATURE: &str = "eula";
/// How long an accepted EULA waits for the install to finish.
const EULA_MARKER_SECONDS: u64 = 7 * 24 * 60 * 60;
/// The node role that takes players' servers.
pub const GAME_ROLE: &str = "game";
/// An admin deploy's defaults: suspended servers count half, some randomness among equals.
pub const SUSPENSION_PENALTY: f64 = 0.5;
pub const RANDOMNESS: f64 = 0.5;
/// The longest server name a player may give (the Pelican panel's).
pub const NAME_MAX: usize = 191;
/// Creates tried again when a picked port was claimed by someone else in the meantime.
const CLAIM_ATTEMPTS: usize = 3;

#[derive(ToSchema, Serialize)]
pub struct ApiGameVariant {
    pub value: compact_str::CompactString,
    pub label: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiGame {
    pub egg_uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,
    /// The picture on the game's card (the games settings' icon).
    pub icon: Option<compact_str::CompactString>,
    /// The player has to accept the game's EULA.
    pub eula: bool,
    /// The server's setup asks for its software (Minecraft's, docs/features/minecraft.md#setup),
    /// so the create page does not; the first variant is used until then.
    pub setup: bool,
    pub variants: Vec<ApiGameVariant>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiCreateLocation {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,
    pub flag: Option<compact_str::CompactString>,
    pub tier: Option<compact_str::CompactString>,
    /// The player may use the location's tier.
    pub unlocked: bool,
    /// A node there takes new servers now; asked with a size, one with room for it.
    pub available: bool,
    /// No node there takes new servers at all (paused, in maintenance, closed to deployments).
    pub full: bool,
    /// The fastest answer of its nodes to the panel, in milliseconds.
    pub latency: Option<i32>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiCreatePlan {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,
    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,
    /// It fits in what is left of the allowance.
    pub available: bool,
}

#[derive(ToSchema, Serialize)]
pub struct ApiCreateOptions {
    pub enabled: bool,
    pub allowance: Allowance,
    pub games: Vec<ApiGame>,
    pub locations: Vec<ApiCreateLocation>,
    pub plans: Vec<ApiCreatePlan>,
}

/// What a player picked on the create page.
pub struct CreateRequest {
    pub name: compact_str::CompactString,
    pub egg_uuid: uuid::Uuid,
    pub location_uuid: uuid::Uuid,
    /// A plan's size; without one, `size`.
    pub plan_uuid: Option<uuid::Uuid>,
    /// CPU, memory and disk of the player's own choosing, each at least 1.
    pub size: Option<Resources>,
    pub variant: Option<compact_str::CompactString>,
    pub eula: bool,
}

#[derive(Debug)]
pub enum CreateError {
    Disabled,
    AccountBlocked,
    Busy,
    InvalidName,
    InvalidSize,
    UnknownGame,
    EulaRequired,
    UnknownVariant,
    UnknownLocation,
    LocationLocked,
    UnknownPlan,
    Allowance(Refusal),
    /// No node can take it; the reason is the deployment blocker's or the allocation error.
    NoNode(String),
    Other(anyhow::Error),
}

impl CreateError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "creating servers is turned off".into(),
            Self::AccountBlocked => "your account cannot create servers".into(),
            Self::Busy => "another server of yours is being created, try again in a moment".into(),
            Self::InvalidName => format!("the name is required (max {NAME_MAX} characters)"),
            Self::InvalidSize => "pick a plan, or cpu, memory and disk of at least 1".into(),
            Self::UnknownGame => "this game is not offered".into(),
            Self::EulaRequired => "you have to accept the game's EULA".into(),
            Self::UnknownVariant => "this software is not offered for the game".into(),
            Self::UnknownLocation => "this location is not offered".into(),
            Self::LocationLocked => "your plan does not include this location".into(),
            Self::UnknownPlan => "this plan is not offered".into(),
            Self::Allowance(refusal) => refusal.message().into(),
            Self::NoNode(reason) => format!("no node can take the server right now: {reason}"),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    /// The HTTP status a route answers with.
    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled | Self::AccountBlocked | Self::LocationLocked => 403,
            Self::Busy => 429,
            Self::Allowance(_) | Self::NoNode(_) => 409,
            Self::Other(_) => 500,
            _ => 400,
        }
    }
}

impl From<anyhow::Error> for CreateError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for CreateError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl From<serde_json::Error> for CreateError {
    fn from(err: serde_json::Error) -> Self {
        Self::Other(err.into())
    }
}

/// Whether a player may use a location of `tier`. Every player may use the ones without; `plus`
/// comes with Plus, `premium` with PlusPlus (shared/src/economy.rs).
pub async fn tier_unlocked(
    state: &crate::State,
    user: &User,
    tier: Option<&str>,
) -> Result<bool, anyhow::Error> {
    if tier.is_none_or(str::is_empty) {
        return Ok(true);
    }

    Ok(crate::economy::membership(&state.database, user.uuid)
        .await?
        .unlocks(tier, chrono::Utc::now().naive_utc()))
}

/// What the create page offers this player; with `size`, a location is available when a node
/// there has room for a server that big.
pub async fn options(
    state: &crate::State,
    user: &User,
    size: Option<Resources>,
) -> Result<ApiCreateOptions, anyhow::Error> {
    let (enabled, catalog) = {
        let settings = state.settings.get().await?;
        (
            settings.games.create_enabled,
            settings.games.catalog.clone(),
        )
    };

    let allowance = crate::allowance::for_user(state, user.uuid).await?;

    let mut games = Vec::with_capacity(catalog.len());
    for game in &catalog {
        let Some(egg) = NestEgg::by_uuid_optional_cached(&state.database, game.egg_uuid).await?
        else {
            continue;
        };

        games.push(ApiGame {
            egg_uuid: egg.uuid,
            name: game.name.clone().unwrap_or_else(|| egg.name.clone()),
            description: game.description.clone().or_else(|| egg.description.clone()),
            icon: game.icon.clone(),
            eula: egg.features.iter().any(|feature| feature == EULA_FEATURE),
            setup: crate::minecraft::is_minecraft(&egg),
            variants: game
                .variants
                .iter()
                .map(|variant| ApiGameVariant {
                    value: variant.value.clone(),
                    label: variant.label.clone(),
                    description: variant.description.clone(),
                })
                .collect(),
        });
    }

    let locations = locations_for(state, user, size).await?;

    let plans = Plan::all_public(&state.database)
        .await?
        .into_iter()
        .map(|plan| ApiCreatePlan {
            available: allowance
                .check_new(Resources::new(plan.cpu as i64, plan.memory, plan.disk))
                .is_ok(),
            uuid: plan.uuid,
            name: plan.name,
            description: plan.description,
            cpu: plan.cpu,
            memory: plan.memory,
            disk: plan.disk,
        })
        .collect();

    Ok(ApiCreateOptions {
        enabled,
        allowance,
        games,
        locations,
        plans,
    })
}

/// The public locations as players see them: whether their tier is theirs, whether a node there
/// takes new servers now, and how fast their nodes answer. Also the transfer page's list.
pub async fn locations(
    state: &crate::State,
    user: &User,
) -> Result<Vec<ApiCreateLocation>, anyhow::Error> {
    locations_for(state, user, None).await
}

/// [`locations`], where with `size` a location is available only when one of its open nodes has
/// the memory and disk left for it, counted like a deploy counts them
/// (`Node::by_location_uuids_most_eligible`: suspended servers half, parked ones not at all).
pub async fn locations_for(
    state: &crate::State,
    user: &User,
    size: Option<Resources>,
) -> Result<Vec<ApiCreateLocation>, anyhow::Error> {
    let (memory, disk) = size.map_or((0, 0), |size| (size.memory.max(0), size.disk.max(0)));

    let rows = sqlx::query(
        r#"
        WITH server_usage AS (
            SELECT node_uuid,
                COALESCE(SUM((memory + memory_overhead) * CASE WHEN suspended THEN $1 ELSE 1.0 END), 0)::BIGINT AS used_memory,
                COALESCE(SUM(disk * CASE WHEN suspended THEN $1 ELSE 1.0 END), 0)::BIGINT AS used_disk
            FROM servers
            WHERE servers.limbo_state IS DISTINCT FROM 'parked'
            GROUP BY node_uuid
        )
        SELECT locations.uuid, locations.name, locations.description, locations.flag, locations.tier,
            COUNT(nodes.uuid) FILTER (
                WHERE nodes.deployment_enabled AND NOT nodes.maintenance_enabled
                AND NOT nodes.deployment_paused AND nodes.role = 'game'
            ) AS open_nodes,
            COUNT(nodes.uuid) FILTER (
                WHERE nodes.deployment_enabled AND NOT nodes.maintenance_enabled
                AND NOT nodes.deployment_paused AND nodes.role = 'game'
                AND (nodes.memory = 0 OR COALESCE(u.used_memory, 0) + $2 <= nodes.memory)
                AND (nodes.disk = 0 OR COALESCE(u.used_disk, 0) + $3 <= nodes.disk)
            ) AS roomy_nodes,
            MIN(telemetry.response_time_ms) FILTER (
                WHERE telemetry.reachable AND nodes.role = 'game'
            ) AS latency
        FROM locations
        LEFT JOIN nodes ON nodes.location_uuid = locations.uuid
        LEFT JOIN server_usage u ON u.node_uuid = nodes.uuid
        LEFT JOIN node_telemetry telemetry ON telemetry.node_uuid = nodes.uuid
        WHERE locations.public
        GROUP BY locations.uuid
        ORDER BY locations.sort, locations.name
        "#,
    )
    .bind(SUSPENSION_PENALTY)
    .bind(memory)
    .bind(disk)
    .fetch_all(state.database.read())
    .await?;

    let mut locations = Vec::with_capacity(rows.len());
    for row in rows {
        let tier: Option<compact_str::CompactString> = row
            .try_get::<Option<String>, _>("tier")?
            .filter(|tier| !tier.is_empty())
            .map(Into::into);

        locations.push(ApiCreateLocation {
            uuid: row.try_get("uuid")?,
            name: row.try_get::<String, _>("name")?.into(),
            description: row
                .try_get::<Option<String>, _>("description")?
                .map(Into::into),
            flag: row.try_get::<Option<String>, _>("flag")?.map(Into::into),
            unlocked: tier_unlocked(state, user, tier.as_deref()).await?,
            tier,
            available: availability(
                row.try_get("open_nodes")?,
                row.try_get("roomy_nodes")?,
                size.is_some(),
            ),
            full: row.try_get::<i64, _>("open_nodes")? == 0,
            latency: row.try_get("latency")?,
        });
    }

    Ok(locations)
}

/// Whether a location takes a new server: an open node, and with a size one with room for it.
fn availability(open_nodes: i64, roomy_nodes: i64, sized: bool) -> bool {
    if sized {
        roomy_nodes > 0
    } else {
        open_nodes > 0
    }
}

/// A server name as given: trimmed, not empty, at most [`NAME_MAX`] characters.
pub fn valid_name(name: &str) -> Option<&str> {
    let name = name.trim();

    (!name.is_empty() && name.chars().count() <= NAME_MAX).then_some(name)
}

/// A size of the player's own choosing: every value at least 1 (0 would be no limit), CPU within
/// what a server's limit holds.
fn custom_size(size: Option<Resources>) -> Result<Resources, CreateError> {
    match size {
        Some(size)
            if size.cpu >= 1
                && size.cpu <= i32::MAX as i64
                && size.memory >= 1
                && size.disk >= 1 =>
        {
            Ok(size)
        }
        _ => Err(CreateError::InvalidSize),
    }
}

/// The startup command and image of a variant: an egg's own startup command or image picked by
/// name, or one given in full; the egg's first without a variant.
pub fn runtime(
    egg: &NestEgg,
    variant: Option<&crate::settings::games::GameVariant>,
) -> Option<(compact_str::CompactString, compact_str::CompactString)> {
    Some((
        pick_named(
            variant.and_then(|v| v.startup.as_ref()),
            &egg.startup_commands,
        )?,
        pick_named(variant.and_then(|v| v.image.as_ref()), &egg.docker_images)?,
    ))
}

/// `wanted` by its name in `named`, else `wanted` itself; the first of `named` without one.
fn pick_named(
    wanted: Option<&compact_str::CompactString>,
    named: &indexmap::IndexMap<compact_str::CompactString, compact_str::CompactString>,
) -> Option<compact_str::CompactString> {
    match wanted {
        Some(wanted) => Some(named.get(wanted).unwrap_or(wanted).clone()),
        None => named.values().next().cloned(),
    }
}

/// Creates the server a player picked.
pub async fn create(
    state: &crate::State,
    user: &User,
    request: CreateRequest,
) -> Result<Server, CreateError> {
    let game = {
        let settings = state.settings.get().await?;
        if !settings.games.create_enabled {
            return Err(CreateError::Disabled);
        }

        settings.games.game(request.egg_uuid).cloned()
    };
    if user.suspended || user.frozen {
        return Err(CreateError::AccountBlocked);
    }
    if valid_name(&request.name).is_none() {
        return Err(CreateError::InvalidName);
    }

    let Some(_lock) = crate::allowance::lock(state, user.uuid).await else {
        return Err(CreateError::Busy);
    };

    create_locked(state, user, request, game).await
}

async fn create_locked(
    state: &crate::State,
    user: &User,
    request: CreateRequest,
    game: Option<Game>,
) -> Result<Server, CreateError> {
    let game = game.ok_or(CreateError::UnknownGame)?;
    let egg = NestEgg::by_uuid_optional(&state.database, game.egg_uuid)
        .await?
        .ok_or(CreateError::UnknownGame)?;

    if egg.features.iter().any(|feature| feature == EULA_FEATURE) && !request.eula {
        return Err(CreateError::EulaRequired);
    }
    // a Minecraft server starts once its player went through the setup, which applies the answers
    let setup = crate::minecraft::is_minecraft(&egg);

    let variant = match (&request.variant, game.variants.is_empty()) {
        (_, true) => None,
        (Some(value), false) => Some(
            game.variant(Some(value.as_str()))
                .ok_or(CreateError::UnknownVariant)?,
        ),
        (None, false) => game.variant(None),
    };
    let (startup, image) = runtime(&egg, variant).ok_or(CreateError::UnknownGame)?;

    let location = Location::by_uuid_optional(&state.database, request.location_uuid)
        .await?
        .filter(|location| location.public)
        .ok_or(CreateError::UnknownLocation)?;
    if !tier_unlocked(state, user, location.tier.as_deref()).await? {
        return Err(CreateError::LocationLocked);
    }

    // a plan only fills in the size; without one the player's own values, within the allowance
    let size = match request.plan_uuid {
        Some(plan_uuid) => {
            let plan = Plan::by_uuid_optional(&state.database, plan_uuid)
                .await?
                .filter(|plan| plan.public)
                .ok_or(CreateError::UnknownPlan)?;

            Resources::new(plan.cpu as i64, plan.memory, plan.disk)
        }
        None => custom_size(request.size)?,
    };

    let allowance = crate::allowance::for_user(state, user.uuid).await?;
    allowance.check_new(size).map_err(CreateError::Allowance)?;

    let limits = AdminApiServerLimits {
        cpu: i32::try_from(size.cpu).map_err(|_| CreateError::InvalidSize)?,
        memory: size.memory,
        memory_overhead: 0,
        swap: 0,
        disk: size.disk,
        io_weight: None,
        bandwidth: None,
    };

    let (limit_allocations, limit_databases, limit_backups, limit_schedules) = {
        let settings = state.settings.get().await?;
        (
            settings.games.limit_allocations,
            settings.games.limit_databases,
            settings.games.limit_backups,
            settings.games.limit_schedules,
        )
    };

    // only nodes for players' servers, the same weighting as an admin deploy's defaults
    let nodes = Node::by_location_uuids_most_eligible(
        &state.database,
        &[location.uuid],
        limits,
        false,
        SUSPENSION_PENALTY,
        RANDOMNESS,
    )
    .await?
    .into_iter()
    .filter(|node| node.role == GAME_ROLE)
    .collect::<Vec<_>>();

    let egg_configurations = EggConfiguration::all_cached(&state.database).await?;
    let variables = NestEggVariable::all_by_egg_uuid(&state.database, egg.uuid).await?;
    // txAdmin's or Multi Theft Auto's web port: a second port right after the first
    let second_port = crate::allocations::second_port_variable(
        crate::fivem::is_fivem(&egg),
        variables
            .iter()
            .map(|variable| variable.env_variable.as_str()),
    );

    // a port another create or move claimed between the pick and this claim: pick again
    let mut attempt = 0;
    let server = loop {
        attempt += 1;

        let (node_uuid, allocation_uuid, allocation_uuids, deployment_variables) =
            match place(state, &nodes, &egg, &egg_configurations, second_port).await? {
                Ok(placed) => placed,
                Err(allocation_error) => {
                    let reason = match Node::find_deployment_blocker(
                        &state.database,
                        &[location.uuid],
                        limits,
                        false,
                        SUSPENSION_PENALTY,
                    )
                    .await?
                    {
                        Some(blocker) => blocker.message().to_string(),
                        None => allocation_error.unwrap_or_else(|| "no eligible node".to_string()),
                    };

                    return Err(CreateError::NoNode(reason));
                }
            };

        let mut server_variables = HashMap::new();
        let mut set_variable = |env: &str, value: compact_str::CompactString| {
            if let Some(variable) = variables.iter().find(|v| v.env_variable == env) {
                server_variables.insert(variable.uuid, value);
            }
        };
        if let (Some(variable), Some(variant)) = (&game.variable, variant) {
            set_variable(variable, variant.value.clone());
        }
        for (name, value) in deployment_variables {
            set_variable(&name, value);
        }

        let feature_limits: ApiServerFeatureLimits = serde_json::from_value(serde_json::json!({
            "allocations": limit_allocations.max(1 + allocation_uuids.len() as i32),
            "databases": limit_databases,
            "backups": limit_backups,
            "schedules": limit_schedules,
        }))?;

        match Server::create(
            state,
            CreateServerOptions {
                node_uuid,
                owner_uuid: user.uuid,
                egg_uuid: egg.uuid,
                backup_configuration_uuid: None,
                allocation_uuid,
                allocation_uuids,
                start_on_completion: !setup,
                skip_installer: false,
                external_id: None,
                name: request.name.trim().into(),
                description: None,
                limits,
                pinned_cpus: Vec::new(),
                startup: startup.clone(),
                image: image.clone(),
                labels: Default::default(),
                timezone: None,
                trash: None,
                hugepages_passthrough_enabled: false,
                kvm_passthrough_enabled: false,
                feature_limits,
                variables: server_variables,
            },
        )
        .await
        {
            Ok(server) => break server,
            Err(err)
                if attempt < CLAIM_ATTEMPTS
                    && err.is_unique_constraint_violation(
                        "server_allocations_allocation_uuid_idx",
                    ) =>
            {
                tracing::info!(
                    attempt,
                    "a port of a new server was taken meanwhile, picking again"
                );
                continue;
            }
            Err(err) => return Err(err.into()),
        }
    };

    if request.eula {
        state
            .cache
            .set(&eula_marker(server.uuid), EULA_MARKER_SECONDS, &true)
            .await
            .ok();
    }
    if setup && let Err(err) = crate::minecraft::onboarding::require(state, server.uuid).await {
        // without the mark the server is just not started: its player starts it themselves
        tracing::warn!(server = %server.uuid, "failed to mark the server for setup: {err:#}");
    }

    Ok(server)
}

/// A node for a new server, its allocations and the variables they go into: the first of `nodes`
/// that has them. Its egg configuration's deployment when it has one, else ports inside the
/// deployment port ranges, made when missing, the second one for `second_port`
/// (docs/features/allocations.md). `Err` with why none had room.
async fn place(
    state: &crate::State,
    nodes: &[Node],
    egg: &NestEgg,
    egg_configurations: &[EggConfiguration],
    second_port: Option<&'static str>,
) -> Result<Result<Placed, Option<String>>, CreateError> {
    let mut allocation_error = None;
    for node in nodes {
        let deployment =
            EggConfiguration::merge(egg_configurations, &EggConfigurationTarget::new(egg, node))
                .config_allocations
                .map(|allocations| allocations.deployment)
                .filter(|deployment| deployment.primary.is_some());

        let Some(deployment) = deployment else {
            let count = 1 + usize::from(second_port.is_some());
            match crate::allocations::deploy(state, node, count).await {
                Ok(Some(picked)) => {
                    let variables = match (second_port, picked.get(1)) {
                        (Some(name), Some((_, port))) => {
                            vec![(name.into(), compact_str::format_compact!("{port}"))]
                        }
                        _ => Vec::new(),
                    };

                    return Ok(Ok((
                        node.uuid,
                        picked.first().map(|(uuid, _)| *uuid),
                        picked.iter().skip(1).map(|(uuid, _)| *uuid).collect(),
                        variables,
                    )));
                }
                Ok(None) => {
                    allocation_error = Some("not enough free allocations".to_string());
                }
                Err(crate::allocations::PortError::Unreachable(_)) => {
                    allocation_error =
                        Some("the node did not say which ports are free".to_string());
                }
                Err(err) => {
                    tracing::warn!(node = %node.uuid, "failed to pick ports for a new server: {err:?}");
                    allocation_error = Some("not enough free allocations".to_string());
                }
            }
            continue;
        };

        let mut deployment_variables = HashMap::new();
        let (primary, additional) = match NodeAllocation::get_from_deployment(
            state,
            &deployment,
            node,
            &mut deployment_variables,
        )
        .await
        {
            Ok(result) => result,
            Err(crate::database::DatabaseError::Any(err)) => {
                allocation_error = Some(err.to_string());
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        if primary.is_none() || additional.len() != deployment.additional.len() {
            allocation_error = Some("not enough free allocations".to_string());
            continue;
        }

        let deployment_variables = deployment_variables
            .into_iter()
            .map(|(name, value)| (compact_str::CompactString::from(name), value))
            .collect::<Vec<_>>();

        return Ok(Ok((node.uuid, primary, additional, deployment_variables)));
    }

    Ok(Err(allocation_error))
}

/// A node, its primary allocation, the others and the variables they go into.
type Placed = (
    uuid::Uuid,
    Option<uuid::Uuid>,
    Vec<uuid::Uuid>,
    Vec<(compact_str::CompactString, compact_str::CompactString)>,
);

fn eula_marker(server_uuid: uuid::Uuid) -> String {
    format!("server_creation::eula::{server_uuid}")
}

/// After a server's install (the install callback, before Wings starts it): writes `eula.txt` for
/// a player who accepted the EULA when creating it.
pub async fn after_install(state: &crate::State, server: &Server, successful: bool) {
    let marker = eula_marker(server.uuid);
    if !successful || !state.cache.exists(&marker).await.unwrap_or(false) {
        return;
    }

    let written = async {
        server
            .node
            .fetch_cached(&state.database)
            .await?
            .api_client(&state.database)
            .await?
            .post_servers_server_files_write(
                server.uuid,
                wings_api::client::AsyncRequestReader::new(std::io::Cursor::new(
                    b"eula=true\n".to_vec(),
                )),
                &wings_api::servers_server_files_write::post::Query {
                    file: Some("eula.txt".into()),
                    ..Default::default()
                },
            )
            .await?;

        Ok::<_, anyhow::Error>(())
    }
    .await;

    match written {
        Ok(()) => {
            state.cache.invalidate(&marker).await.ok();
        }
        Err(err) => {
            tracing::warn!(server = %server.uuid, "failed to write eula.txt: {err:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_pick_by_name_or_take_the_value() {
        let named = indexmap::IndexMap::from([
            ("Java 21".into(), "ghcr.io/example/java:21".into()),
            ("Java 17".into(), "ghcr.io/example/java:17".into()),
        ]);

        assert_eq!(
            pick_named(None, &named).as_deref(),
            Some("ghcr.io/example/java:21")
        );
        assert_eq!(
            pick_named(Some(&"Java 17".into()), &named).as_deref(),
            Some("ghcr.io/example/java:17")
        );
        assert_eq!(
            pick_named(Some(&"ghcr.io/example/other:1".into()), &named).as_deref(),
            Some("ghcr.io/example/other:1")
        );
        assert_eq!(pick_named(None, &indexmap::IndexMap::new()), None);
    }

    #[test]
    fn errors_have_a_status_and_a_message() {
        assert_eq!(CreateError::Busy.status(), 429);
        assert_eq!(CreateError::Allowance(Refusal::Memory).status(), 409);
        assert_eq!(CreateError::UnknownPlan.status(), 400);
        assert_eq!(CreateError::InvalidSize.status(), 400);
        assert_eq!(CreateError::InvalidName.status(), 400);
        assert!(CreateError::NoNode("x".into()).message().ends_with(": x"));
        assert!(CreateError::InvalidName.message().contains("191"));
    }

    #[test]
    fn names_are_trimmed_and_capped() {
        assert_eq!(valid_name("  lovebird "), Some("lovebird"));
        assert_eq!(valid_name("x"), Some("x"));
        assert_eq!(valid_name("   "), None);
        assert_eq!(valid_name(""), None);
        assert!(valid_name(&"a".repeat(NAME_MAX)).is_some());
        assert!(valid_name(&"a".repeat(NAME_MAX + 1)).is_none());
        // characters, not bytes
        assert!(valid_name(&"ä".repeat(NAME_MAX)).is_some());
    }

    #[test]
    fn a_custom_size_needs_every_value() {
        assert_eq!(
            custom_size(Some(Resources::new(100, 2048, 10240))).unwrap(),
            Resources::new(100, 2048, 10240)
        );
        assert!(custom_size(None).is_err());
        assert!(custom_size(Some(Resources::new(0, 2048, 10240))).is_err());
        assert!(custom_size(Some(Resources::new(100, 0, 10240))).is_err());
        assert!(custom_size(Some(Resources::new(100, 2048, 0))).is_err());
        assert!(custom_size(Some(Resources::new(-1, 2048, 10240))).is_err());
        assert!(custom_size(Some(Resources::new(i32::MAX as i64 + 1, 2048, 10240))).is_err());
    }

    #[test]
    fn a_size_needs_a_node_with_room() {
        // without a size: any open node
        assert!(availability(1, 0, false));
        assert!(!availability(0, 0, false));
        // with one: a node with room for it
        assert!(availability(2, 1, true));
        assert!(!availability(2, 0, true));
        assert!(!availability(0, 0, true));
    }
}
