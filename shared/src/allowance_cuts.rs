//! plugged.host: bringing an account's servers down to fit after its allowance went down
//! (docs/features/allowance-cuts.md).
//!
//! A decrease is written to `user_allowance_cuts` where it happens (a smaller noprofit.space plan
//! in [`crate::noprofit::Link::store`], a smaller allowance in Admin › Settings › Games); the
//! `settle_allowance_cuts` task carries it out: what the account's servers use beyond the new
//! allowance, at most the decrease, comes off all its servers evenly, never below a floor on any
//! one of them. An account that was over before keeps that part; one with room to spare loses
//! nothing.

use crate::{
    allowance::Resources,
    models::{
        ByUuid, CreatableModel, UpdatableModel,
        server::{AdminApiServerLimits, Server, UpdateServerOptions},
        server_activity::{CreateServerActivityOptions, ServerActivity},
    },
};
use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;

/// The least CPU (%) a cut leaves on a server.
pub const MIN_CPU: i64 = 25;
/// The least memory (MiB) a cut leaves on a server.
pub const MIN_MEMORY: i64 = 1024;
/// The least disk (MiB) a cut leaves on a server.
pub const MIN_DISK: i64 = 1024;
/// Disk (MiB) a cut leaves free above what a server stores now: Wings stops a server over its
/// disk and will not start it again.
pub const DISK_HEADROOM: i64 = 1024;
/// A decrease from no limit to some: whatever is over.
pub const EVERYTHING_OVER: i64 = i64::MAX / 4;

pub const REASON_PLAN: &str = "plan";
pub const REASON_SETTINGS: &str = "settings";

/// How much a limit went down, per resource. 0 is no limit: going to none takes nothing, coming
/// from none takes whatever is over.
pub fn decrease(old: Resources, new: Resources) -> Resources {
    let one = |old: i64, new: i64| {
        if new <= 0 {
            0
        } else if old <= 0 {
            EVERYTHING_OVER
        } else {
            (old - new).max(0)
        }
    };

    Resources::new(
        one(old.cpu, new.cpu),
        one(old.memory, new.memory),
        one(old.disk, new.disk),
    )
}

fn any(resources: Resources) -> bool {
    resources.cpu > 0 || resources.memory > 0 || resources.disk > 0
}

/// What comes off: what is used beyond the limit, at most what was asked. Nothing without a limit.
pub fn take(asked: Resources, limit: Resources, used: Resources) -> Resources {
    let one = |asked: i64, limit: i64, used: i64| {
        if limit <= 0 {
            0
        } else {
            asked.min((used - limit).max(0))
        }
    };

    Resources::new(
        one(asked.cpu, limit.cpu, used.cpu),
        one(asked.memory, limit.memory, used.memory),
        one(asked.disk, limit.disk, used.disk),
    )
}

/// Takes `amount` off the values equally: two servers half each, five a fifth each. One that
/// would drop below `floor` gives what it can and the others share the rest; what none can give
/// stays untaken. Returns the cut per key, in the values' order, without the zero ones.
pub fn even_cuts<K: Copy + PartialEq>(
    values: &[(K, i64)],
    amount: i64,
    floor: i64,
) -> Vec<(K, i64)> {
    let mut cuts = vec![0i64; values.len()];
    let mut left = amount;

    while left > 0 {
        let room = values
            .iter()
            .enumerate()
            .filter_map(|(i, (_, value))| {
                let room = value - cuts[i] - floor;
                (room > 0).then_some((i, room))
            })
            .collect::<Vec<_>>();
        if room.is_empty() {
            break;
        }

        let share = (left + room.len() as i64 - 1) / room.len() as i64;
        for (i, room) in room {
            let cut = share.min(room).min(left);
            cuts[i] += cut;
            left -= cut;
        }
    }

    values
        .iter()
        .zip(cuts)
        .filter(|(_, cut)| *cut > 0)
        .map(|((key, _), cut)| (*key, cut))
        .collect()
}

/// Writes a decrease for the account's servers to give back; the task carries it out.
pub async fn record(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
    decrease: Resources,
    reason: &str,
) -> Result<(), sqlx::Error> {
    if !any(decrease) {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO user_allowance_cuts (user_uuid, reason, cpu, memory, disk)
        VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_uuid)
    .bind(reason)
    .bind(decrease.cpu)
    .bind(decrease.memory)
    .bind(decrease.disk)
    .execute(database.write())
    .await?;

    Ok(())
}

/// Writes a decrease for every account that owns a server (the allowance of the games settings
/// went down).
pub async fn record_for_everyone(
    database: &crate::database::Database,
    decrease: Resources,
    reason: &str,
) -> Result<(), sqlx::Error> {
    if !any(decrease) {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO user_allowance_cuts (user_uuid, reason, cpu, memory, disk)
        SELECT DISTINCT servers.owner_uuid, $1, $2, $3, $4 FROM servers",
    )
    .bind(reason)
    .bind(decrease.cpu)
    .bind(decrease.memory)
    .bind(decrease.disk)
    .execute(database.write())
    .await?;

    Ok(())
}

#[derive(Serialize)]
struct CutServer {
    uuid: uuid::Uuid,
    name: String,
    cpu: i64,
    memory: i64,
    disk: i64,
}

/// What each server can give of its disk: down to [`MIN_DISK`], never below what it stores plus
/// [`DISK_HEADROOM`]. A server whose node does not answer gives none.
async fn disk_room(state: &crate::State, servers: &[Server]) -> Vec<(uuid::Uuid, i64)> {
    let mut stored: HashMap<uuid::Uuid, u64> = HashMap::new();
    let mut nodes = servers
        .iter()
        .map(|server| server.node.uuid)
        .collect::<Vec<_>>();
    nodes.sort();
    nodes.dedup();

    for node_uuid in nodes {
        let Some(server) = servers.iter().find(|server| server.node.uuid == node_uuid) else {
            continue;
        };
        let usage = async {
            server
                .node
                .fetch_cached(&state.database)
                .await?
                .api_client(&state.database)
                .await?
                .get_servers_utilization()
                .await
                .map_err(anyhow::Error::from)
        }
        .await;

        match usage {
            Ok(usage) => {
                for (uuid, usage) in usage {
                    stored.insert(uuid, usage.disk_bytes);
                }
            }
            Err(err) => {
                tracing::warn!(node = %node_uuid, "allowance cut: no disk usage from the node: {err:#}");
            }
        }
    }

    servers
        .iter()
        .filter(|server| server.disk > 0)
        .filter_map(|server| {
            let stored = stored.get(&server.uuid)?.div_ceil(1024 * 1024) as i64;
            let room = server.disk - MIN_DISK.max(stored + DISK_HEADROOM);

            (room > 0).then_some((server.uuid, room))
        })
        .collect()
}

/// Carries out the account's open decreases together, under its create/resize lock. `false` when
/// the lock was busy (the task comes back).
pub async fn settle_user(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    let Some(_lock) = crate::allowance::lock(state, user_uuid).await else {
        return Ok(false);
    };

    let rows = sqlx::query(
        "SELECT uuid, reason, cpu, memory, disk FROM user_allowance_cuts
        WHERE user_uuid = $1 AND settled IS NULL
        ORDER BY created",
    )
    .bind(user_uuid)
    .fetch_all(state.database.write())
    .await?;
    if rows.is_empty() {
        return Ok(true);
    }

    let mut asked = Resources::default();
    let mut uuids = Vec::with_capacity(rows.len());
    let mut reasons = Vec::new();
    for row in &rows {
        uuids.push(row.try_get::<uuid::Uuid, _>("uuid")?);
        let reason: String = row.try_get("reason")?;
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
        asked.cpu = asked
            .cpu
            .saturating_add(row.try_get("cpu")?)
            .min(EVERYTHING_OVER);
        asked.memory = asked
            .memory
            .saturating_add(row.try_get("memory")?)
            .min(EVERYTHING_OVER);
        asked.disk = asked
            .disk
            .saturating_add(row.try_get("disk")?)
            .min(EVERYTHING_OVER);
    }

    let allowance = crate::allowance::for_user(state, user_uuid).await?;
    let taking = if allowance.has_plan {
        take(asked, allowance.limit, allowance.used)
    } else {
        Resources::default()
    };

    let servers = if any(taking) {
        let server_uuids: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT uuid FROM servers WHERE owner_uuid = $1 ORDER BY created, uuid",
        )
        .bind(user_uuid)
        .fetch_all(state.database.write())
        .await?;

        let mut servers = Vec::with_capacity(server_uuids.len());
        for uuid in server_uuids {
            if let Some(server) = Server::by_uuid_optional(&state.database, uuid).await? {
                servers.push(server);
            }
        }
        servers
    } else {
        Vec::new()
    };

    let values = |get: fn(&Server) -> i64| {
        servers
            .iter()
            .filter(|server| get(server) > 0)
            .map(|server| (server.uuid, get(server)))
            .collect::<Vec<_>>()
    };
    let cpu: HashMap<_, _> = even_cuts(&values(|server| server.cpu as i64), taking.cpu, MIN_CPU)
        .into_iter()
        .collect();
    let memory: HashMap<_, _> =
        even_cuts(&values(|server| server.memory), taking.memory, MIN_MEMORY)
            .into_iter()
            .collect();
    // each server's floor is already out of its room
    let disk: HashMap<_, _> = if taking.disk > 0 {
        even_cuts(&disk_room(state, &servers).await, taking.disk, 0)
            .into_iter()
            .collect()
    } else {
        HashMap::new()
    };

    let reason = reasons.join(",");
    let mut taken = Resources::default();
    let mut changed = Vec::new();
    for mut server in servers {
        let cut = Resources::new(
            cpu.get(&server.uuid).copied().unwrap_or(0),
            memory.get(&server.uuid).copied().unwrap_or(0),
            disk.get(&server.uuid).copied().unwrap_or(0),
        );
        if !any(cut) {
            continue;
        }

        server
            .update(
                state,
                UpdateServerOptions {
                    limits: Some(AdminApiServerLimits {
                        cpu: (server.cpu as i64 - cut.cpu).clamp(0, i32::MAX as i64) as i32,
                        memory: server.memory - cut.memory,
                        memory_overhead: server.memory_overhead,
                        swap: server.swap,
                        disk: server.disk - cut.disk,
                        io_weight: server.io_weight,
                        bandwidth: None,
                    }),
                    ..Default::default()
                },
            )
            .await?;

        taken.cpu += cut.cpu;
        taken.memory += cut.memory;
        taken.disk += cut.disk;
        changed.push(CutServer {
            uuid: server.uuid,
            name: server.name.to_string(),
            cpu: cut.cpu,
            memory: cut.memory,
            disk: cut.disk,
        });

        if let Err(err) = ServerActivity::create(
            state,
            CreateServerActivityOptions {
                server_uuid: server.uuid,
                user_uuid: None,
                impersonator_uuid: None,
                api_key_uuid: None,
                schedule_uuid: None,
                event: "server:allowance.cut".into(),
                ip: None,
                data: serde_json::json!({
                    "reason": reason,
                    "cpu": cut.cpu,
                    "memory": cut.memory,
                    "disk": cut.disk,
                }),
                created: None,
            },
        )
        .await
        {
            tracing::warn!(server = %server.uuid, "failed to log an allowance cut: {err:#?}");
        }

        // Wings applies the new limits to the running container
        let state = state.clone();
        tokio::spawn(async move {
            let uuid = server.uuid;
            if let Err(err) = server.sync(&state).await {
                tracing::warn!(server = %uuid, "allowance cut saved, Wings not updated live: {err:?}");
            }
        });
    }

    sqlx::query("UPDATE user_allowance_cuts SET settled = NOW(), result = $2 WHERE uuid = ANY($1)")
        .bind(&uuids)
        .bind(serde_json::json!({ "taken": taken, "servers": changed }))
        .execute(state.database.write())
        .await?;

    if !changed.is_empty() {
        tracing::info!(user = %user_uuid, servers = changed.len(), "brought servers down to a lowered allowance");
    }

    Ok(true)
}

/// The `settle_allowance_cuts` task: every account with open decreases, the longest waiting first.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let users: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT user_uuid FROM user_allowance_cuts WHERE settled IS NULL
        GROUP BY user_uuid ORDER BY MIN(created)",
    )
    .fetch_all(state.database.write())
    .await?;

    for user_uuid in users {
        if let Err(err) = settle_user(state, user_uuid).await {
            tracing::warn!(user = %user_uuid, "failed to carry out an allowance cut: {err:#}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decrease_counts_only_what_went_down() {
        assert_eq!(
            decrease(Resources::new(400, 8192, 0), Resources::new(200, 12288, 0)),
            Resources::new(200, 0, 0)
        );
        assert_eq!(
            decrease(
                Resources::new(0, 8192, 20480),
                Resources::new(300, 0, 10240)
            ),
            Resources::new(EVERYTHING_OVER, 0, 10240)
        );
    }

    #[test]
    fn only_what_is_over_comes_off_at_most_the_decrease() {
        let limit = Resources::new(200, 4096, 0);
        // over by 100 CPU and 2048 memory; asked 50 CPU and 4096 memory
        assert_eq!(
            take(
                Resources::new(50, 4096, 9999),
                limit,
                Resources::new(300, 6144, 50000)
            ),
            Resources::new(50, 2048, 0)
        );
        // room to spare: nothing
        assert_eq!(
            take(
                Resources::new(100, 100, 100),
                Resources::new(400, 8192, 0),
                Resources::new(100, 1024, 0)
            ),
            Resources::default()
        );
    }

    #[test]
    fn cuts_are_even_and_respect_the_floor() {
        assert_eq!(
            even_cuts(&[(1, 2048), (2, 2048)], 1024, 1024),
            vec![(1, 512), (2, 512)]
        );
        // one server near the floor gives what it can, the other the rest
        assert_eq!(
            even_cuts(&[(1, 1280), (2, 4096)], 1024, 1024),
            vec![(1, 256), (2, 768)]
        );
        // more than all can give: the floors stay
        assert_eq!(
            even_cuts(&[(1, 1100), (2, 1200)], 9999, 1024),
            vec![(1, 76), (2, 176)]
        );
        assert_eq!(even_cuts::<i32>(&[], 100, 0), vec![]);
    }
}
