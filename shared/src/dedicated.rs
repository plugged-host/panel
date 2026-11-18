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

/// Whether any public dedicated plan is offered: players get the dedicated plans page and its
/// sidebar entry (`/api/settings`, `games.dedicated_offered`). Cached for a minute; false when it
/// cannot be told.
pub async fn offered(state: &crate::State) -> bool {
    let database = state.database.clone();
    state
        .cache
        .cached("dedicated::offered", 60, || async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM plans WHERE plans.dedicated AND plans.public)",
            )
            .fetch_one(database.read())
            .await
        })
        .await
        .unwrap_or(false)
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

/// A public dedicated plan as players see it. With a price and noprofit.space connected, players
/// order it themselves and pay monthly with Funds ([`order`]); otherwise through a support ticket.
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

// ---------------------------------------------------------------------------------------------
// Paid by the player: ordered here, paid monthly with noprofit.space Funds

/// Days a paid month runs.
pub const MONTH_DAYS: i64 = 30;
/// Days an unpaid server stays suspended before it is deleted (`suspension::purge_due`).
pub const GRACE_DAYS: i64 = 3;

#[derive(Debug)]
pub enum OrderError {
    /// Not a public dedicated plan with a price.
    NotOffered,
    /// noprofit.space is not connected or the account is not linked to it: no Funds to pay with.
    NoFunds,
    /// The account has fewer Funds than a month costs.
    Short,
    /// noprofit.space did not answer.
    Unavailable,
    /// No node of the plan in the location has room for it now.
    SoldOut,
    Create(crate::server_creation::CreateError),
}

impl OrderError {
    pub fn message(&self) -> String {
        match self {
            Self::NotOffered => "this plan cannot be ordered".into(),
            Self::NoFunds => "paid plans are paid with noprofit.space Funds".into(),
            Self::Short => "not enough Funds: add Funds on noprofit.space".into(),
            Self::Unavailable => "noprofit.space did not answer, try again in a moment".into(),
            Self::SoldOut => "this plan is sold out in this location".into(),
            Self::Create(err) => err.message(),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotOffered => 400,
            Self::NoFunds | Self::Short => 402,
            Self::Unavailable => 503,
            Self::SoldOut => 409,
            Self::Create(err) => err.status(),
        }
    }
}

impl From<anyhow::Error> for OrderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Create(crate::server_creation::CreateError::Other(err))
    }
}

/// A paid server's month, as its players see it (`/api/client/servers/{server}/billing`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ApiSubscription {
    /// Euro cents a month.
    pub price: i32,
    pub paid_until: chrono::DateTime<chrono::Utc>,
    /// Paid again from the owner's Funds when the month ends; off, the server ends then.
    pub renews: bool,
    /// The month is unpaid: the server is suspended since then and deleted at `delete_at`.
    pub unpaid_since: Option<chrono::DateTime<chrono::Utc>>,
    pub delete_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow)]
struct Subscription {
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
    price: i32,
    paid_until: chrono::NaiveDateTime,
    renews: bool,
    unpaid_since: Option<chrono::NaiveDateTime>,
}

const SUBSCRIPTION_COLUMNS: &str =
    "server_uuid, user_uuid, price, paid_until, renews, unpaid_since";

async fn subscription_row(
    state: &crate::State,
    server_uuid: uuid::Uuid,
) -> Result<Option<Subscription>, sqlx::Error> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUBSCRIPTION_COLUMNS} FROM server_subscriptions WHERE server_uuid = $1"
    )))
    .bind(server_uuid)
    .fetch_optional(state.database.write())
    .await
}

/// The server's paid month, `None` for a server nobody pays for here (free, or made by an admin).
pub async fn subscription(
    state: &crate::State,
    server_uuid: uuid::Uuid,
) -> Result<Option<ApiSubscription>, anyhow::Error> {
    Ok(subscription_row(state, server_uuid)
        .await?
        .map(|row| ApiSubscription {
            price: row.price,
            paid_until: row.paid_until.and_utc(),
            renews: row.renews,
            unpaid_since: row.unpaid_since.map(|at| at.and_utc()),
            delete_at: row
                .unpaid_since
                .map(|at| (at + chrono::Duration::days(GRACE_DAYS)).and_utc()),
        }))
}

/// noprofit.space and the account's link to it: what Funds are paid with.
async fn payer(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<Option<(crate::noprofit::Config, crate::noprofit::Link)>, anyhow::Error> {
    let Some(config) = state.settings.get().await?.noprofit.config() else {
        return Ok(None);
    };
    let link = crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
        .await?
        .filter(|link| link.deleted.is_none());

    Ok(link.map(|link| (config, link)))
}

fn euros(cents: i32) -> String {
    format!("€{}.{:02}", cents / 100, cents % 100)
}

/// Orders a public dedicated plan for `user`: takes the first month from their Funds, then makes
/// the server (crate::server_creation::create_dedicated). A server that cannot be made gives the
/// month back.
pub async fn order(
    state: &crate::State,
    user: &crate::models::user::User,
    request: crate::server_creation::CreateRequest,
) -> Result<crate::models::server::Server, OrderError> {
    use crate::models::ByUuid;
    use crate::server_creation::CreateError;

    let plan = match request.plan_uuid {
        Some(plan_uuid) => Plan::by_uuid_optional(&state.database, plan_uuid)
            .await
            .map_err(anyhow::Error::from)?,
        None => None,
    };
    let Some((plan, price)) = plan
        .filter(|plan| plan.dedicated && plan.public)
        .and_then(|plan| {
            plan.price
                .filter(|price| *price > 0)
                .map(|price| (plan, price))
        })
    else {
        return Err(OrderError::NotOffered);
    };
    if user.suspended || user.frozen {
        return Err(OrderError::Create(CreateError::AccountBlocked));
    }
    let Some((noprofit, link)) = payer(state, user.uuid).await? else {
        return Err(OrderError::NoFunds);
    };

    let Some(_lock) = crate::allowance::lock(state, user.uuid).await else {
        return Err(OrderError::Create(CreateError::Busy));
    };

    // sold out: nothing is charged
    let available = offers(state).await?.into_iter().any(|offer| {
        offer.uuid == plan.uuid
            && offer
                .locations
                .iter()
                .any(|location| location.uuid == request.location_uuid && location.available)
    });
    if !available {
        return Err(OrderError::SoldOut);
    }

    let order_uuid = uuid::Uuid::new_v4();
    let description = format!(
        "{}: {} for {} days",
        request.name.trim(),
        plan.name,
        MONTH_DAYS
    );
    match noprofit
        .spend_funds(
            &state.client,
            link.account_id,
            price as i64,
            &description,
            &format!("dedicated:order:{order_uuid}"),
        )
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return Err(OrderError::Short),
        Err(err) => {
            tracing::warn!(user = %user.uuid, "noprofit.space did not take the Funds for a paid server: {err:#}");
            return Err(OrderError::Unavailable);
        }
    }

    let server = match crate::server_creation::create_dedicated(state, user, request, &plan).await {
        Ok(server) => server,
        Err(err) => {
            if let Err(refund) = noprofit
                .refund_funds(
                    &state.client,
                    link.account_id,
                    price as i64,
                    &format!("Refund: {description} (the server could not be made)"),
                    &format!("dedicated:refund:{order_uuid}"),
                )
                .await
            {
                tracing::error!(user = %user.uuid, account = link.account_id, price, order = %order_uuid,
                    "a paid server could not be made and its Funds could not be given back, refund them by hand: {refund:#}");
            }

            return Err(OrderError::Create(err));
        }
    };

    if let Err(err) = sqlx::query(
        "INSERT INTO server_subscriptions (server_uuid, user_uuid, plan_uuid, price, paid_until)
        VALUES ($1, $2, $3, $4, NOW() + make_interval(days => $5))",
    )
    .bind(server.uuid)
    .bind(user.uuid)
    .bind(plan.uuid)
    .bind(price)
    .bind(MONTH_DAYS as i32)
    .execute(state.database.write())
    .await
    {
        tracing::error!(server = %server.uuid, "a paid server was made but its month was not recorded: {err:#}");
    }

    Ok(server)
}

/// What paying a month came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Renewal {
    Paid,
    Short,
    /// noprofit.space is not connected, the account not linked, or noprofit.space did not answer.
    Unavailable,
}

/// Pays the month that ended (`paid_until`) from the owner's Funds, once: the key is the server
/// and that end. A paid server that was suspended for it is back.
async fn pay(state: &crate::State, row: &Subscription) -> Result<Renewal, anyhow::Error> {
    use crate::models::{ByUuid, UpdatableModel};

    let Some((noprofit, link)) = payer(state, row.user_uuid).await? else {
        return Ok(Renewal::Unavailable);
    };
    let Some(mut server) =
        crate::models::server::Server::by_uuid_optional(&state.database, row.server_uuid).await?
    else {
        return Ok(Renewal::Unavailable);
    };

    let key = format!(
        "dedicated:renew:{}:{}",
        row.server_uuid,
        row.paid_until.and_utc().timestamp()
    );
    let description = format!("{}: {} more days", server.name, MONTH_DAYS);
    match noprofit
        .spend_funds(
            &state.client,
            link.account_id,
            row.price as i64,
            &description,
            &key,
        )
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(Renewal::Short),
        Err(err) => {
            tracing::warn!(server = %row.server_uuid, "noprofit.space did not take a paid server's month: {err:#}");
            return Ok(Renewal::Unavailable);
        }
    }

    // on time, the next month follows on; late, it starts now
    sqlx::query(
        "UPDATE server_subscriptions
        SET paid_until = GREATEST(paid_until, NOW()) + make_interval(days => $2), unpaid_since = NULL, renews = TRUE
        WHERE server_uuid = $1 AND paid_until = $3",
    )
    .bind(row.server_uuid)
    .bind(MONTH_DAYS as i32)
    .bind(row.paid_until)
    .execute(state.database.write())
    .await?;

    if server.suspension.as_ref().is_some_and(|suspension| {
        suspension.kind == crate::models::server::suspension::SuspensionKind::Unpaid
    }) {
        let options = crate::models::server::UpdateServerOptions {
            suspended: Some(false),
            ..Default::default()
        };
        if let Err(err) = server.update(state, options).await {
            tracing::warn!(server = %row.server_uuid, "failed to lift a paid server's unpaid suspension: {err:#?}");
        }
    }

    Ok(Renewal::Paid)
}

/// The month ended unpaid (too few Funds, or not renewed): the server is suspended and deleted
/// after [`GRACE_DAYS`] unless it is paid first.
async fn lapse(
    state: &crate::State,
    row: &Subscription,
    reason: String,
) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;

    sqlx::query(
        "UPDATE server_subscriptions SET unpaid_since = NOW()
        WHERE server_uuid = $1 AND unpaid_since IS NULL",
    )
    .bind(row.server_uuid)
    .execute(state.database.write())
    .await?;

    if let Some(mut server) =
        crate::models::server::Server::by_uuid_optional(&state.database, row.server_uuid).await?
    {
        server
            .suspend(
                state,
                crate::models::server::suspension::Suspend {
                    kind: crate::models::server::suspension::SuspensionKind::Unpaid,
                    reason: Some(reason),
                    by: None,
                    delete_at: Some(chrono::Utc::now() + chrono::Duration::days(GRACE_DAYS)),
                },
            )
            .await?;
    }

    Ok(())
}

/// The `settle_paid_servers` task: every paid server whose month ended is paid again from its
/// owner's Funds, or suspended (and deleted [`GRACE_DAYS`] later by the suspension purge). An
/// unpaid one is tried again each time, so Funds added later bring it back by themselves.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    if state.settings.get().await?.noprofit.config().is_none() {
        return Ok(());
    }

    let due: Vec<Subscription> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUBSCRIPTION_COLUMNS} FROM server_subscriptions
        WHERE paid_until <= NOW()
        ORDER BY paid_until
        LIMIT 200"
    )))
    .fetch_all(state.database.read())
    .await?;

    for row in due {
        let result = if row.renews {
            match pay(state, &row).await {
                Ok(Renewal::Short) if row.unpaid_since.is_none() => {
                    lapse(
                        state,
                        &row,
                        format!(
                            "Its month ({}) could not be paid: add Funds on noprofit.space and it comes back.",
                            euros(row.price)
                        ),
                    )
                    .await
                }
                Ok(_) => Ok(()),
                Err(err) => Err(err),
            }
        } else if row.unpaid_since.is_none() {
            lapse(
                state,
                &row,
                "Its renewal was cancelled: renew it from this page to keep it.".into(),
            )
            .await
        } else {
            Ok(())
        };

        if let Err(err) = result {
            tracing::warn!(server = %row.server_uuid, "failed to settle a paid server's month: {err:#}");
        }
    }

    Ok(())
}

/// The owner turns renewal on or off. Turning it on for a month that already ended pays it now.
pub async fn set_renews(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    renews: bool,
) -> Result<Option<Renewal>, anyhow::Error> {
    sqlx::query("UPDATE server_subscriptions SET renews = $2 WHERE server_uuid = $1")
        .bind(server_uuid)
        .bind(renews)
        .execute(state.database.write())
        .await?;

    let Some(row) = subscription_row(state, server_uuid).await? else {
        return Ok(None);
    };
    if renews && row.paid_until <= chrono::Utc::now().naive_utc() {
        return Ok(Some(pay(state, &row).await?));
    }

    Ok(None)
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
    fn prices_read_as_euros() {
        assert_eq!(euros(1000), "€10.00");
        assert_eq!(euros(499), "€4.99");
        assert_eq!(euros(5), "€0.05");
    }

    #[test]
    fn order_errors_have_a_status() {
        assert_eq!(OrderError::Short.status(), 402);
        assert_eq!(OrderError::SoldOut.status(), 409);
        assert_eq!(OrderError::NotOffered.status(), 400);
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
