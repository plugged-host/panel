//! plugged.host: paid servers on dedicated plans: CPU threads and memory of their own on a node
//! kept for them (docs/features/dedicated.md).
//!
//! A dedicated plan's `cpu` is whole threads (100 each). Its server goes on a node with the
//! `dedicated` role that the plan allows, is pinned to threads no other server of that node is
//! pinned to (whole cores first: a thread and its sibling), and only goes where the node's memory
//! is not handed out yet: the servers of a dedicated node never add up to more than its memory.
//! Disk is shared like on any node.

use crate::models::{node::Node, plan::Plan};
use sqlx::Row;
use std::collections::BTreeSet;

/// The node role for dedicated plans: never picked for anything else (players' free servers,
/// moves, limbo restores and failovers only go to `game` nodes).
pub const NODE_ROLE: &str = "dedicated";

/// Threads at the start of a node left to the system (Wings, Docker, the kernel's own work).
pub const SYSTEM_THREADS: usize = 1;

#[derive(Debug)]
pub enum DedicatedError {
    /// The plan is not a dedicated plan.
    NotDedicated,
    /// The node is not a dedicated node, or not one the plan allows.
    NodeNotAllowed,
    /// The node has not said how many threads it has yet (no telemetry sample).
    UnknownThreads,
    /// The node has no free threads for the plan.
    NoThreads,
    /// The node's memory is handed out already.
    NoMemory,
    Other(anyhow::Error),
}

impl DedicatedError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::NotDedicated => "the plan is not a dedicated plan",
            Self::NodeNotAllowed => "the node is not a dedicated node of the plan",
            Self::UnknownThreads => "the node has not reported its CPU threads yet",
            Self::NoThreads => "the node has no free CPU threads for the plan",
            Self::NoMemory => "the node has no free memory for the plan",
            Self::Other(_) => "the node could not be checked",
        }
    }
}

impl From<sqlx::Error> for DedicatedError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

/// The threads a plan's `cpu` stands for (100 each).
#[inline]
pub fn threads(cpu: i32) -> usize {
    (cpu.max(0) / 100) as usize
}

/// Whether `cpu` is a valid size for a dedicated plan: at least one whole thread.
#[inline]
pub fn valid_cpu(cpu: i32) -> bool {
    cpu >= 100 && cpu % 100 == 0
}

/// Whether the plan may put its server on `node`.
pub fn allows(plan: &Plan, node: &Node) -> bool {
    node.role == NODE_ROLE
        && (plan.node_uuids.is_empty() || plan.node_uuids.contains(&node.uuid))
        && (plan.location_uuids.is_empty() || plan.location_uuids.contains(&node.location.uuid))
}

/// Picks `wanted` threads of a node with `total` threads that are not in `used`, leaving the first
/// [`SYSTEM_THREADS`] alone (on a node with more than two threads). Whole cores first: Linux
/// numbers a core's second thread `total / 2` after its first, so `i` and `i + total / 2` go
/// together; single threads only once no whole core is left. `None` when there are not enough.
pub fn pick_threads(total: usize, used: &BTreeSet<i16>, wanted: usize) -> Option<Vec<i16>> {
    if wanted == 0 {
        return Some(Vec::new());
    }

    let reserved = if total > 2 { SYSTEM_THREADS } else { 0 };
    let free = |thread: usize| thread >= reserved && !used.contains(&(thread as i16));
    let mut picked = Vec::with_capacity(wanted);

    // whole cores (a thread and its sibling), when the threads come in pairs
    if total >= 2 && total.is_multiple_of(2) {
        let half = total / 2;
        for first in 0..half {
            if picked.len() + 2 > wanted {
                break;
            }
            let sibling = first + half;
            if free(first) && free(sibling) {
                picked.push(first as i16);
                picked.push(sibling as i16);
            }
        }
    }

    // then single threads
    for thread in 0..total {
        if picked.len() == wanted {
            break;
        }
        if free(thread) && !picked.contains(&(thread as i16)) {
            picked.push(thread as i16);
        }
    }

    if picked.len() < wanted {
        return None;
    }

    picked.sort_unstable();
    Some(picked)
}

/// Holds the node's dedicated placement while one server is put on it, so two at once never get
/// the same threads. Kept until the server exists.
pub async fn lock(
    state: &crate::State,
    node_uuid: uuid::Uuid,
) -> Result<crate::cache::CacheLock, anyhow::Error> {
    state
        .cache
        .lock(format!("dedicated::{node_uuid}"), Some(60), Some(15_000))
        .await
}

/// The threads a new server of `plan` gets on `node`, after checking the node is one of the plan's
/// and has the memory. Call it under [`lock`].
pub async fn reserve(
    state: &crate::State,
    plan: &Plan,
    node: &Node,
) -> Result<Vec<i16>, DedicatedError> {
    if !plan.dedicated || !valid_cpu(plan.cpu) {
        return Err(DedicatedError::NotDedicated);
    }
    if !allows(plan, node) {
        return Err(DedicatedError::NodeNotAllowed);
    }

    let total: Option<i32> =
        sqlx::query_scalar("SELECT cpu_threads FROM node_telemetry WHERE node_uuid = $1")
            .bind(node.uuid)
            .fetch_optional(state.database.read())
            .await?
            .flatten();
    let total = total
        .filter(|total| *total > 0)
        .ok_or(DedicatedError::UnknownThreads)? as usize;

    // the servers on the node or on their way to it
    let rows = sqlx::query(
        "SELECT pinned_cpus, memory + memory_overhead AS memory
        FROM servers
        WHERE node_uuid = $1 OR destination_node_uuid = $1",
    )
    .bind(node.uuid)
    .fetch_all(state.database.read())
    .await?;

    let mut used = BTreeSet::new();
    let mut memory = 0i64;
    for row in rows {
        used.extend(row.try_get::<Vec<i16>, _>("pinned_cpus")?);
        memory += row.try_get::<i64, _>("memory")?;
    }

    if memory + plan.memory > node.memory {
        return Err(DedicatedError::NoMemory);
    }

    pick_threads(total, &used, threads(plan.cpu)).ok_or(DedicatedError::NoThreads)
}

/// What a dedicated node still has: free threads (beyond the system's) and memory not handed out.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct Room {
    pub threads_total: Option<i32>,
    pub threads_free: i32,
    pub memory_free: i64,
}

/// [`Room`] of every dedicated node, for Admin › Plans and Admin › Servers.
pub async fn room(
    state: &crate::State,
) -> Result<std::collections::HashMap<uuid::Uuid, Room>, anyhow::Error> {
    let rows = sqlx::query(
        "SELECT nodes.uuid, nodes.memory, node_telemetry.cpu_threads,
            COALESCE((SELECT SUM(servers.memory + servers.memory_overhead) FROM servers
                WHERE servers.node_uuid = nodes.uuid OR servers.destination_node_uuid = nodes.uuid), 0)::BIGINT
                AS memory_used,
            COALESCE((SELECT COUNT(DISTINCT pinned) FROM servers, UNNEST(servers.pinned_cpus) AS pinned
                WHERE servers.node_uuid = nodes.uuid OR servers.destination_node_uuid = nodes.uuid), 0)::BIGINT
                AS threads_used
        FROM nodes
        LEFT JOIN node_telemetry ON node_telemetry.node_uuid = nodes.uuid
        WHERE nodes.role = $1",
    )
    .bind(NODE_ROLE)
    .fetch_all(state.database.read())
    .await?;

    let mut room = std::collections::HashMap::new();
    for row in rows {
        let total: Option<i32> = row.try_get("cpu_threads")?;
        let used: i64 = row.try_get("threads_used")?;
        let reserved = match total {
            Some(total) if total > 2 => SYSTEM_THREADS as i64,
            _ => 0,
        };
        room.insert(
            row.try_get("uuid")?,
            Room {
                threads_total: total,
                threads_free: total
                    .map_or(0, |total| (total as i64 - reserved - used).max(0) as i32),
                memory_free: (row.try_get::<i64, _>("memory")?
                    - row.try_get::<i64, _>("memory_used")?)
                .max(0),
            },
        );
    }

    Ok(room)
}

/// A location a dedicated plan's servers can go to.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ApiDedicatedLocation {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub flag: Option<compact_str::CompactString>,
    /// A node of the plan there has the threads and memory for one more server now.
    pub available: bool,
}

/// A public dedicated plan as players see it: ordered through a support ticket, set up by an admin.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ApiDedicatedPlan {
    pub uuid: uuid::Uuid,
    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,
    pub threads: i32,
    pub memory: i64,
    pub disk: i64,
    /// Cents a month.
    pub price: Option<i32>,
    pub locations: Vec<ApiDedicatedLocation>,
}

/// The public dedicated plans with where they can go and whether there is room.
pub async fn offers(state: &crate::State) -> Result<Vec<ApiDedicatedPlan>, anyhow::Error> {
    let plans = Plan::all_public_dedicated(&state.database).await?;
    if plans.is_empty() {
        return Ok(Vec::new());
    }

    let room = room(state).await?;
    let nodes = sqlx::query(
        "SELECT nodes.uuid AS node_uuid, locations.uuid, locations.name, locations.flag
        FROM nodes
        JOIN locations ON locations.uuid = nodes.location_uuid
        WHERE nodes.role = $1
        ORDER BY locations.sort, locations.name",
    )
    .bind(NODE_ROLE)
    .fetch_all(state.database.read())
    .await?;

    let mut offers = Vec::with_capacity(plans.len());
    for plan in plans {
        let mut locations: Vec<ApiDedicatedLocation> = Vec::new();
        for row in &nodes {
            let node_uuid: uuid::Uuid = row.try_get("node_uuid")?;
            let location_uuid: uuid::Uuid = row.try_get("uuid")?;
            if !(plan.node_uuids.is_empty() || plan.node_uuids.contains(&node_uuid))
                || !(plan.location_uuids.is_empty() || plan.location_uuids.contains(&location_uuid))
            {
                continue;
            }

            let fits = room.get(&node_uuid).is_some_and(|room| {
                room.threads_free as usize >= threads(plan.cpu) && room.memory_free >= plan.memory
            });
            match locations
                .iter_mut()
                .find(|location| location.uuid == location_uuid)
            {
                Some(location) => location.available |= fits,
                None => locations.push(ApiDedicatedLocation {
                    uuid: location_uuid,
                    name: row.try_get("name")?,
                    flag: row
                        .try_get::<Option<String>, _>("flag")?
                        .map(|flag| flag.trim().into()),
                    available: fits,
                }),
            }
        }

        offers.push(ApiDedicatedPlan {
            uuid: plan.uuid,
            name: plan.name,
            description: plan.description,
            threads: threads(plan.cpu) as i32,
            memory: plan.memory,
            disk: plan.disk,
            price: plan.price,
            locations,
        });
    }

    Ok(offers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(threads: &[i16]) -> BTreeSet<i16> {
        threads.iter().copied().collect()
    }

    #[test]
    fn whole_cores_come_first() {
        // 8 threads: cores are (0,4) (1,5) (2,6) (3,7); thread 0 is the system's
        assert_eq!(pick_threads(8, &used(&[]), 2), Some(vec![1, 5]));
        assert_eq!(pick_threads(8, &used(&[]), 4), Some(vec![1, 2, 5, 6]));
    }

    #[test]
    fn the_system_thread_is_never_picked() {
        let picked = pick_threads(8, &used(&[]), 7).unwrap();
        assert!(!picked.contains(&0));
        assert_eq!(picked.len(), 7);
        assert_eq!(pick_threads(8, &used(&[]), 8), None);
    }

    #[test]
    fn used_threads_are_skipped() {
        assert_eq!(pick_threads(8, &used(&[1, 5]), 2), Some(vec![2, 6]));
        // no whole core left: single threads
        assert_eq!(
            pick_threads(8, &used(&[1, 2, 3, 6, 7]), 2),
            Some(vec![4, 5])
        );
        assert_eq!(pick_threads(8, &used(&[1, 2, 3, 5, 6, 7]), 2), None);
    }

    #[test]
    fn an_odd_count_takes_a_core_and_a_thread() {
        assert_eq!(pick_threads(8, &used(&[]), 3), Some(vec![1, 2, 5]));
    }

    #[test]
    fn small_nodes_keep_no_thread_back() {
        assert_eq!(pick_threads(2, &used(&[]), 2), Some(vec![0, 1]));
        assert_eq!(pick_threads(1, &used(&[]), 1), Some(vec![0]));
    }

    #[test]
    fn zero_threads_is_nothing() {
        assert_eq!(pick_threads(8, &used(&[]), 0), Some(Vec::new()));
    }

    #[test]
    fn dedicated_cpu_is_whole_threads() {
        assert!(valid_cpu(100));
        assert!(valid_cpu(400));
        assert!(!valid_cpu(0));
        assert!(!valid_cpu(150));
        assert_eq!(threads(400), 4);
    }
}
