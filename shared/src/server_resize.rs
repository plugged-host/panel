//! plugged.host: owners changing their servers' CPU, memory and disk within their allowance
//! (docs/features/server-settings.md).

use crate::{
    allowance::{Allowance, Refusal, Resources},
    models::{
        ByUuid, UpdatableModel,
        plan::Plan,
        server::{AdminApiServerLimits, Server, UpdateServerOptions},
    },
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// The most each resource may be set to; `None` where there is no limit.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub cpu: Option<i64>,
    pub memory: Option<i64>,
    pub disk: Option<i64>,
}

/// One of the plans players create servers with (Admin › Plans), as a size to resize to.
#[derive(ToSchema, Serialize)]
pub struct ApiResizePlan {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub cpu: i64,
    pub memory: i64,
    pub disk: i64,
    /// The server may be resized to it: it fits in the server's size plus what its owner has left.
    pub fits: bool,
    /// The server has exactly this size now.
    pub current: bool,
}

#[derive(ToSchema, Serialize)]
pub struct ApiResize {
    /// Resizing is on (Admin › Settings › Hosting › Games).
    pub enabled: bool,
    pub current: Resources,
    pub max: Bounds,
    /// The public plans, in their order; values of their own stay possible ("Advanced").
    pub plans: Vec<ApiResizePlan>,
}

#[derive(Debug)]
pub enum ResizeError {
    Disabled,
    Busy,
    /// A value of 0 (no limit) that the server did not have, or a negative one.
    Invalid,
    Allowance(Refusal),
    /// The node has no room for the bigger server (`memory` or `disk`).
    NodeFull(&'static str),
    Other(anyhow::Error),
}

impl ResizeError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "resizing servers is turned off".into(),
            Self::Busy => "another change to your servers is still running, try again".into(),
            Self::Invalid => "cpu, memory and disk must be at least 1".into(),
            Self::Allowance(refusal) => refusal.message().into(),
            Self::NodeFull(resource) => {
                format!("the server's node has no {resource} left for this, move the server first")
            }
            Self::Other(err) => err.to_string(),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled => 403,
            Self::Busy => 429,
            Self::Invalid => 400,
            Self::Allowance(_) | Self::NodeFull(_) => 409,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for ResizeError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for ResizeError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl From<sqlx::Error> for ResizeError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

pub fn current(server: &Server) -> Resources {
    Resources::new(server.cpu as i64, server.memory, server.disk)
}

/// The server's own size plus what its owner has left. Without a plan it can only shrink.
pub fn bounds(allowance: &Allowance, current: Resources) -> Bounds {
    if !allowance.has_plan {
        return Bounds {
            cpu: Some(current.cpu),
            memory: Some(current.memory),
            disk: Some(current.disk),
        };
    }

    let (cpu, memory, disk) = allowance.remaining();

    Bounds {
        cpu: cpu.map(|left| current.cpu + left),
        memory: memory.map(|left| current.memory + left),
        disk: disk.map(|left| current.disk + left),
    }
}

/// Every value at least 1; 0 (no limit) only where the server already has it.
pub fn valid(from: Resources, to: Resources) -> bool {
    let ok = |from: i64, to: i64| to >= 1 || (to == 0 && from == 0);

    ok(from.cpu, to.cpu) && ok(from.memory, to.memory) && ok(from.disk, to.disk)
}

/// Whether a server of size `current` may become `plan`: what [`resize`] checks against the
/// owner's allowance, without the node (a full node answers when the resize is sent).
pub fn plan_fits(allowance: &Allowance, current: Resources, plan: Resources) -> bool {
    valid(current, plan) && allowance.check_change(current, plan).is_ok()
}

/// Whether the server's node has room for it to grow: allocated memory (with overhead) and disk
/// against the node's, like a deploy. Shrinking always fits.
pub fn node_fits(
    node: Resources,
    allocated: Resources,
    from: Resources,
    to: Resources,
) -> Result<(), &'static str> {
    let fits = |limit: i64, used: i64, from: i64, to: i64| {
        limit <= 0 || to <= from || used - from + to <= limit
    };

    if !fits(node.memory, allocated.memory, from.memory, to.memory) {
        return Err("memory");
    }
    if !fits(node.disk, allocated.disk, from.disk, to.disk) {
        return Err("disk");
    }

    Ok(())
}

/// What the server may be resized to now.
pub async fn options(state: &crate::State, server: &Server) -> Result<ApiResize, anyhow::Error> {
    let enabled = state.settings.get().await?.games.allow_resizing;
    let current = current(server);
    let allowance = crate::allowance::for_user(state, server.owner.uuid).await?;

    let plans = Plan::all_public(&state.database)
        .await?
        .into_iter()
        .map(|plan| {
            let size = Resources::new(plan.cpu as i64, plan.memory, plan.disk);

            ApiResizePlan {
                uuid: plan.uuid,
                name: plan.name,
                cpu: size.cpu,
                memory: size.memory,
                disk: size.disk,
                fits: plan_fits(&allowance, current, size),
                current: size == current,
            }
        })
        .collect();

    Ok(ApiResize {
        enabled,
        current,
        max: bounds(&allowance, current),
        plans,
    })
}

/// Resizes the server and pushes the new limits to Wings, which applies them live. `admin` skips
/// the setting (an admin can resize anything in Admin › Servers anyway), never the allowance.
pub async fn resize(
    state: &crate::State,
    server: &mut Server,
    to: Resources,
    admin: bool,
) -> Result<(), ResizeError> {
    if !admin && !state.settings.get().await?.games.allow_resizing {
        return Err(ResizeError::Disabled);
    }

    let from = current(server);
    if !valid(from, to) {
        return Err(ResizeError::Invalid);
    }

    let Some(_lock) = crate::allowance::lock(state, server.owner.uuid).await else {
        return Err(ResizeError::Busy);
    };

    // the row may have changed while waiting for the lock
    let Some(fresh) = Server::by_uuid_optional(&state.database, server.uuid).await? else {
        return Err(ResizeError::Other(anyhow::anyhow!("server not found")));
    };
    *server = fresh;
    let from = current(server);

    crate::allowance::for_user(state, server.owner.uuid)
        .await?
        .check_change(from, to)
        .map_err(ResizeError::Allowance)?;

    let row = sqlx::query(
        r#"
        SELECT nodes.memory, nodes.disk,
            COALESCE(SUM(servers.memory + servers.memory_overhead), 0)::BIGINT AS used_memory,
            COALESCE(SUM(servers.disk), 0)::BIGINT AS used_disk
        FROM nodes
        LEFT JOIN servers ON servers.node_uuid = nodes.uuid
            AND servers.limbo_state IS DISTINCT FROM 'parked'
        WHERE nodes.uuid = $1
        GROUP BY nodes.uuid
        "#,
    )
    .bind(server.node.uuid)
    .fetch_one(state.database.write())
    .await?;

    node_fits(
        Resources::new(0, row.try_get("memory")?, row.try_get("disk")?),
        Resources::new(0, row.try_get("used_memory")?, row.try_get("used_disk")?),
        Resources::new(from.cpu, from.memory + server.memory_overhead, from.disk),
        Resources::new(to.cpu, to.memory + server.memory_overhead, to.disk),
    )
    .map_err(ResizeError::NodeFull)?;

    server
        .update(
            state,
            UpdateServerOptions {
                limits: Some(AdminApiServerLimits {
                    cpu: to.cpu.clamp(0, i32::MAX as i64) as i32,
                    memory: to.memory,
                    memory_overhead: server.memory_overhead,
                    swap: server.swap,
                    disk: to.disk,
                    io_weight: server.io_weight,
                    bandwidth: None,
                }),
                ..Default::default()
            },
        )
        .await?;

    let synced = server.clone();
    let state = state.clone();
    tokio::spawn(async move {
        let uuid = synced.uuid;
        if let Err(err) = synced.sync(&state).await {
            tracing::error!(server = %uuid, "failed to sync a resized server: {err:?}");
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allowance::AllowanceSource;

    fn allowance(has_plan: bool, limit: Resources, used: Resources) -> Allowance {
        Allowance {
            source: AllowanceSource::Settings,
            has_plan,
            limit,
            used,
            servers: 2,
            max_servers: 0,
        }
    }

    #[test]
    fn bounds_are_the_own_size_plus_what_is_left() {
        let current = Resources::new(100, 2048, 4096);
        let roomy = allowance(
            true,
            Resources::new(400, 0, 20480),
            Resources::new(300, 6144, 10240),
        );
        assert_eq!(
            bounds(&roomy, current),
            Bounds {
                cpu: Some(200),
                memory: None,
                disk: Some(14336),
            }
        );

        let none = allowance(false, Resources::default(), current);
        assert_eq!(
            bounds(&none, current),
            Bounds {
                cpu: Some(100),
                memory: Some(2048),
                disk: Some(4096),
            }
        );
    }

    #[test]
    fn no_limit_is_kept_never_taken() {
        let from = Resources::new(0, 2048, 4096);
        assert!(valid(from, Resources::new(0, 1024, 4096)));
        assert!(valid(from, Resources::new(100, 1024, 4096)));
        assert!(!valid(from, Resources::new(0, 0, 4096)));
        assert!(!valid(from, Resources::new(-5, 1024, 4096)));
    }

    #[test]
    fn plans_fit_within_the_own_size_plus_what_is_left() {
        let current = Resources::new(100, 2048, 4096);
        let roomy = allowance(
            true,
            Resources::new(400, 8192, 0),
            Resources::new(300, 6144, 10240),
        );

        // 100 + 100 CPU and 2048 + 2048 MiB left; disk has no limit
        assert!(plan_fits(
            &roomy,
            current,
            Resources::new(200, 4096, 99_999)
        ));
        assert!(!plan_fits(&roomy, current, Resources::new(201, 4096, 4096)));
        assert!(!plan_fits(&roomy, current, Resources::new(100, 4097, 4096)));
        assert!(plan_fits(&roomy, current, current));

        // without a plan the server can only shrink
        let none = allowance(false, Resources::default(), current);
        assert!(plan_fits(&none, current, Resources::new(50, 1024, 2048)));
        assert!(!plan_fits(&none, current, Resources::new(100, 3072, 4096)));

        // 0 (no limit) is never a plan's value to take
        assert!(!plan_fits(&roomy, current, Resources::new(0, 2048, 4096)));
    }

    #[test]
    fn a_full_node_refuses_growth_but_not_shrinking() {
        let node = Resources::new(0, 16384, 0);
        let allocated = Resources::new(0, 15360, 999_999);
        let from = Resources::new(100, 2048, 4096);

        assert_eq!(
            node_fits(node, allocated, from, Resources::new(100, 3072, 99_999)),
            Ok(())
        );
        assert_eq!(
            node_fits(node, allocated, from, Resources::new(100, 3073, 4096)),
            Err("memory")
        );
        assert_eq!(
            node_fits(
                Resources::new(0, 0, 8192),
                Resources::new(0, 0, 8192),
                from,
                Resources::new(100, 1024, 2048)
            ),
            Ok(())
        );
    }
}
