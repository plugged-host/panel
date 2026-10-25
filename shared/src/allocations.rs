//! plugged.host: allocations made when they are needed, and locked ones (docs/features/allocations.md).
//!
//! A player's new server, a move, a failover restore, a limbo restore, a devbox and a port a player
//! adds take a free allocation of the node inside the deployment port ranges
//! (`placement::port_ranges`), and create one when none is free: on the address most of the node's
//! allocations use (0.0.0.0 with the node's host name as alias on a node without any). Ports
//! something on the node already listens on are skipped. Picks run under a lock per node and stay
//! reserved for a couple of minutes, so two creates or moves at the same time never pick the same
//! port; the unique index on `node_allocations` and `server_allocations` is the last word, and a
//! pick that loses to it is made again.
//!
//! Games that need a second port (txAdmin on FiveM, Multi Theft Auto's web server) get two ports in
//! a row, the second written into its variable. When a server moves, egg variables that held one
//! of its old ports follow it to the new port, and its new allocations keep the lock and notes of
//! the ones they replace. Allocations a server is created with, and ones an admin adds, are locked:
//! only an admin can remove them.

use crate::{
    models::{
        CreatableModel, EventEmittingModel, ListenerPriority,
        node::Node,
        server::{Server, ServerEvent},
        server_allocation::ServerAllocation,
        server_variable::ServerVariable,
    },
    prelude::SqlxErrorExt,
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
};

/// The deployment port ranges until an admin sets others: earlier ranges are used first.
pub const DEFAULT_PORT_RANGES: &str = "30000-50000,25000-30000";
/// Ports below this are left to the node's own services.
pub const PORT_FLOOR: u16 = 1024;
/// Variables that hold a server's second port: txAdmin's web interface on FiveM and RedM, the
/// HTTP server Multi Theft Auto sends resources from. Their defaults are ports the container does
/// not publish, so such a server gets two ports in a row and the second goes into the variable.
pub const SECOND_PORT_VARIABLES: [&str; 2] = ["TXHOST_TXA_PORT", "SERVER_WEBPORT"];
/// How long a picked port stays out of other picks while it is being claimed.
const RESERVATION_SECONDS: i64 = 120;
/// Picks made again after losing a port to someone else.
const ATTEMPTS: usize = 3;

pub type PortRange = (u16, u16);

/// Reads `30000-50000,25000-30000`: ranges and single ports, comma separated, from
/// [`PORT_FLOOR`] up.
pub fn parse_ranges(value: &str) -> Result<Vec<PortRange>, String> {
    let mut ranges = Vec::new();

    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (start, end) = match part.split_once('-') {
            Some((start, end)) => (start.trim().parse::<u16>(), end.trim().parse::<u16>()),
            None => (part.parse::<u16>(), part.parse::<u16>()),
        };
        let (Ok(start), Ok(end)) = (start, end) else {
            return Err(format!("{part} is not a port or a range of ports"));
        };
        if start < PORT_FLOOR {
            return Err(format!("{part} starts below port {PORT_FLOOR}"));
        }
        if start > end {
            return Err(format!("{part} ends before it starts"));
        }

        ranges.push((start, end));
    }

    if ranges.is_empty() {
        return Err("at least one port range is needed".into());
    }

    Ok(ranges)
}

/// [`parse_ranges`] for a settings payload.
pub fn validate_ranges(value: &compact_str::CompactString, _context: &()) -> garde::Result {
    parse_ranges(value).map(|_| ()).map_err(garde::Error::new)
}

/// The variable that takes the egg's second port, when it needs one: txAdmin's on a FiveM egg,
/// else the first of [`SECOND_PORT_VARIABLES`] the egg declares.
pub fn second_port_variable<'a>(
    fivem: bool,
    declared: impl Iterator<Item = &'a str>,
) -> Option<&'static str> {
    if fivem {
        return Some(SECOND_PORT_VARIABLES[0]);
    }

    let declared = declared.collect::<HashSet<_>>();
    SECOND_PORT_VARIABLES
        .into_iter()
        .find(|name| declared.contains(name))
}

/// Egg variables (environment name, value) whose name has `PORT` in it and whose value is one of
/// the old ports, set to the port that replaces it.
pub fn follow_ports<'a>(
    variables: impl Iterator<Item = (&'a str, &'a str)>,
    moved: &HashMap<i32, i32>,
) -> HashMap<String, String> {
    if moved.is_empty() {
        return HashMap::new();
    }

    variables
        .filter(|(name, _)| name.to_ascii_uppercase().contains("PORT"))
        .filter_map(|(name, value)| {
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }

            let to = moved.get(&value.parse::<i32>().ok()?)?;
            Some((name.to_string(), to.to_string()))
        })
        .collect()
}

/// Old port to new port, for the ports that changed.
pub fn port_moves(old: &[i32], new: &[i32]) -> HashMap<i32, i32> {
    old.iter()
        .zip(new)
        .filter(|(from, to)| from != to)
        .map(|(from, to)| (*from, *to))
        .collect()
}

/// The address an allocation is made on and the alias it shows.
#[derive(Clone, Debug)]
pub struct Address {
    pub ip: sqlx::types::ipnetwork::IpNetwork,
    pub alias: Option<compact_str::CompactString>,
}

/// A port of a node, as a pick sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pick {
    /// A free allocation that exists.
    Existing(uuid::Uuid, u16),
    /// A port without an allocation on the address: made when picked.
    New(u16),
}

impl Pick {
    pub(crate) fn port(self) -> u16 {
        match self {
            Self::Existing(_, port) | Self::New(port) => port,
        }
    }

    fn is_new(self) -> bool {
        matches!(self, Self::New(_))
    }
}

/// An allocation of the node, for [`Ports::build`].
pub(crate) struct NodePort {
    pub(crate) uuid: uuid::Uuid,
    pub(crate) ip: IpAddr,
    pub(crate) port: u16,
    pub(crate) assigned: bool,
}

/// Two addresses a container could not both listen on with the same port.
fn conflicts(a: IpAddr, b: IpAddr) -> bool {
    a == b || a.is_unspecified() || b.is_unspecified()
}

/// The ports of a node a pick may take on one address.
pub(crate) struct Ports {
    /// Inside the ranges, in the ranges' order: a free allocation or a port to make one on.
    slots: Vec<Pick>,
    /// The range (its place in the settings) of each slot.
    range_of: Vec<usize>,
    /// Free allocations on the address, inside the ranges or not (a move keeps its port numbers).
    free: HashMap<u16, uuid::Uuid>,
}

impl Ports {
    /// `blocked`: ports in use on the node or reserved by another pick.
    pub(crate) fn build(
        rows: &[NodePort],
        address: IpAddr,
        blocked: &HashSet<u16>,
        ranges: &[PortRange],
    ) -> Self {
        let mut free = HashMap::new();
        let mut on_address = HashSet::new();
        // a port taken on an address that overlaps this one
        let mut taken = HashSet::new();
        // any allocation on an overlapping address of its own: none can be made on this one
        let mut elsewhere = HashSet::new();

        for row in rows {
            if row.ip == address {
                on_address.insert(row.port);
                if row.assigned {
                    taken.insert(row.port);
                } else {
                    free.insert(row.port, row.uuid);
                }
            } else if conflicts(row.ip, address) {
                elsewhere.insert(row.port);
                if row.assigned {
                    taken.insert(row.port);
                }
            }
        }

        free.retain(|port, _| !blocked.contains(port) && !taken.contains(port));

        let mut slots = Vec::new();
        let mut range_of = Vec::new();
        let mut seen = HashSet::new();
        for (range, &(start, end)) in ranges.iter().enumerate() {
            for port in start..=end {
                if !seen.insert(port) {
                    continue;
                }
                if let Some(uuid) = free.get(&port) {
                    slots.push(Pick::Existing(*uuid, port));
                } else if !on_address.contains(&port)
                    && !elsewhere.contains(&port)
                    && !blocked.contains(&port)
                {
                    slots.push(Pick::New(port));
                } else {
                    continue;
                }
                range_of.push(range);
            }
        }

        Self {
            slots,
            range_of,
            free,
        }
    }

    fn slot(&self, port: u16) -> Option<Pick> {
        self.slots.iter().copied().find(|pick| pick.port() == port)
    }

    /// `count` ports for a new server. One: a random free allocation, else a random port of the
    /// first range with room. More: in a row, with as few new allocations as possible (the lowest
    /// such run, ranges in order), else any.
    pub(crate) fn pick_fresh(
        &self,
        count: usize,
        random: &mut impl FnMut(usize) -> usize,
    ) -> Option<Vec<Pick>> {
        match count {
            0 => return Some(Vec::new()),
            1 => {
                let existing = self
                    .slots
                    .iter()
                    .copied()
                    .filter(|pick| !pick.is_new())
                    .collect::<Vec<_>>();
                if !existing.is_empty() {
                    return Some(vec![existing[random(existing.len()) % existing.len()]]);
                }

                // the new ports of the first range that has any (next to each other in `slots`)
                let first = *self.range_of.first()?;
                let range_end = self
                    .range_of
                    .iter()
                    .position(|range| *range != first)
                    .unwrap_or(self.slots.len());
                let candidates = &self.slots[..range_end];
                return Some(vec![
                    candidates[random(candidates.len()) % candidates.len()],
                ]);
            }
            _ => {}
        }

        let ports = self
            .slots
            .iter()
            .map(|pick| (pick.port(), *pick))
            .collect::<HashMap<_, _>>();

        let mut best: Option<(usize, Vec<Pick>)> = None;
        for start in &self.slots {
            let run = (0..count)
                .map(|offset| {
                    start
                        .port()
                        .checked_add(offset as u16)
                        .and_then(|port| ports.get(&port).copied())
                })
                .collect::<Option<Vec<_>>>();
            let Some(run) = run else {
                continue;
            };

            let new = run.iter().filter(|pick| pick.is_new()).count();
            if best.as_ref().is_none_or(|(fewest, _)| new < *fewest) {
                best = Some((new, run));
                if new == 0 {
                    break;
                }
            }
        }
        if let Some((_, run)) = best {
            return Some(run);
        }

        let mut any = self
            .slots
            .iter()
            .copied()
            .filter(|pick| !pick.is_new())
            .chain(self.slots.iter().copied().filter(|pick| pick.is_new()))
            .take(count)
            .collect::<Vec<_>>();
        if any.len() < count {
            return None;
        }
        any.sort_by_key(|pick| pick.port());

        Some(any)
    }

    /// The ports a moved server gets, index for index with `old`: the same number where it is
    /// free or can be made inside the ranges, else a free allocation inside the ranges, else a new
    /// port. `None` when the ranges are full.
    pub(crate) fn pick_move(&self, old: &[u16]) -> Option<Vec<Pick>> {
        let mut picks: Vec<Option<Pick>> = vec![None; old.len()];
        let mut claimed = HashSet::new();

        for (index, port) in old.iter().enumerate() {
            if claimed.contains(port) {
                continue;
            }

            let pick = match self.free.get(port) {
                Some(uuid) => Some(Pick::Existing(*uuid, *port)),
                None => self.slot(*port).filter(|pick| pick.is_new()),
            };
            if let Some(pick) = pick {
                claimed.insert(*port);
                picks[index] = Some(pick);
            }
        }

        for index in 0..old.len() {
            if picks[index].is_some() {
                continue;
            }

            let pick = self
                .slots
                .iter()
                .copied()
                .filter(|pick| !claimed.contains(&pick.port()))
                .find(|pick| !pick.is_new())
                .or_else(|| {
                    self.slots
                        .iter()
                        .copied()
                        .find(|pick| !claimed.contains(&pick.port()))
                })?;
            claimed.insert(pick.port());
            picks[index] = Some(pick);
        }

        picks.into_iter().collect()
    }

    /// A port a player adds: the first one above `from` (the primary's), else the first of the
    /// ranges.
    pub(crate) fn pick_above(&self, from: u16) -> Option<Pick> {
        self.slots
            .iter()
            .copied()
            .filter(|pick| pick.port() > from)
            .min_by_key(|pick| pick.port())
            .or_else(|| self.slots.first().copied())
    }
}

/// Why no port could be had.
#[derive(Debug)]
pub enum PortError {
    /// Every port of the ranges is taken.
    NoPort,
    /// The node did not say which ports are in use.
    Unreachable(anyhow::Error),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for PortError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for PortError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl From<sqlx::Error> for PortError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl PortError {
    /// The error a route answers with.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::NoPort => anyhow::Error::new(
                crate::response::DisplayError::new("every port this node may use is taken")
                    .with_status(StatusCode::EXPECTATION_FAILED),
            ),
            Self::Unreachable(err) => {
                tracing::warn!("failed to ask a node which ports are in use: {err:#}");

                anyhow::Error::new(
                    crate::response::DisplayError::new(
                        "could not reach the node to check which ports are free",
                    )
                    .with_status(StatusCode::EXPECTATION_FAILED),
                )
            }
            Self::Other(err) => err,
        }
    }
}

/// The deployment port ranges of the settings.
pub async fn ranges(state: &crate::State) -> Result<Vec<PortRange>, anyhow::Error> {
    let value = state.settings.get().await?.placement.port_ranges.clone();

    Ok(parse_ranges(&value).unwrap_or_else(|_| {
        parse_ranges(DEFAULT_PORT_RANGES).expect("the default port ranges are valid")
    }))
}

/// The address new allocations of the node get: the address and alias most of its allocations
/// use, else 0.0.0.0 with the node's host name (its SFTP host, else its Wings URL's) as alias.
pub async fn node_address(
    database: &crate::database::Database,
    node: &Node,
) -> Result<Address, anyhow::Error> {
    let row = sqlx::query(
        "SELECT node_allocations.ip, node_allocations.ip_alias
        FROM node_allocations
        WHERE node_allocations.node_uuid = $1
        GROUP BY node_allocations.ip, node_allocations.ip_alias
        ORDER BY COUNT(*) DESC, node_allocations.ip
        LIMIT 1",
    )
    .bind(node.uuid)
    .fetch_optional(database.read())
    .await?;

    if let Some(row) = row {
        return Ok(Address {
            ip: row.try_get("ip")?,
            alias: row
                .try_get::<Option<String>, _>("ip_alias")?
                .map(Into::into),
        });
    }

    let alias = node
        .sftp_host
        .clone()
        .filter(|host| !host.is_empty())
        .or_else(|| node.url.host_str().map(Into::into));

    Ok(Address {
        ip: sqlx::types::ipnetwork::IpNetwork::from(IpAddr::from([0, 0, 0, 0])),
        alias,
    })
}

fn lock_key(node_uuid: uuid::Uuid) -> String {
    format!("nodes::{node_uuid}::allocations")
}

fn reserved_key(node_uuid: uuid::Uuid) -> String {
    format!("nodes::{node_uuid}::reserved_ports")
}

#[derive(Serialize, Deserialize)]
struct Reserved {
    ip: IpAddr,
    port: u16,
    until: i64,
}

async fn reservations(state: &crate::State, node_uuid: uuid::Uuid) -> Vec<Reserved> {
    let now = chrono::Utc::now().timestamp();

    match state
        .cache
        .get::<Vec<Reserved>>(&reserved_key(node_uuid))
        .await
    {
        Ok(reserved) => reserved
            .unwrap_or_default()
            .into_iter()
            .filter(|reserved| reserved.until > now)
            .collect(),
        Err(err) => {
            tracing::warn!(node = %node_uuid, "failed to read the reserved ports: {err:#}");
            Vec::new()
        }
    }
}

/// Keeps `ports` out of other picks until they are claimed.
async fn reserve(state: &crate::State, node_uuid: uuid::Uuid, ip: IpAddr, ports: &[u16]) {
    let until = chrono::Utc::now().timestamp() + RESERVATION_SECONDS;
    let mut reserved = reservations(state, node_uuid).await;
    reserved.extend(ports.iter().map(|port| Reserved {
        ip,
        port: *port,
        until,
    }));

    if let Err(err) = state
        .cache
        .set(
            &reserved_key(node_uuid),
            RESERVATION_SECONDS as u64,
            &reserved,
        )
        .await
    {
        tracing::warn!(node = %node_uuid, "failed to reserve ports: {err:#}");
    }
}

/// The node's ports on `address` as of now: its allocations, what listens on the node, what other
/// picks reserved.
async fn load(
    state: &crate::State,
    node: &Node,
    address: &Address,
    ranges: &[PortRange],
) -> Result<Ports, PortError> {
    let ip = address.ip.ip();

    let rows = sqlx::query(
        "SELECT node_allocations.uuid, node_allocations.ip, node_allocations.port,
            server_allocations.uuid IS NOT NULL AS assigned
        FROM node_allocations
        LEFT JOIN server_allocations ON server_allocations.allocation_uuid = node_allocations.uuid
        WHERE node_allocations.node_uuid = $1",
    )
    .bind(node.uuid)
    .fetch_all(state.database.write())
    .await?
    .into_iter()
    .map(|row| {
        Ok::<_, sqlx::Error>(NodePort {
            uuid: row.try_get("uuid")?,
            ip: row
                .try_get::<sqlx::types::ipnetwork::IpNetwork, _>("ip")?
                .ip(),
            port: u16::try_from(row.try_get::<i32, _>("port")?).unwrap_or(0),
            assigned: row.try_get("assigned")?,
        })
    })
    .collect::<Result<Vec<_>, _>>()?;

    let mut blocked = node
        .used_ports(state, &[ip])
        .await
        .map_err(PortError::Unreachable)?
        .into_values()
        .flatten()
        .collect::<HashSet<_>>();
    blocked.extend(
        reservations(state, node.uuid)
            .await
            .into_iter()
            .filter(|reserved| conflicts(reserved.ip, ip))
            .map(|reserved| reserved.port),
    );

    Ok(Ports::build(&rows, ip, &blocked, ranges))
}

/// Makes the allocations of the new ports among `picks`. `None` when one of them was made by
/// someone else meanwhile: pick again.
async fn materialize(
    database: &crate::database::Database,
    node_uuid: uuid::Uuid,
    address: &Address,
    picks: &[Pick],
) -> Result<Option<Vec<(uuid::Uuid, i32)>>, sqlx::Error> {
    let mut made = Vec::with_capacity(picks.len());

    for pick in picks {
        match *pick {
            Pick::Existing(uuid, port) => made.push((uuid, port as i32)),
            Pick::New(port) => {
                let uuid: Option<uuid::Uuid> = sqlx::query_scalar(
                    "INSERT INTO node_allocations (node_uuid, ip, ip_alias, port)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT DO NOTHING
                    RETURNING uuid",
                )
                .bind(node_uuid)
                .bind(address.ip)
                .bind(address.alias.as_deref())
                .bind(port as i32)
                .fetch_optional(database.write())
                .await?;

                match uuid {
                    Some(uuid) => made.push((uuid, port as i32)),
                    None => return Ok(None),
                }
            }
        }
    }

    Ok(Some(made))
}

/// Picks with `plan` under the node's lock, makes what is missing and reserves the result for its
/// caller to claim. `None` when the ranges have no room.
async fn deploy_with(
    state: &crate::State,
    node: &Node,
    mut plan: impl FnMut(&Ports) -> Option<Vec<Pick>>,
) -> Result<Option<Vec<(uuid::Uuid, i32)>>, PortError> {
    let ranges = ranges(state).await?;
    let _lock = state
        .cache
        .lock(lock_key(node.uuid), Some(30), Some(10_000))
        .await?;
    let address = node_address(&state.database, node).await?;

    for _ in 0..ATTEMPTS {
        let ports = load(state, node, &address, &ranges).await?;
        let Some(picks) = plan(&ports) else {
            return Ok(None);
        };

        if let Some(made) = materialize(&state.database, node.uuid, &address, &picks).await? {
            reserve(
                state,
                node.uuid,
                address.ip.ip(),
                &picks.iter().map(|pick| pick.port()).collect::<Vec<_>>(),
            )
            .await;

            return Ok(Some(made));
        }
    }

    Ok(None)
}

fn random(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (rand::random::<u64>() % len as u64) as usize
    }
}

/// `count` free allocations for a new server on `node` (more than one in a row), made when
/// missing; the first is its primary. `None` when the ranges have no room. Reserved for a couple
/// of minutes: claim them with the server.
pub async fn deploy(
    state: &crate::State,
    node: &Node,
    count: usize,
) -> Result<Option<Vec<(uuid::Uuid, i32)>>, PortError> {
    deploy_with(state, node, |ports| ports.pick_fresh(count, &mut random)).await
}

/// The allocations a server moving to `node` gets for its `ports` (primary first), keeping the
/// port numbers where it can; made when missing, reserved like [`deploy`]'s.
pub async fn deploy_for_move(
    state: &crate::State,
    node: &Node,
    ports: &[i32],
) -> Result<Option<Vec<(uuid::Uuid, i32)>>, PortError> {
    let old = ports
        .iter()
        .map(|port| u16::try_from(*port).unwrap_or(0))
        .collect::<Vec<_>>();

    deploy_with(state, node, |available| available.pick_move(&old)).await
}

/// Gives the server one more port: a free allocation of its node above its primary port (on the
/// primary's address) inside the ranges, made when missing. Returns the server allocation.
pub async fn add_port(
    state: &crate::State,
    server: &Server,
    notes: Option<&str>,
) -> Result<uuid::Uuid, PortError> {
    let node = server.node.fetch_cached(&state.database).await?;
    let ranges = ranges(state).await?;
    let _lock = state
        .cache
        .lock(lock_key(node.uuid), Some(30), Some(10_000))
        .await?;

    let (address, from) = match &server.allocation {
        Some(primary) => (
            Address {
                ip: primary.allocation.ip,
                alias: primary.allocation.ip_alias.clone(),
            },
            u16::try_from(primary.allocation.port).unwrap_or(0),
        ),
        None => (node_address(&state.database, &node).await?, 0),
    };

    for _ in 0..ATTEMPTS {
        let ports = load(state, &node, &address, &ranges).await?;
        let Some(pick) = ports.pick_above(from) else {
            return Err(PortError::NoPort);
        };
        let Some(made) = materialize(&state.database, node.uuid, &address, &[pick]).await? else {
            continue;
        };

        let claimed = sqlx::query_scalar::<_, uuid::Uuid>(
            "INSERT INTO server_allocations (server_uuid, allocation_uuid, notes)
            VALUES ($1, $2, $3)
            RETURNING uuid",
        )
        .bind(server.uuid)
        .bind(made[0].0)
        .bind(notes)
        .fetch_one(state.database.write())
        .await;

        match claimed {
            Ok(uuid) => return Ok(uuid),
            Err(err) if err.is_unique_violation() => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Err(PortError::NoPort)
}

/// The ports of allocations by their uuid.
pub async fn ports_of(
    database: &crate::database::Database,
    uuids: &[uuid::Uuid],
) -> Result<HashMap<uuid::Uuid, i32>, sqlx::Error> {
    if uuids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query("SELECT uuid, port FROM node_allocations WHERE uuid = ANY($1)")
        .bind(uuids)
        .fetch_all(database.read())
        .await?;

    rows.into_iter()
        .map(|row| Ok((row.try_get("uuid")?, row.try_get("port")?)))
        .collect()
}

/// The server's allocations in the order a move lines them up with the new ones: its primary
/// first, the others in their order.
pub fn move_order(server: &Server, allocations: Vec<ServerAllocation>) -> Vec<ServerAllocation> {
    let primary = server.allocation.as_ref().map(|primary| primary.uuid);
    let (first, rest): (Vec<_>, Vec<_>) = allocations
        .into_iter()
        .partition(|allocation| Some(allocation.uuid) == primary);

    first.into_iter().chain(rest).collect()
}

/// The egg variables that held one of the server's `old` ports, set to the `new` port in the same
/// place (by environment name).
pub async fn followed_variables(
    database: &crate::database::Database,
    server: &Server,
    old: &[i32],
    new: &[i32],
) -> Result<HashMap<String, String>, crate::database::DatabaseError> {
    let moved = port_moves(old, new);
    if moved.is_empty() {
        return Ok(HashMap::new());
    }

    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(database, server.uuid, server.egg.uuid).await?;

    Ok(follow_ports(
        variables.iter().map(|variable| {
            (
                variable.variable.env_variable.as_str(),
                variable.value.as_str(),
            )
        }),
        &moved,
    ))
}

/// Gives the server's allocations on its new node (`new`: node allocations, primary first) the
/// lock and notes of the ones on other nodes they replace, index for index in move order
/// (`primary` is the old primary).
pub async fn carry_over(
    executor: impl sqlx::PgExecutor<'_>,
    server_uuid: uuid::Uuid,
    primary: Option<uuid::Uuid>,
    node_uuid: uuid::Uuid,
    new: &[uuid::Uuid],
) -> Result<(), sqlx::Error> {
    if new.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "WITH old AS (
            SELECT server_allocations.locked, server_allocations.notes,
                ROW_NUMBER() OVER (ORDER BY
                    COALESCE(server_allocations.uuid = $2, FALSE) DESC, server_allocations.created,
                    node_allocations.ip, node_allocations.port, server_allocations.uuid
                ) AS n
            FROM server_allocations
            JOIN node_allocations ON node_allocations.uuid = server_allocations.allocation_uuid
            WHERE server_allocations.server_uuid = $1 AND node_allocations.node_uuid <> $3
        ), new AS (
            SELECT allocation_uuid, n FROM UNNEST($4::uuid[]) WITH ORDINALITY AS new(allocation_uuid, n)
        )
        UPDATE server_allocations
        SET locked = old.locked, notes = COALESCE(server_allocations.notes, old.notes)
        FROM new
        JOIN old ON old.n = new.n
        WHERE server_allocations.server_uuid = $1
            AND server_allocations.allocation_uuid = new.allocation_uuid",
    )
    .bind(server_uuid)
    .bind(primary)
    .bind(node_uuid)
    .bind(new)
    .execute(executor)
    .await?;

    Ok(())
}

/// Locks the allocations a server is created with (an admin's create, a player's, a devbox), in
/// the creating transaction, and carries locks and notes over to a moving server's new
/// allocations once its transfer started.
pub fn register() {
    Server::register_after_create_handler(
        ListenerPriority::Normal,
        |server, _options, _state, transaction| {
            Box::pin(async move {
                sqlx::query("UPDATE server_allocations SET locked = TRUE WHERE server_uuid = $1")
                    .bind(server.uuid)
                    .execute(&mut **transaction)
                    .await?;
                if let Some(primary) = server.allocation.as_mut() {
                    primary.locked = true;
                }

                Ok(())
            })
        },
    );

    let _ = Server::register_event_handler(|state, event| async move {
        if let ServerEvent::TransferStarted {
            server,
            destination_node,
            destination_allocation,
            destination_allocations,
        } = &*event
        {
            let mut new = Vec::with_capacity(destination_allocations.len() + 1);
            if let Some(primary) = destination_allocation {
                let allocation_uuid: Option<uuid::Uuid> = sqlx::query_scalar(
                    "SELECT allocation_uuid FROM server_allocations WHERE uuid = $1",
                )
                .bind(primary)
                // just inserted by the transfer: not from a replica that may lag
                .fetch_optional(state.database.write())
                .await?;
                new.extend(allocation_uuid);
            }
            new.extend(destination_allocations.iter().copied());

            carry_over(
                state.database.write(),
                server.uuid,
                server.allocation.as_ref().map(|primary| primary.uuid),
                destination_node.uuid,
                &new,
            )
            .await?;
        }

        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(port: u16, assigned: bool) -> NodePort {
        NodePort {
            uuid: uuid::Uuid::from_u128(port as u128),
            ip: IpAddr::from([0, 0, 0, 0]),
            port,
            assigned,
        }
    }

    fn any() -> IpAddr {
        IpAddr::from([0, 0, 0, 0])
    }

    fn port_numbers(picks: &[Pick]) -> Vec<u16> {
        picks.iter().map(|pick| pick.port()).collect()
    }

    #[test]
    fn ranges_are_read_and_checked() {
        assert_eq!(
            parse_ranges(DEFAULT_PORT_RANGES).unwrap(),
            vec![(30000, 50000), (25000, 30000)]
        );
        assert_eq!(
            parse_ranges(" 25565 , 27015-27020 ").unwrap(),
            vec![(25565, 25565), (27015, 27020)]
        );
        assert!(parse_ranges("").is_err());
        assert!(parse_ranges("80-90").is_err());
        assert!(parse_ranges("30000-20000").is_err());
        assert!(parse_ranges("30000-70000").is_err());
        assert!(parse_ranges("abc").is_err());
    }

    #[test]
    fn the_second_port_goes_to_txadmin_or_the_web_port() {
        assert_eq!(
            second_port_variable(true, std::iter::empty()),
            Some("TXHOST_TXA_PORT")
        );
        assert_eq!(
            second_port_variable(false, ["SERVER_JARFILE", "SERVER_WEBPORT"].into_iter()),
            Some("SERVER_WEBPORT")
        );
        assert_eq!(
            second_port_variable(false, ["TXHOST_TXA_PORT", "SERVER_WEBPORT"].into_iter()),
            Some("TXHOST_TXA_PORT")
        );
        assert_eq!(
            second_port_variable(false, ["SERVER_JARFILE"].into_iter()),
            None
        );
    }

    #[test]
    fn port_variables_follow_the_move() {
        let moved = port_moves(&[30000, 30001, 30002], &[41000, 41001, 30002]);
        assert_eq!(moved.len(), 2);

        let followed = follow_ports(
            [
                ("TXHOST_TXA_PORT", "30001"),
                ("QUERY_port", " 30000 "),
                ("SERVER_JARFILE", "30001"),
                ("RCON_PORT", "25575"),
                ("OTHER_PORT", "30002"),
                ("WEB_PORT", "30001abc"),
            ]
            .into_iter(),
            &moved,
        );

        assert_eq!(followed.len(), 2);
        assert_eq!(followed["TXHOST_TXA_PORT"], "41001");
        assert_eq!(followed["QUERY_port"], "41000");
        assert!(follow_ports([("A_PORT", "1")].into_iter(), &HashMap::new()).is_empty());
    }

    #[test]
    fn slots_skip_taken_used_and_overlapping_ports() {
        let mut other = row(30003, false);
        other.ip = IpAddr::from([192, 0, 2, 10]);
        let rows = [
            row(30000, true),
            row(30001, false),
            row(30002, false),
            other,
        ];
        let blocked = HashSet::from([30002]);

        let ports = Ports::build(&rows, any(), &blocked, &[(30000, 30005)]);

        assert_eq!(
            ports.slots,
            vec![
                Pick::Existing(uuid::Uuid::from_u128(30001), 30001),
                Pick::New(30004),
                Pick::New(30005),
            ]
        );
        assert_eq!(ports.free.len(), 1);
    }

    #[test]
    fn one_port_prefers_a_free_allocation() {
        let rows = [row(30001, true), row(30003, false)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30005)]);

        let picked = ports.pick_fresh(1, &mut |_| 0).unwrap();
        assert_eq!(
            picked,
            vec![Pick::Existing(uuid::Uuid::from_u128(30003), 30003)]
        );

        // none free: a new one in the first range with room
        let rows = [row(30000, true), row(30001, true)];
        let ports = Ports::build(
            &rows,
            any(),
            &HashSet::new(),
            &[(30000, 30001), (25000, 25002)],
        );
        let picked = ports.pick_fresh(1, &mut |len| len - 1).unwrap();
        assert_eq!(port_numbers(&picked), vec![25002]);
        let picked = ports.pick_fresh(1, &mut |_| 0).unwrap();
        assert_eq!(port_numbers(&picked), vec![25000]);
    }

    #[test]
    fn two_ports_come_in_a_row_with_as_few_new_ones_as_possible() {
        // a free pair
        let rows = [row(30000, false), row(30002, false), row(30003, false)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30010)]);
        assert_eq!(
            port_numbers(&ports.pick_fresh(2, &mut |_| 0).unwrap()),
            vec![30002, 30003]
        );

        // a free one and a new one next to it
        let rows = [row(30000, true), row(30001, true), row(30005, false)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30010)]);
        let picked = ports.pick_fresh(2, &mut |_| 0).unwrap();
        assert_eq!(port_numbers(&picked), vec![30004, 30005]);
        assert_eq!(picked.iter().filter(|pick| pick.is_new()).count(), 1);

        // only new ones
        let rows = [row(30000, true)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30010)]);
        assert_eq!(
            port_numbers(&ports.pick_fresh(2, &mut |_| 0).unwrap()),
            vec![30001, 30002]
        );

        // no two in a row: any two
        let rows = [row(30001, true), row(30003, true)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30004)]);
        assert_eq!(
            port_numbers(&ports.pick_fresh(2, &mut |_| 0).unwrap()),
            vec![30000, 30002]
        );

        // full
        let rows = [row(30000, true), row(30001, true)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30001)]);
        assert!(ports.pick_fresh(1, &mut |_| 0).is_none());
        assert!(ports.pick_fresh(2, &mut |_| 0).is_none());
    }

    #[test]
    fn a_move_keeps_its_port_numbers_where_it_can() {
        // 30000 is free there, 30001 can be made, 30002 is taken
        let rows = [row(30000, false), row(30002, true), row(30010, false)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30010)]);

        let picked = ports.pick_move(&[30000, 30001, 30002]).unwrap();
        assert_eq!(
            picked[0],
            Pick::Existing(uuid::Uuid::from_u128(30000), 30000)
        );
        assert_eq!(picked[1], Pick::New(30001));
        // a free allocation before a new port
        assert_eq!(
            picked[2],
            Pick::Existing(uuid::Uuid::from_u128(30010), 30010)
        );

        // a free allocation outside the ranges keeps its number too
        let rows = [row(25565, false)];
        let ports = Ports::build(&rows, any(), &HashSet::new(), &[(30000, 30001)]);
        assert_eq!(
            port_numbers(&ports.pick_move(&[25565, 25566]).unwrap()),
            vec![25565, 30000]
        );

        // not enough room
        let ports = Ports::build(&[], any(), &HashSet::new(), &[(30000, 30000)]);
        assert!(ports.pick_move(&[1, 2]).is_none());
    }

    #[test]
    fn an_added_port_follows_the_primary() {
        let rows = [row(30000, true), row(30001, true), row(30003, false)];
        let ports = Ports::build(
            &rows,
            any(),
            &HashSet::new(),
            &[(30000, 30005), (25000, 25001)],
        );

        assert_eq!(ports.pick_above(30000).map(Pick::port), Some(30002));
        assert_eq!(ports.pick_above(30004).map(Pick::port), Some(30005));
        // nothing above: the first port of the ranges
        assert_eq!(ports.pick_above(40000).map(Pick::port), Some(30002));
    }
}
