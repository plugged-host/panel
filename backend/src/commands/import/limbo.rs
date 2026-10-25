// plugged.host: the Pelican panel's limbo in an import (docs/features/limbo.md#parked-on-a-node):
// the servers it parked by moving them to a node it keeps for them stay there, that node becomes a
// `limbo` node and they become parked servers (restored by a move off it, home on their old ports
// when free), and its idle stops carry over. Nothing is moved or copied at the import.

use super::{
    SourceContext, model::*, source_i64, source_optional_i64, source_optional_text, source_text,
    source_timestamp,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use shared::limbo::{LimboState, NODE_ROLE};
use std::collections::{HashMap, HashSet};

/// The Pelican limbo plugin's rows (`server_limbo_states`) and its node tag.
#[derive(Default)]
pub struct SourceLimbo {
    pub rows: Vec<SourceLimboRow>,
    /// `LIMBO_NODE_TAG` in lower case (`limbo` when unset): nodes with it are where it parks.
    pub node_tag: String,
}

/// One server limbo touched.
#[derive(Debug, Clone, Default)]
pub struct SourceLimboRow {
    pub server_id: i64,
    /// `stopped`, `hibernating`, `limbo`, `restoring`, `restored` or `failed`.
    pub state: String,
    pub stopped_at: Option<DateTime<Utc>>,
    pub limbo_at: Option<DateTime<Utc>>,
    pub origin_node_id: Option<i64>,
    pub origin_allocation_id: Option<i64>,
    pub origin_allocation_ids: Vec<i64>,
    /// The node it was parked on (the plugin's migration 021).
    pub limbo_node_id: Option<i64>,
    pub restore_cost: i64,
    pub updated: Option<DateTime<Utc>>,
}

/// The states in which a row's server is parked or on its way in or out
/// (`LimboService::UNSETTLED`).
const UNSETTLED: [&str; 3] = ["hibernating", "limbo", "restoring"];

/// The allocation ids in `origin_allocations` (a JSON list).
fn allocation_ids(raw: Option<&str>) -> Vec<i64> {
    raw.and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| match id {
            serde_json::Value::Number(id) => id.as_i64(),
            serde_json::Value::String(id) => id.trim().parse().ok(),
            _ => None,
        })
        .collect()
}

/// Reads `server_limbo_states`; a panel without the limbo plugin reads as empty.
pub(super) async fn read(context: &SourceContext) -> Result<SourceLimbo, anyhow::Error> {
    let node_tag = std::env::var("LIMBO_NODE_TAG")
        .ok()
        .map(|tag| tag.trim().trim_matches('"').to_lowercase())
        .filter(|tag| !tag.is_empty() && tag != "null")
        .unwrap_or_else(|| "limbo".into());

    if !context.has_table("server_limbo_states").await? {
        return Ok(SourceLimbo {
            rows: Vec::new(),
            node_tag,
        });
    }

    let rows = context
        .map_table("server_limbo_states", None, |table, row| {
            Ok(SourceLimboRow {
                server_id: source_i64(row, "server_id")?,
                state: source_text(row, "state")?.trim().to_lowercase(),
                stopped_at: source_timestamp(row, "stopped_at"),
                limbo_at: source_timestamp(row, "limbo_at"),
                origin_node_id: source_optional_i64(row, "origin_node_id")?,
                origin_allocation_id: source_optional_i64(row, "origin_allocation_id")?,
                origin_allocation_ids: allocation_ids(
                    source_optional_text(row, "origin_allocations")?.as_deref(),
                ),
                limbo_node_id: if table.has("limbo_node_id") {
                    source_optional_i64(row, "limbo_node_id")?
                } else {
                    None
                },
                restore_cost: if table.has("restore_cost") {
                    source_optional_i64(row, "restore_cost")?.unwrap_or(0)
                } else {
                    0
                },
                updated: source_timestamp(row, "updated_at"),
            })
        })
        .await?;

    Ok(SourceLimbo { rows, node_tag })
}

/// The nodes the Pelican panel parks servers on, or did (its `parkingNodeIds()`): the ones with its
/// limbo tag, the ones its rows record for servers parked or on their way, and any node a parked
/// server sits on away from the node it came from. `server_nodes` maps a server to its node.
pub fn parking_nodes(
    rows: &[SourceLimboRow],
    tagged: impl IntoIterator<Item = i64>,
    server_nodes: &HashMap<i64, i64>,
) -> HashSet<i64> {
    let mut nodes: HashSet<i64> = tagged.into_iter().collect();

    for row in rows {
        if UNSETTLED.contains(&row.state.as_str())
            && let Some(node_id) = row.limbo_node_id
        {
            nodes.insert(node_id);
        }
        if row.state == "limbo"
            && let Some(node_id) = server_nodes.get(&row.server_id)
            && row.origin_node_id != Some(*node_id)
        {
            nodes.insert(*node_id);
        }
    }

    nodes
}

/// What a server becomes, from its row, its node and whether it is suspended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Imported {
    /// On a node servers are parked on: `parked` there, not suspended (limbo suspended it), since
    /// it was parked.
    Parked { since: Option<DateTime<Utc>> },
    /// Stopped for being idle on its own node.
    Stopped { since: Option<DateTime<Utc>> },
    /// An ordinary server; `unsuspend` when limbo's suspension is left on it (a restore that
    /// landed, a parked server that is back on its own node), as the plugin's reconcile would.
    Ordinary { unsuspend: bool },
}

/// The Pelican panel's state of a server as Calagopus' (its `isParked()` and `reconcile()`): a
/// server is parked when it sits on a node of `parking` that is not the node it came from. A move
/// in flight at the import counts by where the server is: a move to the limbo node that has not
/// landed leaves it on its own node, a restore that has not landed leaves it parked.
pub fn imported(
    row: &SourceLimboRow,
    node_id: i64,
    suspended: bool,
    parking: &HashSet<i64>,
) -> Imported {
    let away = row.origin_node_id != Some(node_id) && parking.contains(&node_id);

    match row.state.as_str() {
        "limbo" | "hibernating" | "restoring" if away => Imported::Parked {
            since: row.limbo_at.or(row.updated),
        },
        "limbo" | "restoring" => Imported::Ordinary {
            unsuspend: suspended,
        },
        "stopped" if !suspended && !parking.contains(&node_id) => Imported::Stopped {
            since: row.stopped_at.or(row.updated),
        },
        _ => Imported::Ordinary { unsuspend: false },
    }
}

/// The ports of the allocations a parked server had on the node it came from, primary first;
/// allocations that are gone or on another node are left out.
pub fn home_ports(
    row: &SourceLimboRow,
    home_node_id: i64,
    allocations: &HashMap<i64, (i64, i64)>,
) -> Vec<i32> {
    let mut ids = Vec::with_capacity(row.origin_allocation_ids.len() + 1);
    ids.extend(row.origin_allocation_id);
    ids.extend(
        row.origin_allocation_ids
            .iter()
            .copied()
            .filter(|id| Some(*id) != row.origin_allocation_id),
    );

    let mut ports = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some((node_id, port)) = allocations.get(&id)
            && *node_id == home_node_id
            && let Ok(port) = i32::try_from(*port)
            && !ports.contains(&port)
        {
            ports.push(port);
        }
    }

    ports
}

/// One server's limbo row to write.
pub struct PlanLimbo {
    pub server_uuid: uuid::Uuid,
    pub state: LimboState,
    pub since: DateTime<Utc>,
    pub parked: Option<DateTime<Utc>>,
    pub home_node_uuid: Option<uuid::Uuid>,
    pub home_ports: Vec<i32>,
}

/// Plans the limbo rows and changes the planned nodes (role `limbo`, their locations hidden) and
/// servers (limbo's suspension lifted). Returns notes for the report.
pub fn plan(
    source: &SourceData,
    nodes: &IdMap,
    servers: &IdMap,
    target: &mut Plan,
) -> Vec<(&'static str, String)> {
    let limbo = &source.extras.limbo;
    let mut notes = Vec::new();
    if limbo.rows.is_empty() {
        return notes;
    }

    let now = Utc::now();
    let server_nodes: HashMap<i64, i64> = source
        .servers
        .iter()
        .map(|server| (server.id, server.node_id))
        .collect();
    let tagged = source
        .nodes
        .iter()
        .filter(|node| {
            !limbo.node_tag.is_empty()
                && node
                    .placement
                    .tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(&limbo.node_tag))
        })
        .map(|node| node.id);
    let parking = parking_nodes(&limbo.rows, tagged, &server_nodes);
    let allocations: HashMap<i64, (i64, i64)> = source
        .allocations
        .iter()
        .map(|allocation| (allocation.id, (allocation.node_id, allocation.port)))
        .collect();

    // the parking nodes become limbo nodes; a location with nothing else becomes admins' only
    let parking_uuids: HashSet<uuid::Uuid> = parking
        .iter()
        .filter_map(|node_id| nodes.get(node_id).copied())
        .collect();
    for node in &mut target.nodes {
        if parking_uuids.contains(&node.uuid) {
            node.role = NODE_ROLE.into();
        }
    }
    let mixed: HashSet<uuid::Uuid> = target
        .nodes
        .iter()
        .filter(|node| !parking_uuids.contains(&node.uuid))
        .map(|node| node.location_uuid)
        .collect();
    for location in &mut target.locations {
        if !mixed.contains(&location.uuid)
            && target
                .nodes
                .iter()
                .any(|node| node.location_uuid == location.uuid)
        {
            location.placement.public = false;
        }
    }

    let suspended: HashMap<uuid::Uuid, bool> = target
        .servers
        .iter()
        .map(|server| (server.uuid, server.suspended))
        .collect();
    let mut unsuspend: HashSet<uuid::Uuid> = HashSet::new();
    let (mut homeless, mut restores, mut restore_tokens, mut moves) = (0, 0, 0, 0);

    for row in &limbo.rows {
        let (Some(server_uuid), Some(node_id)) = (
            servers.get(&row.server_id),
            server_nodes.get(&row.server_id),
        ) else {
            continue;
        };
        let is_suspended = suspended.get(server_uuid).copied().unwrap_or(false);

        match imported(row, *node_id, is_suspended, &parking) {
            Imported::Parked { since } => {
                let since = since.unwrap_or(now);
                let home = row
                    .origin_node_id
                    .filter(|home| !parking.contains(home))
                    .and_then(|home| Some((home, *nodes.get(&home)?)));
                if home.is_none() {
                    homeless += 1;
                }
                if row.state == "restoring" {
                    restores += 1;
                    restore_tokens += row.restore_cost.max(0);
                }

                target.extras.limbo.push(PlanLimbo {
                    server_uuid: *server_uuid,
                    state: LimboState::Parked,
                    since,
                    parked: Some(since),
                    home_node_uuid: home.map(|(_, uuid)| uuid),
                    home_ports: home
                        .map(|(home, _)| home_ports(row, home, &allocations))
                        .unwrap_or_default(),
                });
                unsuspend.insert(*server_uuid);
            }
            Imported::Stopped { since } => target.extras.limbo.push(PlanLimbo {
                server_uuid: *server_uuid,
                state: LimboState::Stopped,
                since: since.unwrap_or(now),
                parked: None,
                home_node_uuid: None,
                home_ports: Vec::new(),
            }),
            Imported::Ordinary { unsuspend: lift } => {
                if row.state == "hibernating" {
                    moves += 1;
                }
                if lift {
                    unsuspend.insert(*server_uuid);
                }
            }
        }
    }

    for server in &mut target.servers {
        if unsuspend.contains(&server.uuid) {
            server.suspended = false;
        }
    }

    let parked = target
        .extras
        .limbo
        .iter()
        .filter(|row| row.state == LimboState::Parked)
        .count();
    let stopped = target.extras.limbo.len() - parked;
    if parked > 0 {
        notes.push((
            "server_limbo",
            format!(
                "{parked} parked server(s) stay on {} limbo node(s) and restore by a move off them; {homeless} of them have no home node any more, their owners pick a location",
                parking_uuids.len()
            ),
        ));
    }
    if stopped > 0 {
        notes.push((
            "server_limbo",
            format!("{stopped} idle stop(s) were carried over"),
        ));
    }
    if restores > 0 {
        notes.push((
            "server_limbo",
            format!(
                "{restores} restore(s) had not landed and stay parked; the {restore_tokens} token(s) paid for them were not refunded"
            ),
        ));
    }
    if moves > 0 {
        notes.push((
            "server_limbo",
            format!(
                "{moves} move(s) to the limbo node had not landed: those servers stay on their own node"
            ),
        ));
    }

    notes
}

/// Writes the limbo rows inside the import's transaction (the node roles and the suspensions are
/// written with the nodes and servers).
pub async fn write(tx: &mut sqlx::PgConnection, plan: &Plan) -> Result<(), anyhow::Error> {
    for row in &plan.extras.limbo {
        sqlx::query("UPDATE servers SET limbo_state = $2 WHERE uuid = $1")
            .bind(row.server_uuid)
            .bind(row.state.as_str())
            .execute(&mut *tx)
            .await
            .context("failed to write a server's limbo state")?;
        sqlx::query(
            "INSERT INTO server_limbo (server_uuid, since, parked, home_node_uuid, home_ports, updated)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (server_uuid) DO UPDATE SET since = EXCLUDED.since,
                parked = EXCLUDED.parked, home_node_uuid = EXCLUDED.home_node_uuid,
                home_ports = EXCLUDED.home_ports, updated = NOW()",
        )
        .bind(row.server_uuid)
        .bind(row.since.naive_utc())
        .bind(row.parked.map(|parked| parked.naive_utc()))
        .bind(row.home_node_uuid)
        .bind(&row.home_ports)
        .execute(&mut *tx)
        .await
        .context("failed to write a server's limbo row")?;
    }
    tracing::info!("wrote {} limbo rows", plan.extras.limbo.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32) -> Option<DateTime<Utc>> {
        chrono::NaiveDate::from_ymd_opt(2026, 10, day)
            .and_then(|date| date.and_hms_opt(12, 0, 0))
            .map(|at| at.and_utc())
    }

    fn row(server_id: i64, state: &str, origin: i64, limbo_node: Option<i64>) -> SourceLimboRow {
        SourceLimboRow {
            server_id,
            state: state.into(),
            stopped_at: at(1),
            limbo_at: at(3),
            origin_node_id: Some(origin),
            origin_allocation_id: Some(11),
            origin_allocation_ids: vec![12, 11],
            limbo_node_id: limbo_node,
            restore_cost: 5,
            updated: at(4),
        }
    }

    #[test]
    fn origin_allocations_are_read_from_json() {
        assert_eq!(allocation_ids(Some("[3, \"4\", null, 5]")), vec![3, 4, 5]);
        assert!(allocation_ids(Some("not json")).is_empty());
        assert!(allocation_ids(None).is_empty());
    }

    #[test]
    fn parking_nodes_are_tagged_recorded_or_where_parked_servers_are() {
        // node 86 has the tag; node 2 lost it, its servers still record it; node 5 holds a parked
        // server with no record; node 73 is a home
        let rows = vec![
            row(1, "limbo", 73, Some(2)),
            row(2, "limbo", 73, None),
            row(3, "stopped", 73, Some(9)),
            row(4, "restored", 73, Some(8)),
            row(5, "limbo", 80, None),
        ];
        let server_nodes = HashMap::from([(1, 2), (2, 5), (3, 73), (4, 73), (5, 80)]);

        let parking = parking_nodes(&rows, [86], &server_nodes);

        assert_eq!(parking, HashSet::from([86, 2, 5]));
    }

    #[test]
    fn states_map_by_where_the_server_is() {
        let parking = HashSet::from([86, 2]);

        // parked on a limbo node: parked since it went there, whatever moved it
        for state in ["limbo", "hibernating", "restoring"] {
            assert_eq!(
                imported(&row(1, state, 73, Some(86)), 86, true, &parking),
                Imported::Parked { since: at(3) }
            );
        }
        let mut no_time = row(1, "limbo", 73, Some(2));
        no_time.limbo_at = None;
        assert_eq!(
            imported(&no_time, 2, true, &parking),
            Imported::Parked { since: at(4) }
        );

        // back on (or never off) its own node: an ordinary server, limbo's suspension lifted
        assert_eq!(
            imported(&row(1, "limbo", 73, Some(86)), 73, true, &parking),
            Imported::Ordinary { unsuspend: true }
        );
        assert_eq!(
            imported(&row(1, "restoring", 73, Some(86)), 80, true, &parking),
            Imported::Ordinary { unsuspend: true }
        );
        // a move to the limbo node that had not landed: left alone
        assert_eq!(
            imported(&row(1, "hibernating", 73, Some(86)), 73, false, &parking),
            Imported::Ordinary { unsuspend: false }
        );
        assert_eq!(
            imported(&row(1, "hibernating", 73, Some(86)), 73, true, &parking),
            Imported::Ordinary { unsuspend: false }
        );

        // idle stops carry over on a game node, not on a suspended server
        assert_eq!(
            imported(&row(1, "stopped", 73, None), 73, false, &parking),
            Imported::Stopped { since: at(1) }
        );
        assert_eq!(
            imported(&row(1, "stopped", 73, None), 73, true, &parking),
            Imported::Ordinary { unsuspend: false }
        );

        for state in ["restored", "failed", "something"] {
            assert_eq!(
                imported(&row(1, state, 73, Some(86)), 73, true, &parking),
                Imported::Ordinary { unsuspend: false }
            );
        }
    }

    #[test]
    fn home_ports_are_the_primary_first_on_the_home_node() {
        let allocations = HashMap::from([
            (11, (73, 25565)),
            (12, (73, 25566)),
            (13, (80, 30000)),
            (14, (73, 25565)),
        ]);

        assert_eq!(
            home_ports(&row(1, "limbo", 73, None), 73, &allocations),
            vec![25565, 25566]
        );

        // allocations that are gone or moved to another node are left out, no port twice
        let mut moved = row(1, "limbo", 73, None);
        moved.origin_allocation_id = Some(99);
        moved.origin_allocation_ids = vec![13, 11, 14, 99];
        assert_eq!(home_ports(&moved, 73, &allocations), vec![25565]);
        assert!(home_ports(&moved, 80, &HashMap::new()).is_empty());
    }
}
