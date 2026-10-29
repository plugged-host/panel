//! plugged.host: what a user buys with tokens for one server (docs/features/server-upgrades.md).
//!
//! - **Keep awake**: while it runs, limbo neither stops, parks nor deletes the server as idle
//!   ([`crate::limbo`] leaves it out). Only sold while limbo is on.
//! - **Boost**: more memory and CPU for some hours, on top of the paid limits and outside the
//!   allowance, like burst cores: `servers.boost_memory` and `servers.boost_cpu` are added to the
//!   limits Wings gets ([`boosted_memory`], [`boosted_cpu`]), and a sync applies them to the running
//!   container. Sold only while the node has room for it: allocated memory against the node's, and
//!   its telemetry under the deployment pause line with the boost on top ([`load_fits`]).
//! - **Extra port, database, backup slot**: one more of that limit (`servers.allocation_limit`,
//!   `database_limit`, `backup_limit`) for some days, up to a few at once. When one ends the limit
//!   goes back down; nothing is deleted, the server just cannot add more while it is over.
//! - **Card colour**: a colour from a fixed palette for the server's card in the server list, bought
//!   once per server; changing it later is free.
//!
//! A purchase locks the server row and the buyer's row, checks the price the user saw and the
//! balance, and writes the ledger row and the effect in one transaction; the buyer pays. Buying a
//! keep-awake pass or a boost while one runs (or renewing an extra slot) adds its time to the end.
//! [`settle`] (every minute) ends what ran out and tells the buyer 48 hours before a pass or a slot
//! ends, once per end date. Short subdomain names take a bigger deposit ([`short_name_deposit`],
//! paid and refunded by `crate::subdomains`).

use crate::{
    economy::{self, Currency},
    models::{
        ByUuid, CreatableModel,
        server::Server,
        server_activity::{CreateServerActivityOptions, ServerActivity},
        user::User,
    },
    settings::upgrades::AppSettingsUpgrades,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

/// The ledger type of every purchase and renewal (shared/src/economy.rs).
pub const TYPE_UPGRADE: &str = "server_upgrade";

/// Hours before its end a pass or a slot is said to end soon.
pub const REMIND_HOURS: i64 = 48;

/// Upgrades ended (and reminders sent) per run at most.
const BATCH: i64 = 200;

/// Minutes a node's telemetry may be old for a boost to be sold on it.
const FRESH_MINUTES: i64 = 3;

/// The card colours, Mantine's palette (Theme v2's accent is `blue`).
pub const PALETTE: &[&str] = &[
    "blue", "cyan", "teal", "green", "yellow", "orange", "red", "pink", "grape", "violet",
];

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    KeepAwake,
    Boost,
    ExtraAllocation,
    ExtraDatabase,
    ExtraBackup,
    CardColor,
}

impl Kind {
    /// In the order the server's Upgrades section lists them.
    pub const ALL: [Kind; 6] = [
        Kind::KeepAwake,
        Kind::Boost,
        Kind::ExtraAllocation,
        Kind::ExtraDatabase,
        Kind::ExtraBackup,
        Kind::CardColor,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepAwake => "keep_awake",
            Self::Boost => "boost",
            Self::ExtraAllocation => "extra_allocation",
            Self::ExtraDatabase => "extra_database",
            Self::ExtraBackup => "extra_backup",
            Self::CardColor => "card_color",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// The server's limit an extra slot raises while it runs.
    pub fn limit_column(self) -> Option<&'static str> {
        match self {
            Self::ExtraAllocation => Some("allocation_limit"),
            Self::ExtraDatabase => Some("database_limit"),
            Self::ExtraBackup => Some("backup_limit"),
            _ => None,
        }
    }

    /// Several may run at once, each renewed on its own.
    pub fn stacks(self) -> bool {
        self.limit_column().is_some()
    }

    /// The name in the token history.
    fn title(self) -> &'static str {
        match self {
            Self::KeepAwake => "Keep awake",
            Self::Boost => "Boost",
            Self::ExtraAllocation => "Extra port",
            Self::ExtraDatabase => "Extra database",
            Self::ExtraBackup => "Extra backup slot",
            Self::CardColor => "Card colour",
        }
    }
}

/// What one purchase of a kind costs and gives now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    pub enabled: bool,
    pub tokens: i64,
    /// How long one purchase (or renewal) runs; `None` for good.
    pub duration: Option<chrono::Duration>,
    /// How many may run at once.
    pub max: i64,
}

pub fn terms(settings: &AppSettingsUpgrades, kind: Kind) -> Terms {
    let days = |days: i32| Some(chrono::Duration::days(days.max(1) as i64));
    let tokens = |tokens: i32| tokens.max(0) as i64;

    match kind {
        Kind::KeepAwake => Terms {
            enabled: settings.keep_awake_enabled,
            tokens: tokens(settings.keep_awake_tokens),
            duration: days(settings.keep_awake_days),
            max: 1,
        },
        Kind::Boost => Terms {
            enabled: settings.boost_enabled,
            tokens: tokens(settings.boost_tokens),
            duration: Some(chrono::Duration::hours(settings.boost_hours.max(1) as i64)),
            max: 1,
        },
        Kind::ExtraAllocation => Terms {
            enabled: settings.extra_allocation_enabled,
            tokens: tokens(settings.extra_allocation_tokens),
            duration: days(settings.extra_allocation_days),
            max: settings.extra_allocation_max.max(0) as i64,
        },
        Kind::ExtraDatabase => Terms {
            enabled: settings.extra_database_enabled,
            tokens: tokens(settings.extra_database_tokens),
            duration: days(settings.extra_database_days),
            max: settings.extra_database_max.max(0) as i64,
        },
        Kind::ExtraBackup => Terms {
            enabled: settings.extra_backup_enabled,
            tokens: tokens(settings.extra_backup_tokens),
            duration: days(settings.extra_backup_days),
            max: settings.extra_backup_max.max(0) as i64,
        },
        Kind::CardColor => Terms {
            enabled: settings.card_color_enabled,
            tokens: tokens(settings.card_color_tokens),
            duration: None,
            max: 1,
        },
    }
}

/// The least deposit a subdomain name of this length takes (`short_subdomain_two` for 2
/// characters, `short_subdomain_three` for 3), 0 for longer names or while short names cost
/// nothing extra. The deposit is the larger of this and the server's tier (crate::subdomains).
pub fn short_name_deposit(settings: &AppSettingsUpgrades, name: &str) -> i32 {
    short_length_deposit(settings, name.chars().count())
}

/// [`short_name_deposit`] for a name of `length` characters.
pub fn short_length_deposit(settings: &AppSettingsUpgrades, length: usize) -> i32 {
    if !settings.short_subdomain_enabled {
        return 0;
    }

    match length {
        0..=2 => settings.short_subdomain_two.max(0),
        3 => settings.short_subdomain_three.max(0),
        _ => 0,
    }
}

/// The server's columns this module keeps (`Server::upgrades`).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ServerUpgrades {
    /// A running boost's memory (MiB) and CPU (%), 0 without one.
    pub boost_memory: i64,
    pub boost_cpu: i32,
    /// The colour of its card in the server list, one of [`PALETTE`].
    pub card_color: Option<compact_str::CompactString>,
}

impl ServerUpgrades {
    pub fn map(prefix: &str, row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            boost_memory: row.try_get(format!("{prefix}boost_memory").as_str())?,
            boost_cpu: row.try_get(format!("{prefix}boost_cpu").as_str())?,
            card_color: row.try_get(format!("{prefix}card_color").as_str())?,
        })
    }
}

/// The columns, for `Server::base_columns`.
pub fn base_columns(prefix: &str) -> [(&'static str, compact_str::CompactString); 3] {
    [
        (
            "servers.boost_memory",
            compact_str::format_compact!("{prefix}boost_memory"),
        ),
        (
            "servers.boost_cpu",
            compact_str::format_compact!("{prefix}boost_cpu"),
        ),
        (
            "servers.card_color",
            compact_str::format_compact!("{prefix}card_color"),
        ),
    ]
}

/// The memory limit (MiB) with a running boost on top; unlimited (0) stays unlimited.
pub fn boosted_memory(memory: i64, boost: i64) -> i64 {
    if memory > 0 {
        memory.saturating_add(boost.max(0))
    } else {
        memory
    }
}

/// The CPU limit (%) with a running boost on top, before burst cores; unlimited (0) stays
/// unlimited.
pub fn boosted_cpu(cpu: i32, boost: i32) -> i32 {
    if cpu > 0 {
        cpu.saturating_add(boost.max(0))
    } else {
        cpu
    }
}

/// Whether the node's allocated memory (MiB) takes `add` more; a node without a limit always does.
pub fn allocated_fits(capacity: i64, used: i64, add: i64) -> bool {
    capacity <= 0 || add <= 0 || used.saturating_add(add) <= capacity
}

/// Whether the node stays under the deployment pause `line` (percent) with `add_memory` bytes and
/// `add_cpu` percent of one thread on top of what it uses now.
pub fn load_fits(
    line: f32,
    memory_used: i64,
    memory_total: i64,
    cpu_percent: Option<f32>,
    threads: Option<i32>,
    add_memory: i64,
    add_cpu: i64,
) -> bool {
    if memory_total <= 0 {
        return false;
    }
    let memory = memory_used.saturating_add(add_memory.max(0)) as f64 * 100.0 / memory_total as f64;
    if memory > line as f64 {
        return false;
    }

    if add_cpu > 0 {
        let (Some(cpu), Some(threads)) = (cpu_percent, threads) else {
            return false;
        };
        if threads <= 0 || cpu as f64 + add_cpu as f64 / threads as f64 > line as f64 {
            return false;
        }
    }

    true
}

/// Why an upgrade cannot be bought now.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    /// As many run as may.
    Max,
    /// The node has no room for a boost (or takes no new servers).
    NodeBusy,
    /// The node's load is not known right now.
    NodeUnknown,
    /// The server has no memory or CPU limit a boost could raise.
    Unlimited,
    /// The server is parked: only a keep-awake pass and the card colour are sold.
    Parked,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiOffer {
    pub kind: Kind,
    /// Tokens one purchase or renewal costs.
    pub tokens: i64,
    /// Hours one purchase or renewal runs; `null` for good.
    pub hours: Option<i64>,
    /// A boost's memory (MiB) and CPU (%); 0 for the others.
    pub memory: i64,
    pub cpu: i64,
    /// How many may run at once.
    pub max: i64,
    /// Why a new one cannot be bought now; renewing one that runs still works.
    pub unavailable: Option<Unavailable>,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiPurchase {
    pub uuid: uuid::Uuid,
    pub kind: Kind,
    /// When it ends; `null` for good (the card colour).
    pub until: Option<chrono::DateTime<chrono::Utc>>,
    pub created: chrono::DateTime<chrono::Utc>,
}

/// The server's Upgrades section.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiUpgrades {
    /// What may be bought (the kinds that are on; none while tokens are off).
    pub offers: Vec<ApiOffer>,
    /// What runs now, and the card colour once bought.
    pub purchases: Vec<ApiPurchase>,
    pub card_color: Option<compact_str::CompactString>,
    pub palette: Vec<&'static str>,
    /// The viewer's tokens: the buyer pays.
    pub balance: i64,
}

#[derive(Debug)]
pub enum UpgradeError {
    /// Not offered: off, tokens off, or limbo off for a keep-awake pass.
    Off,
    /// The price the user saw is not the price now.
    PriceChanged(i64),
    Short {
        cost: i64,
        balance: i64,
    },
    Unavailable(Unavailable),
    InvalidColor,
    /// The card already has a colour: changing it is free.
    Owned,
    /// No card colour was bought for the server.
    NotBought,
    /// The purchase to renew does not run (any more).
    NotFound,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for UpgradeError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<sqlx::Error> for UpgradeError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<crate::database::DatabaseError> for UpgradeError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(anyhow::anyhow!("{err}"))
    }
}

impl UpgradeError {
    pub fn message(&self) -> String {
        match self {
            Self::Off => "this upgrade is not offered".into(),
            Self::PriceChanged(price) => format!("the price changed to {price} tokens"),
            Self::Short { cost, balance } => {
                format!("this costs {cost} tokens, you have {balance}")
            }
            Self::Unavailable(Unavailable::Max) => {
                "this server has as many of these as it may have".into()
            }
            Self::Unavailable(Unavailable::NodeBusy) => {
                "the server's node has no room for a boost right now".into()
            }
            Self::Unavailable(Unavailable::NodeUnknown) => {
                "the server's node cannot be checked right now, try again in a few minutes".into()
            }
            Self::Unavailable(Unavailable::Unlimited) => {
                "the server has no memory or CPU limit to boost".into()
            }
            Self::Unavailable(Unavailable::Parked) => "the server is parked".into(),
            Self::InvalidColor => "pick one of the offered colours".into(),
            Self::Owned => "this server's card has a colour already, changing it is free".into(),
            Self::NotBought => "buy a card colour for this server first".into(),
            Self::NotFound => "this upgrade does not run any more".into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Off | Self::InvalidColor => 400,
            Self::NotFound => 404,
            Self::Other(_) => 500,
            _ => 409,
        }
    }
}

/// What the settings and the panel offer now: `(upgrades, tokens on, limbo on)`.
async fn offered(state: &crate::State) -> Result<(AppSettingsUpgrades, bool, bool), anyhow::Error> {
    let settings = state.settings.get().await?;

    Ok((
        settings.upgrades.clone(),
        settings.economy.tokens_enabled,
        settings.limbo.enabled,
    ))
}

fn sold(terms: &Terms, kind: Kind, tokens_enabled: bool, limbo_enabled: bool) -> bool {
    tokens_enabled && terms.enabled && (kind != Kind::KeepAwake || limbo_enabled)
}

/// Whether the server's node has room for a boost of `memory` MiB and `cpu` percent now.
async fn node_room(
    state: &crate::State,
    server: &Server,
    memory: i64,
    cpu: i64,
) -> Result<Option<Unavailable>, anyhow::Error> {
    let placement = state.settings.get().await?.placement.clone();
    let node = server.node.fetch_cached(&state.database).await?;

    // a node that takes no new servers takes no boosts either; a full disk is no reason
    if node.deployment_paused
        && node.deployment_paused_reason.as_deref()
            != Some(crate::models::node::placement::PAUSED_FOR_DISK)
    {
        return Ok(Some(Unavailable::NodeBusy));
    }

    // a part of the boost the server has no limit for adds nothing
    let memory = if server.memory > 0 { memory.max(0) } else { 0 };
    let cpu = if server.cpu > 0 { cpu.max(0) } else { 0 };

    let row = sqlx::query(
        r#"
        SELECT nodes.memory,
            COALESCE(SUM(servers.memory + servers.memory_overhead + servers.boost_memory), 0)::BIGINT
                AS used_memory
        FROM nodes
        LEFT JOIN servers ON servers.node_uuid = nodes.uuid
            AND servers.limbo_state IS DISTINCT FROM 'parked'
        WHERE nodes.uuid = $1
        GROUP BY nodes.uuid
        "#,
    )
    .bind(node.uuid)
    .fetch_optional(state.database.read())
    .await?;
    if let Some(row) = row
        && !allocated_fits(row.try_get("memory")?, row.try_get("used_memory")?, memory)
    {
        return Ok(Some(Unavailable::NodeBusy));
    }

    let Some(telemetry) = crate::node_telemetry::latest(&state.database, node.uuid).await? else {
        return Ok(Some(Unavailable::NodeUnknown));
    };
    let (Some(memory_used), Some(memory_total)) = (telemetry.memory_used, telemetry.memory_total)
    else {
        return Ok(Some(Unavailable::NodeUnknown));
    };
    if !telemetry.reachable
        || telemetry.sampled < chrono::Utc::now() - chrono::Duration::minutes(FRESH_MINUTES)
    {
        return Ok(Some(Unavailable::NodeUnknown));
    }

    let line = crate::models::node::placement::pause_line(
        &placement,
        Some(memory_total),
        telemetry.cpu_threads,
    );
    if !load_fits(
        line,
        memory_used,
        memory_total,
        telemetry.cpu_percent,
        telemetry.cpu_threads,
        memory.saturating_mul(1024 * 1024),
        cpu,
    ) {
        return Ok(Some(Unavailable::NodeBusy));
    }

    Ok(None)
}

/// What runs for the server now (and its card colour), oldest first.
async fn running(
    database: &crate::database::Database,
    server_uuid: uuid::Uuid,
) -> Result<Vec<ApiPurchase>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT uuid, kind, until, created FROM server_upgrades
        WHERE server_uuid = $1 AND ended IS NULL
        ORDER BY created, uuid",
    )
    .bind(server_uuid)
    .fetch_all(database.read())
    .await?;

    let mut purchases = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(kind) = Kind::parse(&row.try_get::<String, _>("kind")?) else {
            continue;
        };
        purchases.push(ApiPurchase {
            uuid: row.try_get("uuid")?,
            kind,
            until: row
                .try_get::<Option<chrono::NaiveDateTime>, _>("until")?
                .map(|until| until.and_utc()),
            created: row
                .try_get::<chrono::NaiveDateTime, _>("created")?
                .and_utc(),
        });
    }

    Ok(purchases)
}

/// The server's Upgrades section for `viewer`.
pub async fn view(
    state: &crate::State,
    server: &Server,
    viewer: &User,
) -> Result<ApiUpgrades, anyhow::Error> {
    let (settings, tokens_enabled, limbo_enabled) = offered(state).await?;
    let purchases = running(&state.database, server.uuid).await?;
    let balance: i64 = sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1")
        .bind(viewer.uuid)
        .fetch_optional(state.database.read())
        .await?
        .unwrap_or(0);
    let away = server.limbo.is_some_and(|limbo| limbo.away());

    let mut offers = Vec::new();
    for kind in Kind::ALL {
        let terms = terms(&settings, kind);
        if !sold(&terms, kind, tokens_enabled, limbo_enabled) {
            continue;
        }

        let runs = purchases
            .iter()
            .filter(|purchase| purchase.kind == kind)
            .count() as i64;
        let unavailable = if away && !matches!(kind, Kind::KeepAwake | Kind::CardColor) {
            Some(Unavailable::Parked)
        } else {
            match kind {
                Kind::Boost if server.memory <= 0 && server.cpu <= 0 => {
                    Some(Unavailable::Unlimited)
                }
                Kind::Boost if runs == 0 => {
                    node_room(
                        state,
                        server,
                        settings.boost_memory as i64,
                        settings.boost_cpu as i64,
                    )
                    .await?
                }
                kind if kind.stacks() && runs >= terms.max => Some(Unavailable::Max),
                _ => None,
            }
        };

        offers.push(ApiOffer {
            kind,
            tokens: terms.tokens,
            hours: terms.duration.map(|duration| duration.num_hours()),
            memory: if kind == Kind::Boost {
                settings.boost_memory.max(0) as i64
            } else {
                0
            },
            cpu: if kind == Kind::Boost {
                settings.boost_cpu.max(0) as i64
            } else {
                0
            },
            max: terms.max,
            unavailable,
        });
    }

    Ok(ApiUpgrades {
        offers,
        purchases,
        card_color: server.upgrades.card_color.clone(),
        palette: PALETTE.to_vec(),
        balance,
    })
}

pub struct PurchaseRequest {
    pub kind: Kind,
    /// The extra slot to renew; a new one without.
    pub renew: Option<uuid::Uuid>,
    /// The price the user saw.
    pub tokens: i64,
    /// The card colour picked.
    pub color: Option<compact_str::CompactString>,
}

#[derive(Debug, Clone)]
pub struct Bought {
    pub uuid: uuid::Uuid,
    pub kind: Kind,
    pub until: Option<chrono::NaiveDateTime>,
    pub tokens: i64,
    /// It made one that ran longer instead of adding a new one.
    pub renewed: bool,
}

/// Inserts a purchase, ending `seconds` from now (`None`: never).
async fn insert(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    server_uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
    kind: Kind,
    tokens: i64,
    data: serde_json::Value,
    seconds: Option<f64>,
) -> Result<(uuid::Uuid, Option<chrono::NaiveDateTime>), sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO server_upgrades (server_uuid, user_uuid, kind, amount, tokens, data, until)
        VALUES ($1, $2, $3, 1, $4, $5, CASE WHEN $6::DOUBLE PRECISION IS NULL THEN NULL
            ELSE NOW() + make_interval(secs => $6::DOUBLE PRECISION) END)
        RETURNING uuid, until",
    )
    .bind(server_uuid)
    .bind(user_uuid)
    .bind(kind.as_str())
    .bind(tokens.clamp(0, i32::MAX as i64) as i32)
    .bind(data)
    .bind(seconds)
    .fetch_one(&mut **transaction)
    .await?;

    Ok((row.try_get("uuid")?, row.try_get("until")?))
}

/// Makes a purchase that runs `seconds` longer, from its end (or from now, once past it).
async fn extend(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uuid: uuid::Uuid,
    tokens: i64,
    seconds: f64,
) -> Result<Option<chrono::NaiveDateTime>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE server_upgrades
        SET until = GREATEST(until, NOW()::TIMESTAMP) + make_interval(secs => $3::DOUBLE PRECISION),
            tokens = tokens + $2
        WHERE uuid = $1
        RETURNING until",
    )
    .bind(uuid)
    .bind(tokens.clamp(0, i32::MAX as i64) as i32)
    .bind(seconds)
    .fetch_one(&mut **transaction)
    .await
}

/// Buys (or renews) an upgrade for the server; `buyer` pays. See the module docs.
pub async fn buy(
    state: &crate::State,
    server: &Server,
    buyer: &User,
    request: PurchaseRequest,
) -> Result<Bought, UpgradeError> {
    let (settings, tokens_enabled, limbo_enabled) = offered(state).await?;
    let kind = request.kind;
    let terms = terms(&settings, kind);
    if !sold(&terms, kind, tokens_enabled, limbo_enabled) {
        return Err(UpgradeError::Off);
    }
    if request.tokens != terms.tokens {
        return Err(UpgradeError::PriceChanged(terms.tokens));
    }
    if server.limbo.is_some_and(|limbo| limbo.away())
        && !matches!(kind, Kind::KeepAwake | Kind::CardColor)
    {
        return Err(UpgradeError::Unavailable(Unavailable::Parked));
    }

    let color = match kind {
        Kind::CardColor => match request.color {
            Some(color) if PALETTE.contains(&color.as_str()) => Some(color),
            _ => return Err(UpgradeError::InvalidColor),
        },
        _ => None,
    };

    let boost = (
        settings.boost_memory.max(0) as i64,
        settings.boost_cpu.max(0),
    );
    if kind == Kind::Boost {
        if server.memory <= 0 && server.cpu <= 0 {
            return Err(UpgradeError::Unavailable(Unavailable::Unlimited));
        }

        // a boost that runs only gets longer; a new one needs room on the node
        let runs: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM server_upgrades
                WHERE server_uuid = $1 AND kind = $2 AND ended IS NULL
            )",
        )
        .bind(server.uuid)
        .bind(Kind::Boost.as_str())
        .fetch_one(state.database.write())
        .await?;
        if !runs && let Some(reason) = node_room(state, server, boost.0, boost.1 as i64).await? {
            return Err(UpgradeError::Unavailable(reason));
        }
    }

    let seconds = terms.duration.map(|duration| duration.num_seconds() as f64);
    let price = terms.tokens;

    let mut transaction = state.database.write().begin().await?;
    // the server first, as everything else that changes its limits does
    sqlx::query("SELECT 1 FROM servers WHERE uuid = $1 FOR UPDATE")
        .bind(server.uuid)
        .execute(&mut *transaction)
        .await?;
    let balance: i64 = sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(buyer.uuid)
        .fetch_one(&mut *transaction)
        .await?;
    if balance < price {
        return Err(UpgradeError::Short {
            cost: price,
            balance,
        });
    }

    let (uuid, until, renewed, synced) = match kind {
        Kind::CardColor => {
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM server_upgrades WHERE server_uuid = $1 AND kind = $2)",
            )
            .bind(server.uuid)
            .bind(kind.as_str())
            .fetch_one(&mut *transaction)
            .await?;
            if owned {
                return Err(UpgradeError::Owned);
            }

            let (uuid, until) = insert(
                &mut transaction,
                server.uuid,
                buyer.uuid,
                kind,
                price,
                serde_json::json!({ "color": color }),
                None,
            )
            .await?;
            sqlx::query("UPDATE servers SET card_color = $2 WHERE uuid = $1")
                .bind(server.uuid)
                .bind(color.as_deref())
                .execute(&mut *transaction)
                .await?;

            (uuid, until, false, false)
        }
        Kind::KeepAwake | Kind::Boost => {
            let seconds = seconds.unwrap_or_default();
            let runs: Option<uuid::Uuid> = sqlx::query_scalar(
                "SELECT uuid FROM server_upgrades
                WHERE server_uuid = $1 AND kind = $2 AND ended IS NULL
                ORDER BY until DESC NULLS LAST
                LIMIT 1
                FOR UPDATE",
            )
            .bind(server.uuid)
            .bind(kind.as_str())
            .fetch_optional(&mut *transaction)
            .await?;

            match runs {
                Some(uuid) => {
                    let until = extend(&mut transaction, uuid, price, seconds).await?;
                    (uuid, until, true, false)
                }
                None => {
                    let data = if kind == Kind::Boost {
                        serde_json::json!({ "memory": boost.0, "cpu": boost.1 })
                    } else {
                        serde_json::json!({})
                    };
                    let (uuid, until) = insert(
                        &mut transaction,
                        server.uuid,
                        buyer.uuid,
                        kind,
                        price,
                        data,
                        Some(seconds),
                    )
                    .await?;
                    if kind == Kind::Boost {
                        sqlx::query(
                            "UPDATE servers SET boost_memory = $2, boost_cpu = $3 WHERE uuid = $1",
                        )
                        .bind(server.uuid)
                        .bind(boost.0)
                        .bind(boost.1)
                        .execute(&mut *transaction)
                        .await?;
                    }

                    (uuid, until, false, kind == Kind::Boost)
                }
            }
        }
        Kind::ExtraAllocation | Kind::ExtraDatabase | Kind::ExtraBackup => {
            let seconds = seconds.unwrap_or_default();
            match request.renew {
                Some(renew) => {
                    let runs: Option<uuid::Uuid> = sqlx::query_scalar(
                        "SELECT uuid FROM server_upgrades
                        WHERE uuid = $1 AND server_uuid = $2 AND kind = $3 AND ended IS NULL
                        FOR UPDATE",
                    )
                    .bind(renew)
                    .bind(server.uuid)
                    .bind(kind.as_str())
                    .fetch_optional(&mut *transaction)
                    .await?;
                    let Some(uuid) = runs else {
                        return Err(UpgradeError::NotFound);
                    };

                    let until = extend(&mut transaction, uuid, price, seconds).await?;
                    (uuid, until, true, false)
                }
                None => {
                    let runs: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM server_upgrades
                        WHERE server_uuid = $1 AND kind = $2 AND ended IS NULL",
                    )
                    .bind(server.uuid)
                    .bind(kind.as_str())
                    .fetch_one(&mut *transaction)
                    .await?;
                    if runs >= terms.max {
                        return Err(UpgradeError::Unavailable(Unavailable::Max));
                    }

                    let (uuid, until) = insert(
                        &mut transaction,
                        server.uuid,
                        buyer.uuid,
                        kind,
                        price,
                        serde_json::json!({}),
                        Some(seconds),
                    )
                    .await?;
                    if let Some(column) = kind.limit_column() {
                        sqlx::query(sqlx::AssertSqlSafe(format!(
                            "UPDATE servers SET {column} = {column} + 1 WHERE uuid = $1"
                        )))
                        .bind(server.uuid)
                        .execute(&mut *transaction)
                        .await?;
                    }

                    (uuid, until, false, false)
                }
            }
        }
    };

    if price > 0
        && economy::change(
            &mut transaction,
            Currency::Tokens,
            buyer.uuid,
            -price,
            TYPE_UPGRADE,
            &format!("{}: {}", kind.title(), server.name),
            serde_json::json!({
                "server_uuid": server.uuid,
                "kind": kind.as_str(),
                "upgrade": uuid,
                "renewed": renewed,
            }),
        )
        .await?
        .is_none()
    {
        return Err(UpgradeError::Short {
            cost: price,
            balance,
        });
    }
    transaction.commit().await?;

    Server::invalidate_cached(&state.database, server.uuid).await;
    if synced {
        sync(state, server.uuid).await;
    }

    Ok(Bought {
        uuid,
        kind,
        until,
        tokens: price,
        renewed,
    })
}

/// Changes the colour of the server's card, free once a colour was bought; `None` takes it off.
pub async fn set_color(
    state: &crate::State,
    server: &Server,
    color: Option<compact_str::CompactString>,
) -> Result<(), UpgradeError> {
    if let Some(color) = &color
        && !PALETTE.contains(&color.as_str())
    {
        return Err(UpgradeError::InvalidColor);
    }

    let bought: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM server_upgrades WHERE server_uuid = $1 AND kind = $2)",
    )
    .bind(server.uuid)
    .bind(Kind::CardColor.as_str())
    .fetch_one(state.database.read())
    .await?;
    if !bought {
        return Err(UpgradeError::NotBought);
    }

    sqlx::query("UPDATE servers SET card_color = $2 WHERE uuid = $1")
        .bind(server.uuid)
        .bind(color.as_deref())
        .execute(state.database.write())
        .await?;
    Server::invalidate_cached(&state.database, server.uuid).await;

    Ok(())
}

/// Sends the server's limits to Wings, which applies them to the running container. A parked
/// server is not on its node: its limits go along when it is restored.
async fn sync(state: &crate::State, server_uuid: uuid::Uuid) {
    match Server::by_uuid_optional(&state.database, server_uuid).await {
        Ok(Some(server)) if !server.limbo.is_some_and(|limbo| limbo.away()) => {
            server.batch_sync(state).await;
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(server = %server_uuid, "failed to load a server to sync its boost: {err:#?}");
        }
    }
}

async fn log(state: &crate::State, server_uuid: uuid::Uuid, event: &str, data: serde_json::Value) {
    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid,
            user_uuid: None,
            impersonator_uuid: None,
            api_key_uuid: None,
            schedule_uuid: None,
            event: event.into(),
            ip: None,
            data,
            created: None,
        },
    )
    .await
    {
        tracing::warn!(server = %server_uuid, "failed to log {event}: {err:#?}");
    }
}

/// Ends one purchase that ran out: its limit goes back down (never below 0, nothing is
/// deleted), a boost comes off and Wings gets the limits.
async fn end(
    state: &crate::State,
    uuid: uuid::Uuid,
    server_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    // the server first, as a purchase locks it before its upgrades
    sqlx::query("SELECT 1 FROM servers WHERE uuid = $1 FOR UPDATE")
        .bind(server_uuid)
        .execute(&mut *transaction)
        .await?;
    let Some(row) = sqlx::query(
        "UPDATE server_upgrades SET ended = NOW()
        WHERE uuid = $1 AND ended IS NULL AND until <= NOW()
        RETURNING kind, amount",
    )
    .bind(uuid)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        // renewed (or ended) meanwhile
        return Ok(());
    };

    let kind = Kind::parse(&row.try_get::<String, _>("kind")?);
    let amount: i32 = row.try_get("amount")?;
    match kind {
        Some(Kind::Boost) => {
            sqlx::query("UPDATE servers SET boost_memory = 0, boost_cpu = 0 WHERE uuid = $1")
                .bind(server_uuid)
                .execute(&mut *transaction)
                .await?;
        }
        Some(kind) => {
            if let Some(column) = kind.limit_column() {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE servers SET {column} = GREATEST({column} - $2, 0) WHERE uuid = $1"
                )))
                .bind(server_uuid)
                .bind(amount.max(0))
                .execute(&mut *transaction)
                .await?;
            }
        }
        None => {}
    }
    transaction.commit().await?;

    Server::invalidate_cached(&state.database, server_uuid).await;
    if let Some(kind) = kind {
        log(
            state,
            server_uuid,
            "server:upgrade.end",
            serde_json::json!({ "kind": kind.as_str(), "upgrade": uuid }),
        )
        .await;
    }
    if kind == Some(Kind::Boost) {
        sync(state, server_uuid).await;
    }

    Ok(())
}

/// Tells whoever bought a pass or a slot (the owner when the buyer has no access any more) that
/// it ends within [`REMIND_HOURS`], once per end date, with what renewing it costs. A boost is
/// shorter than that and is left out.
async fn remind(state: &crate::State) -> Result<(), anyhow::Error> {
    let (settings, tokens_enabled, limbo_enabled) = offered(state).await?;

    let rows = sqlx::query(
        r#"
        SELECT server_upgrades.uuid, server_upgrades.server_uuid, server_upgrades.kind,
            server_upgrades.until, servers.name,
            CASE WHEN server_upgrades.user_uuid = servers.owner_uuid OR EXISTS (
                    SELECT 1 FROM server_subusers
                    WHERE server_subusers.server_uuid = servers.uuid
                        AND server_subusers.user_uuid = server_upgrades.user_uuid
                )
                THEN server_upgrades.user_uuid ELSE servers.owner_uuid END AS recipient
        FROM server_upgrades
        JOIN servers ON servers.uuid = server_upgrades.server_uuid
        WHERE server_upgrades.ended IS NULL
            AND server_upgrades.kind <> 'boost'
            AND server_upgrades.until > NOW()
            AND server_upgrades.until <= NOW() + make_interval(hours => $1)
            AND server_upgrades.reminded_until IS DISTINCT FROM server_upgrades.until
        ORDER BY server_upgrades.until
        LIMIT $2
        "#,
    )
    .bind(REMIND_HOURS as i32)
    .bind(BATCH)
    .fetch_all(state.database.write())
    .await?;

    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let Some(kind) = Kind::parse(&row.try_get::<String, _>("kind")?) else {
            continue;
        };
        let until: chrono::NaiveDateTime = row.try_get("until")?;

        let marked = sqlx::query(
            "UPDATE server_upgrades SET reminded_until = until
            WHERE uuid = $1 AND until = $2 AND reminded_until IS DISTINCT FROM until",
        )
        .bind(uuid)
        .bind(until)
        .execute(state.database.write())
        .await?
        .rows_affected();
        if marked == 0 {
            continue;
        }

        let terms = terms(&settings, kind);
        let renew = sold(&terms, kind, tokens_enabled, limbo_enabled).then_some(terms.tokens);
        crate::notifications::send(
            state,
            row.try_get("recipient")?,
            Some(row.try_get("server_uuid")?),
            crate::notifications::UPGRADE_ENDING,
            serde_json::json!({
                "server": row.try_get::<String, _>("name")?,
                "upgrade": kind.as_str(),
                "until": until.and_utc().to_rfc3339(),
                "tokens": renew,
            }),
        )
        .await;
    }

    Ok(())
}

/// The `settle_upgrades` task, every minute: ends what ran out and sends the ends-soon
/// notifications.
pub async fn settle(state: &crate::State) -> Result<(), anyhow::Error> {
    let due = sqlx::query(
        "SELECT uuid, server_uuid FROM server_upgrades
        WHERE ended IS NULL AND until <= NOW()
        ORDER BY until
        LIMIT $1",
    )
    .bind(BATCH)
    .fetch_all(state.database.write())
    .await?;

    for row in due {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let server_uuid: uuid::Uuid = row.try_get("server_uuid")?;
        if let Err(err) = end(state, uuid, server_uuid).await {
            tracing::warn!(server = %server_uuid, upgrade = %uuid, "failed to end a server upgrade: {err:#}");
        }
    }

    if let Err(err) = remind(state).await {
        tracing::warn!("failed to send the server upgrades' ends-soon notifications: {err:#}");
    }

    Ok(())
}

/// Tells a server's owner that limbo stopped it for being idle (`crate::limbo`), when it will be
/// parked, and what a keep-awake pass costs when one is sold. Never fails the caller.
pub async fn notify_idle_stop(state: &crate::State, server_uuid: uuid::Uuid, idle_minutes: i64) {
    if let Err(err) = idle_stop_notification(state, server_uuid, idle_minutes).await {
        tracing::warn!(server = %server_uuid, "failed to tell the owner of an idle stop: {err:#}");
    }
}

async fn idle_stop_notification(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    idle_minutes: i64,
) -> Result<(), anyhow::Error> {
    let (settings, tokens_enabled, limbo_enabled, park_minutes) = {
        let settings = state.settings.get().await?;
        (
            settings.upgrades.clone(),
            settings.economy.tokens_enabled,
            settings.limbo.enabled,
            settings.limbo.park_minutes as i64,
        )
    };

    let Some(row) = sqlx::query("SELECT owner_uuid, name FROM servers WHERE uuid = $1")
        .bind(server_uuid)
        .fetch_optional(state.database.read())
        .await?
    else {
        return Ok(());
    };

    let terms = terms(&settings, Kind::KeepAwake);
    let offer = sold(&terms, Kind::KeepAwake, tokens_enabled, limbo_enabled);
    crate::notifications::send(
        state,
        row.try_get("owner_uuid")?,
        Some(server_uuid),
        crate::notifications::SERVER_IDLE_STOPPED,
        serde_json::json!({
            "server": row.try_get::<String, _>("name")?,
            "hours": (idle_minutes / 60).max(1),
            "park": ((park_minutes - idle_minutes).max(0) + 59) / 60,
            "tokens": offer.then_some(terms.tokens),
            "days": offer.then_some(settings.keep_awake_days.max(1)),
        }),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_only_extra_slots_stack() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::parse("nothing"), None);
        assert!(Kind::ExtraAllocation.stacks());
        assert!(Kind::ExtraDatabase.stacks());
        assert!(Kind::ExtraBackup.stacks());
        assert!(!Kind::KeepAwake.stacks());
        assert!(!Kind::Boost.stacks());
        assert!(!Kind::CardColor.stacks());
        assert_eq!(Kind::ExtraBackup.limit_column(), Some("backup_limit"));
    }

    #[test]
    fn the_defaults_are_the_prices() {
        let settings = AppSettingsUpgrades::default();

        let keep = terms(&settings, Kind::KeepAwake);
        assert_eq!(keep.tokens, 3);
        assert_eq!(keep.duration, Some(chrono::Duration::days(7)));

        let boost = terms(&settings, Kind::Boost);
        assert_eq!(boost.tokens, 3);
        assert_eq!(boost.duration, Some(chrono::Duration::hours(24)));

        let port = terms(&settings, Kind::ExtraAllocation);
        assert_eq!((port.tokens, port.max), (2, 3));
        assert_eq!(port.duration, Some(chrono::Duration::days(30)));

        let color = terms(&settings, Kind::CardColor);
        assert_eq!((color.tokens, color.duration), (2, None));
    }

    #[test]
    fn keep_awake_is_only_sold_with_tokens_and_limbo_on() {
        let settings = AppSettingsUpgrades::default();
        let keep = terms(&settings, Kind::KeepAwake);
        let boost = terms(&settings, Kind::Boost);

        assert!(sold(&keep, Kind::KeepAwake, true, true));
        assert!(!sold(&keep, Kind::KeepAwake, true, false));
        assert!(!sold(&keep, Kind::KeepAwake, false, true));
        assert!(sold(&boost, Kind::Boost, true, false));
    }

    #[test]
    fn short_names_take_the_bigger_deposit() {
        let mut settings = AppSettingsUpgrades::default();

        assert_eq!(short_name_deposit(&settings, "ab"), 30);
        assert_eq!(short_name_deposit(&settings, "abc"), 15);
        assert_eq!(short_name_deposit(&settings, "abcd"), 0);

        settings.short_subdomain_enabled = false;
        assert_eq!(short_name_deposit(&settings, "ab"), 0);
    }

    #[test]
    fn a_boost_adds_to_limits_but_not_to_unlimited_ones() {
        assert_eq!(boosted_memory(4096, 2048), 6144);
        assert_eq!(boosted_memory(0, 2048), 0);
        assert_eq!(boosted_memory(4096, 0), 4096);
        assert_eq!(boosted_cpu(200, 100), 300);
        assert_eq!(boosted_cpu(0, 100), 0);
    }

    #[test]
    fn the_node_needs_room_under_the_line() {
        const GIB: i64 = 1024 * 1024 * 1024;

        // 32 GiB, 20 used: 2 GiB more is 68.75 %
        assert!(load_fits(
            85.0,
            20 * GIB,
            32 * GIB,
            Some(40.0),
            Some(8),
            2 * GIB,
            100
        ));
        // 26 used: 28 of 32 is 87.5 %
        assert!(!load_fits(
            85.0,
            26 * GIB,
            32 * GIB,
            Some(40.0),
            Some(8),
            2 * GIB,
            100
        ));
        // a whole thread of 8 is 12.5 % more: 75 + 12.5 is over
        assert!(!load_fits(
            85.0,
            20 * GIB,
            32 * GIB,
            Some(75.0),
            Some(8),
            2 * GIB,
            100
        ));
        // CPU unknown: only without a CPU boost
        assert!(!load_fits(
            85.0,
            20 * GIB,
            32 * GIB,
            None,
            None,
            2 * GIB,
            100
        ));
        assert!(load_fits(85.0, 20 * GIB, 32 * GIB, None, None, 2 * GIB, 0));
        // no memory figure
        assert!(!load_fits(85.0, 0, 0, Some(10.0), Some(8), 0, 0));

        assert!(allocated_fits(0, 100_000, 2048));
        assert!(allocated_fits(16384, 14336, 2048));
        assert!(!allocated_fits(16384, 14337, 2048));
    }
}
