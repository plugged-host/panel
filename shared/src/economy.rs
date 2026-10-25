//! plugged.host: credits, tokens, the memberships and the Resource Store
//! (docs/features/economy.md).
//!
//! Credits buy CPU, memory and disk for good. Without noprofit.space they are the `users.credits`
//! column, the store prices with [`crate::pricing`] and what was bought is `users.extra_*`, added
//! to the games allowance. With noprofit.space connected credits and the store are
//! noprofit.space's: the balance is the account's snapshot, the store is its store, and what was
//! bought arrives with the account's plan.
//!
//! Tokens are earned from sponsored links ([`crate::earn`]) and buy the memberships, which are
//! always the panel's: Plus once, for good (the `plus` locations), PlusPlus for a while with
//! tokens or credits (the `premium` locations, more CPU). When PlusPlus runs out its servers leave
//! the premium locations and the CPU it added comes off ([`settle_memberships`]).
//!
//! Both memberships give **burst cores**: CPU above each server's paid limit (100 % per core) that
//! a server uses while its node has it to spare. They are outside the allowance and the store. The
//! account's count is kept in `users.burst_cores` ([`sync_burst`]), so every server read has it;
//! Wings gets the raised limit and a CPU weight from the paid limit alone, so paid CPU wins when
//! the node is busy ([`cpu_limit_with_burst`], [`cpu_weight`]).

use crate::{
    allowance::Resources,
    models::{ByUuid, user::User},
    pricing::{CatalogEntry, MAX_STEPS, Pricing, StoreResource, UNAFFORDABLE},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

/// The location tier Plus opens (PlusPlus too).
pub const TIER_PLUS: &str = "plus";
/// The location tier only PlusPlus opens.
pub const TIER_PREMIUM: &str = "premium";

/// Ledger types.
pub const TYPE_STARTING: &str = "starting";
pub const TYPE_DAILY: &str = "daily";
pub const TYPE_STORE: &str = "resource_purchase";
pub const TYPE_PLUS: &str = "plus";
pub const TYPE_PLUSPLUS: &str = "plusplus";
pub const TYPE_ADMIN: &str = "admin";

/// Transactions shown on a page.
pub const HISTORY: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    Credits,
    Tokens,
}

impl Currency {
    fn column(self) -> &'static str {
        match self {
            Self::Credits => "credits",
            Self::Tokens => "tokens",
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Credits => "user_credit_transactions",
            Self::Tokens => "user_token_transactions",
        }
    }
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Pay {
    Tokens,
    Credits,
}

/// Where credits and the store live.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Panel,
    Noprofit,
}

#[derive(Clone, Debug, Default)]
pub struct Membership {
    pub plus_since: Option<chrono::NaiveDateTime>,
    pub plusplus_until: Option<chrono::NaiveDateTime>,
    pub plusplus_purchases: i32,
}

impl Membership {
    pub fn has_plus(&self) -> bool {
        self.plus_since.is_some()
    }

    pub fn has_plusplus(&self, now: chrono::NaiveDateTime) -> bool {
        self.plusplus_until.is_some_and(|until| until > now)
    }

    /// The account's place in the start queue: 2 with PlusPlus, 1 with Plus, 0 without.
    pub fn priority(&self, now: chrono::NaiveDateTime) -> u8 {
        if self.has_plusplus(now) {
            2
        } else if self.has_plus() {
            1
        } else {
            0
        }
    }

    /// Whether a location of `tier` is open to the account. A tier the economy does not know
    /// stays closed to players.
    pub fn unlocks(&self, tier: Option<&str>, now: chrono::NaiveDateTime) -> bool {
        match tier.filter(|tier| !tier.is_empty()) {
            None => true,
            Some(TIER_PLUS) => self.has_plus() || self.has_plusplus(now),
            Some(TIER_PREMIUM) => self.has_plusplus(now),
            Some(_) => false,
        }
    }

    /// The burst cores each of the account's servers gets: PlusPlus's count while it runs (the
    /// total, not added to Plus's), else Plus's.
    pub fn burst_cores(
        &self,
        settings: &crate::settings::economy::AppSettingsEconomy,
        now: chrono::NaiveDateTime,
    ) -> i16 {
        let cores = if self.has_plusplus(now) {
            settings.plusplus_burst_cores
        } else if self.has_plus() {
            settings.plus_burst_cores
        } else {
            0
        };

        cores.clamp(0, i16::MAX as i32) as i16
    }

    /// When PlusPlus bought now runs out: bought time adds to what is left.
    pub fn extended_until(&self, now: chrono::NaiveDateTime, days: i64) -> chrono::NaiveDateTime {
        let from = self
            .plusplus_until
            .filter(|until| *until > now)
            .unwrap_or(now);

        from + chrono::Duration::days(days.max(1))
    }
}

pub async fn membership(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<Membership, sqlx::Error> {
    let row = sqlx::query(
        "SELECT plus_since, plusplus_until, plusplus_purchases FROM user_memberships
        WHERE user_uuid = $1",
    )
    .bind(user_uuid)
    .fetch_optional(database.write())
    .await?;

    match row {
        Some(row) => Ok(Membership {
            plus_since: row.try_get("plus_since")?,
            plusplus_until: row.try_get("plusplus_until")?,
            plusplus_purchases: row.try_get("plusplus_purchases")?,
        }),
        None => Ok(Membership::default()),
    }
}

/// The CPU limit (%) Wings enforces: the paid limit plus 100 % per burst core. Unlimited (0) stays
/// unlimited.
pub fn cpu_limit_with_burst(cpu: i32, burst_cores: i16) -> i64 {
    if cpu > 0 {
        cpu as i64 + burst_cpu(cpu, burst_cores)
    } else {
        0
    }
}

/// What burst cores add to a server's CPU limit (%), 0 for an unlimited server.
pub fn burst_cpu(cpu: i32, burst_cores: i16) -> i64 {
    if cpu > 0 {
        100 * burst_cores.max(0) as i64
    } else {
        0
    }
}

/// The container's CPU weight, as Docker's CPU shares: 1024 per paid core, from the paid limit
/// alone. `None` for an unlimited server (the host default).
pub fn cpu_weight(cpu: i32) -> Option<u32> {
    (cpu > 0).then(|| (cpu as i64 * 1024 / 100).clamp(2, 262_144) as u32)
}

/// Brings `users.burst_cores` in line with the memberships and the settings, for one account or
/// every account, and syncs the servers of each account whose count changed: Wings applies the
/// new CPU limit live. Runs after every membership change and each minute, which catches a
/// PlusPlus running out and changed settings.
pub async fn sync_burst(
    state: &crate::State,
    user_uuid: Option<uuid::Uuid>,
) -> Result<(), anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    let now = chrono::Utc::now().naive_utc();

    let rows = sqlx::query(
        "SELECT users.uuid, users.burst_cores, user_memberships.plus_since,
            user_memberships.plusplus_until
        FROM users
        LEFT JOIN user_memberships ON user_memberships.user_uuid = users.uuid
        WHERE ($1::uuid IS NULL OR users.uuid = $1)
            AND (user_memberships.user_uuid IS NOT NULL OR users.burst_cores <> 0)",
    )
    .bind(user_uuid)
    .fetch_all(state.database.write())
    .await?;

    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let stored: i16 = row.try_get("burst_cores")?;
        let cores = Membership {
            plus_since: row.try_get("plus_since")?,
            plusplus_until: row.try_get("plusplus_until")?,
            plusplus_purchases: 0,
        }
        .burst_cores(&economy, now);

        if cores == stored {
            continue;
        }

        // only from the value read, so a change made meanwhile is not undone
        let updated =
            sqlx::query("UPDATE users SET burst_cores = $2 WHERE uuid = $1 AND burst_cores = $3")
                .bind(uuid)
                .bind(cores)
                .bind(stored)
                .execute(state.database.write())
                .await?
                .rows_affected();
        if updated == 0 {
            continue;
        }

        let servers: Vec<uuid::Uuid> =
            sqlx::query_scalar("SELECT uuid FROM servers WHERE owner_uuid = $1 AND cpu > 0")
                .bind(uuid)
                .fetch_all(state.database.write())
                .await?;

        for server in servers {
            if let Some(server) =
                crate::models::server::Server::by_uuid_optional(&state.database, server).await?
            {
                server.batch_sync(state).await;
            }
        }
    }

    Ok(())
}

/// [`sync_burst`] for one account right after its membership changed; a failure waits for the
/// next minute's run.
async fn sync_burst_now(state: &crate::State, user_uuid: uuid::Uuid) {
    if let Err(err) = sync_burst(state, Some(user_uuid)).await {
        tracing::warn!(user = %user_uuid, "failed to sync the burst cores: {err:#}");
    }
}

/// The resources bought in the store without noprofit.space.
pub async fn extras(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<Resources, sqlx::Error> {
    let row = sqlx::query("SELECT extra_cpu, extra_memory, extra_disk FROM users WHERE uuid = $1")
        .bind(user_uuid)
        .fetch_optional(database.write())
        .await?;

    match row {
        Some(row) => Ok(Resources::new(
            row.try_get::<i32, _>("extra_cpu")? as i64,
            row.try_get::<i32, _>("extra_memory")? as i64,
            row.try_get::<i32, _>("extra_disk")? as i64,
        )),
        None => Ok(Resources::default()),
    }
}

/// Changes a balance by `amount` and writes the ledger row, in the caller's transaction. `None`
/// (and no change) when the balance would go below zero.
pub async fn change(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    currency: Currency,
    user_uuid: uuid::Uuid,
    amount: i64,
    kind: &str,
    description: &str,
    data: serde_json::Value,
) -> Result<Option<i64>, sqlx::Error> {
    let column = currency.column();
    // Tokens that came in (not an admin's correction) count towards what the account earned.
    let earned = if currency == Currency::Tokens && amount > 0 && kind != TYPE_ADMIN {
        ", tokens_earned = tokens_earned + $2"
    } else {
        ""
    };

    let balance: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "UPDATE users SET {column} = {column} + $2{earned}
        WHERE uuid = $1 AND {column} + $2 >= 0
        RETURNING {column}"
    )))
    .bind(user_uuid)
    .bind(amount)
    .fetch_optional(&mut **transaction)
    .await?;

    let Some(balance) = balance else {
        return Ok(None);
    };

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {} (user_uuid, amount, balance_after, type, description, data)
        VALUES ($1, $2, $3, $4, $5, $6)",
        currency.table()
    )))
    .bind(user_uuid)
    .bind(amount)
    .bind(balance)
    .bind(kind)
    .bind(truncate(description, 255))
    .bind(data)
    .execute(&mut **transaction)
    .await?;

    Ok(Some(balance))
}

fn truncate(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiTransaction {
    pub amount: i64,
    pub balance_after: i64,
    #[serde(rename = "type")]
    pub kind: compact_str::CompactString,
    pub description: String,
    pub created: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<crate::noprofit::StoreTransaction> for ApiTransaction {
    fn from(transaction: crate::noprofit::StoreTransaction) -> Self {
        Self {
            amount: transaction.amount,
            balance_after: transaction.balance_after,
            kind: transaction.kind.into(),
            description: transaction.description,
            created: transaction.created_at,
        }
    }
}

pub async fn transactions(
    database: &crate::database::Database,
    currency: Currency,
    user_uuid: uuid::Uuid,
    limit: i64,
) -> Result<Vec<ApiTransaction>, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT amount, balance_after, type, description, created FROM {}
        WHERE user_uuid = $1 ORDER BY created DESC LIMIT $2",
        currency.table()
    )))
    .bind(user_uuid)
    .bind(limit)
    .fetch_all(database.write())
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(ApiTransaction {
                amount: row.try_get("amount")?,
                balance_after: row.try_get("balance_after")?,
                kind: row.try_get::<String, _>("type")?.into(),
                description: row.try_get("description")?,
                created: Some(
                    row.try_get::<chrono::NaiveDateTime, _>("created")?
                        .and_utc(),
                ),
            })
        })
        .collect()
}

/// The account's balances: `(credits, tokens, tokens earned)`, credits being the panel's own.
async fn balances(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<(i64, i64, i64), sqlx::Error> {
    let row = sqlx::query("SELECT credits, tokens, tokens_earned FROM users WHERE uuid = $1")
        .bind(user_uuid)
        .fetch_optional(database.write())
        .await?;

    match row {
        Some(row) => Ok((
            row.try_get("credits")?,
            row.try_get("tokens")?,
            row.try_get("tokens_earned")?,
        )),
        None => Ok((0, 0, 0)),
    }
}

/// The tokens every account gets once a day, given the first time it looks after midnight (UTC).
pub async fn grant_daily_tokens(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    let daily = {
        let settings = state.settings.get().await?;
        if !settings.economy.tokens_enabled {
            return Ok(());
        }
        settings.economy.daily_tokens.max(0) as i64
    };
    if daily == 0 {
        return Ok(());
    }

    let mut transaction = state.database.write().begin().await?;
    let marked = sqlx::query(
        "UPDATE users SET tokens_daily_granted = CURRENT_DATE
        WHERE uuid = $1 AND (tokens_daily_granted IS NULL OR tokens_daily_granted < CURRENT_DATE)",
    )
    .bind(user_uuid)
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    if marked > 0 {
        change(
            &mut transaction,
            Currency::Tokens,
            user_uuid,
            daily,
            TYPE_DAILY,
            "Daily tokens",
            serde_json::json!({}),
        )
        .await?;
    }
    transaction.commit().await?;

    Ok(())
}

/// The starting credits, once per account, the first time its balance is shown (without
/// noprofit.space, whose accounts get theirs there).
async fn grant_starting_credits(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    amount: i64,
) -> Result<(), anyhow::Error> {
    if amount <= 0 {
        return Ok(());
    }

    let given = || {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM user_credit_transactions WHERE user_uuid = $1 AND type = $2)",
        )
        .bind(user_uuid)
        .bind(TYPE_STARTING)
    };

    if given().fetch_one(state.database.write()).await? {
        return Ok(());
    }

    let mut transaction = state.database.write().begin().await?;
    sqlx::query("SELECT 1 FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(user_uuid)
        .execute(&mut *transaction)
        .await?;

    if !given().fetch_one(&mut *transaction).await? {
        change(
            &mut transaction,
            Currency::Credits,
            user_uuid,
            amount,
            TYPE_STARTING,
            "Starting credits",
            serde_json::json!({}),
        )
        .await?;
    }
    transaction.commit().await?;

    Ok(())
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiPlusPlusOffer {
    pub pay: Pay,
    pub cost: i64,
    pub days: i64,
}

/// The account's balances and memberships, and what the memberships cost.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiEconomy {
    pub source: Source,
    pub credits_enabled: bool,
    pub credits: i64,
    pub tokens_enabled: bool,
    pub tokens: i64,
    pub tokens_earned: i64,
    pub membership_enabled: bool,
    pub plus_name: compact_str::CompactString,
    pub plusplus_name: compact_str::CompactString,
    pub plus_since: Option<chrono::DateTime<chrono::Utc>>,
    /// While PlusPlus runs.
    pub plusplus_until: Option<chrono::DateTime<chrono::Utc>>,
    /// The start queue's priority: 2 PlusPlus, 1 Plus, 0 none.
    pub priority: u8,
    pub plus_cost: i64,
    pub plusplus_offers: Vec<ApiPlusPlusOffer>,
    /// Burst cores on every server with Plus, and with PlusPlus (the total).
    pub plus_burst_cores: i64,
    pub plusplus_burst_cores: i64,
    /// What the account's servers get now.
    pub burst_cores: i64,
    /// More neo usage than the free limits, in %, with the first membership and with the second
    /// (the total); `None` while neo is not offered.
    pub neo_plus_percent: Option<i64>,
    pub neo_plusplus_percent: Option<i64>,
    /// The devbox each membership gets, while devboxes are offered.
    pub devbox: Option<ApiMembershipDevbox>,
}

/// A devbox's resources: CPU in % of one thread, memory and disk in MiB.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiDevboxSize {
    pub cpu: i64,
    pub memory: i64,
    pub disk: i64,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiMembershipDevbox {
    /// Only members get one; otherwise everyone gets the standard one.
    pub members_only: bool,
    pub standard: ApiDevboxSize,
    /// With the second membership.
    pub premium: ApiDevboxSize,
}

/// The account's economy. Gives the daily tokens and the starting credits when they are due.
pub async fn summary(state: &crate::State, user: &User) -> Result<ApiEconomy, anyhow::Error> {
    let (economy, connected, neo_percents, devbox) = {
        let settings = state.settings.get().await?;
        let neo_percents = crate::neo::offered(&settings).then(|| {
            use crate::settings::neo::NeoPlan;

            (
                settings.neo.usage_percent(NeoPlan::Plus) - 100,
                settings.neo.usage_percent(NeoPlan::Plusplus) - 100,
            )
        });
        let devbox = crate::devboxes::offered(&settings).then(|| {
            let devboxes = &settings.devboxes;

            ApiMembershipDevbox {
                members_only: devboxes.members_only,
                standard: ApiDevboxSize {
                    cpu: devboxes.cpu.max(0) as i64,
                    memory: devboxes.memory.max(0) as i64,
                    disk: devboxes.disk.max(0) as i64,
                },
                premium: ApiDevboxSize {
                    cpu: devboxes.premium_cpu.max(0) as i64,
                    memory: devboxes.premium_memory.max(0) as i64,
                    disk: devboxes.premium_disk.max(0) as i64,
                },
            }
        });

        (
            settings.economy.clone(),
            settings.noprofit.connected(),
            neo_percents,
            devbox,
        )
    };

    grant_daily_tokens(state, user.uuid).await?;
    if economy.credits_enabled && !connected {
        grant_starting_credits(state, user.uuid, economy.starting_credits).await?;
    }

    let (local_credits, tokens, tokens_earned) = balances(&state.database, user.uuid).await?;
    let credits = if connected {
        crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
            .await?
            .map_or(0, |link| link.credits)
    } else {
        local_credits
    };

    let membership = membership(&state.database, user.uuid).await?;
    let now = chrono::Utc::now().naive_utc();

    let mut plusplus_offers = Vec::with_capacity(2);
    if economy.tokens_enabled {
        plusplus_offers.push(ApiPlusPlusOffer {
            pay: Pay::Tokens,
            cost: economy.plusplus_tokens.max(0) as i64,
            days: economy.plusplus_token_days.max(1) as i64,
        });
    }
    if economy.credits_enabled && economy.plusplus_credits > 0 {
        plusplus_offers.push(ApiPlusPlusOffer {
            pay: Pay::Credits,
            cost: economy.plusplus_credits as i64,
            days: economy.plusplus_credit_days.max(1) as i64,
        });
    }

    let burst_cores = membership.burst_cores(&economy, now) as i64;

    Ok(ApiEconomy {
        source: if connected {
            Source::Noprofit
        } else {
            Source::Panel
        },
        credits_enabled: economy.credits_enabled,
        credits,
        tokens_enabled: economy.tokens_enabled,
        tokens,
        tokens_earned,
        membership_enabled: economy.membership_enabled,
        plus_name: economy.plus_name,
        plusplus_name: economy.plusplus_name,
        plus_since: membership.plus_since.map(|since| since.and_utc()),
        plusplus_until: membership
            .plusplus_until
            .filter(|_| membership.has_plusplus(now))
            .map(|until| until.and_utc()),
        priority: membership.priority(now),
        plus_cost: economy.plus_tokens.max(0) as i64,
        plusplus_offers,
        plus_burst_cores: economy.plus_burst_cores.max(0) as i64,
        plusplus_burst_cores: economy.plusplus_burst_cores.max(0) as i64,
        burst_cores,
        neo_plus_percent: neo_percents.map(|(plus, _)| plus),
        neo_plusplus_percent: neo_percents.map(|(_, plusplus)| plusplus),
        devbox,
    })
}

/// The sidebar's card: what the account's servers use of its allowance, and its balances.
#[derive(ToSchema, Serialize)]
pub struct ApiBalance {
    /// `None` while credits are turned off.
    pub credits: Option<i64>,
    /// `None` while tokens are turned off.
    pub tokens: Option<i64>,
    pub allowance: crate::allowance::Allowance,
}

/// What the sidebar shows; reads only (no daily tokens given), since it is asked every minute.
pub async fn balance(state: &crate::State, user: &User) -> Result<ApiBalance, anyhow::Error> {
    let (credits_enabled, tokens_enabled, connected) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.credits_enabled,
            settings.economy.tokens_enabled,
            settings.noprofit.connected(),
        )
    };

    let (local_credits, tokens, _) = balances(&state.database, user.uuid).await?;
    let credits = if connected {
        crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
            .await?
            .map_or(0, |link| link.credits)
    } else {
        local_credits
    };

    Ok(ApiBalance {
        credits: credits_enabled.then_some(credits),
        tokens: tokens_enabled.then_some(tokens),
        allowance: crate::allowance::for_user(state, user.uuid).await?,
    })
}

#[derive(Debug)]
pub enum EconomyError {
    /// Turned off in the settings.
    Disabled,
    /// Nothing for sale right now (no prices, an unlimited resource, noprofit.space down).
    Unavailable,
    /// The balance is short.
    Short,
    NeedsPlus,
    Owned,
    /// The price moved since the player saw it; the store as it is now.
    PriceChanged(Box<ApiStore>),
    /// noprofit.space refused, with its reason.
    Refused(String),
    BadRequest(&'static str),
    Other(anyhow::Error),
}

impl EconomyError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "this is turned off".into(),
            Self::Unavailable => "this is not for sale right now".into(),
            Self::Short => "your balance is too low".into(),
            Self::NeedsPlus => "this needs the first membership".into(),
            Self::Owned => "you have this already".into(),
            Self::PriceChanged(_) => "the price changed, look at the new one".into(),
            Self::Refused(reason) if reason.is_empty() => "the purchase was refused".into(),
            Self::Refused(reason) => reason.clone(),
            Self::BadRequest(reason) => (*reason).into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled | Self::NeedsPlus => 403,
            Self::Unavailable | Self::Short | Self::Owned | Self::PriceChanged(_) => 409,
            Self::Refused(_) => 422,
            Self::BadRequest(_) => 400,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for EconomyError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<sqlx::Error> for EconomyError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<crate::database::DatabaseError> for EconomyError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

/// The Resource Store as the account sees it.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiStore {
    pub source: Source,
    /// Whether anything is for sale (credits on, prices known).
    pub available: bool,
    pub credits: i64,
    pub catalog: Vec<CatalogEntry>,
    pub transactions: Vec<ApiTransaction>,
    /// False when noprofit.space is connected and the account has no plan: no resources to show.
    pub has_plan: bool,
    /// One meter per resource (CPU, memory, disk), shown whether or not anything is for sale.
    pub usage: Vec<ApiStoreUsage>,
    /// Where credits are earned on Discord (noprofit.space's Discord page); `None` without
    /// noprofit.space.
    pub rewards_url: Option<String>,
    /// What the account has not claimed yet; the page's bar is gone once this is empty.
    pub perks: Vec<ApiPerk>,
}

/// A resource's meter: what the account's servers use against its limit.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiStoreUsage {
    pub resource: StoreResource,
    pub used: i64,
    /// 0: no limit.
    pub limit: i64,
}

/// An upgrade the account has not claimed yet (the Pelican panel's perk discovery), in the order
/// the page lists them.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "id", rename_all = "snake_case")]
pub enum ApiPerk {
    /// Linking Discord and joining the Discord servers adds `gain` to the plan.
    DiscordLink { gain: Resources, url: String },
    /// Linked: joining the Discord servers the account is not in yet adds `gain`.
    DiscordJoin { gain: Resources, url: String },
    /// Credits for each person invited to the Discord server, and for a boost (0: none).
    DiscordEarn {
        per_invite: i64,
        per_boost: i64,
        url: String,
    },
    /// `price` credits buy the next `amount` MiB of memory.
    Store { price: i64, amount: i64 },
}

fn amount_of(resources: Resources, resource: StoreResource) -> i64 {
    match resource {
        StoreResource::Cpu => resources.cpu,
        StoreResource::Memory => resources.memory,
        StoreResource::Disk => resources.disk,
    }
}

/// The meters' numbers: this panel's own servers against the share noprofit.space gives the panel.
/// Other services' use of the shared pool is already taken out of the plan's `cpu`, `memory` and
/// `disk` (its `planFor`), and is not shown here: the sidebar and the store count plugged.host
/// only.
pub fn store_usage(limit: Resources, used: Resources) -> Vec<ApiStoreUsage> {
    StoreResource::ALL
        .into_iter()
        .map(|resource| ApiStoreUsage {
            resource,
            used: amount_of(used, resource),
            limit: amount_of(limit, resource),
        })
        .collect()
}

/// noprofit.space's side of the perks: its accounts site, whether the account linked Discord, and
/// the Discord rewards the store answered with (`None`: not sent, so nothing to offer from them).
struct DiscordRewards<'a> {
    accounts_url: &'a str,
    linked: bool,
    status: Option<&'a crate::noprofit::StoreDiscord>,
}

/// The perks in the Pelican panel's order (`PerkDiscovery`): what the Discord servers would still
/// add, credits for invites, then what the next step of memory costs.
fn perks(
    has_plan: bool,
    catalog: &[CatalogEntry],
    discord: Option<DiscordRewards<'_>>,
) -> Vec<ApiPerk> {
    let mut perks = Vec::new();

    if let Some(discord) = discord
        && let Some(status) = discord.status
    {
        let gain = Resources::new(
            (status.max_bonus.cpu - status.bonus.cpu).max(0),
            (status.max_bonus.memory - status.bonus.memory).max(0),
            (status.max_bonus.disk - status.bonus.disk).max(0),
        );

        if gain != Resources::default() {
            perks.push(if discord.linked {
                ApiPerk::DiscordJoin {
                    gain,
                    url: format!("{}/discord", discord.accounts_url),
                }
            } else {
                ApiPerk::DiscordLink {
                    gain,
                    url: format!("{}/settings", discord.accounts_url),
                }
            });
        }

        // Invites only count for a linked account.
        if discord.linked && status.credits_per_invite > 0 {
            perks.push(ApiPerk::DiscordEarn {
                per_invite: status.credits_per_invite,
                per_boost: status.credits_per_boost.max(0),
                url: format!("{}/discord", discord.accounts_url),
            });
        }
    }

    if has_plan
        && let Some(offer) = catalog
            .iter()
            .find(|entry| entry.resource == StoreResource::Memory)
            .and_then(|entry| entry.offers.first())
        && offer.price > 0
        && offer.price < UNAFFORDABLE
    {
        perks.push(ApiPerk::Store {
            price: offer.price,
            amount: offer.amount,
        });
    }

    perks
}

/// The store without noprofit.space: the games allowance plus what was bought, priced on the
/// settings' curve. A resource the allowance does not limit is not for sale.
async fn panel_store(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    pricing: &Pricing,
    base: Resources,
) -> Result<ApiStore, anyhow::Error> {
    grant_starting_credits(
        state,
        user_uuid,
        state.settings.get().await?.economy.starting_credits,
    )
    .await?;

    let (credits, _, _) = balances(&state.database, user_uuid).await?;
    let extras = extras(&state.database, user_uuid).await?;
    let (used, _) = crate::allowance::usage(&state.database, user_uuid).await?;
    let catalog = panel_catalog(pricing, base, extras);

    Ok(ApiStore {
        source: Source::Panel,
        available: true,
        credits,
        perks: perks(true, &catalog, None),
        catalog,
        transactions: transactions(&state.database, Currency::Credits, user_uuid, HISTORY).await?,
        has_plan: true,
        usage: store_usage(crate::allowance::add_where_limited(base, extras), used),
        rewards_url: None,
    })
}

fn panel_catalog(pricing: &Pricing, base: Resources, extras: Resources) -> Vec<CatalogEntry> {
    let mut pricing = *pricing;
    // An unlimited resource is not sold: there is nothing to add to.
    for (index, limit) in [base.cpu, base.memory, base.disk].into_iter().enumerate() {
        if limit <= 0 {
            pricing.steps[index] = 0;
        }
    }

    pricing.catalog([
        base.cpu.max(0) + extras.cpu,
        base.memory.max(0) + extras.memory,
        base.disk.max(0) + extras.disk,
    ])
}

fn noprofit_store(store: crate::noprofit::Store) -> ApiStore {
    ApiStore {
        source: Source::Noprofit,
        available: true,
        credits: store.credits,
        catalog: store.catalog,
        transactions: store
            .transactions
            .into_iter()
            .map(ApiTransaction::from)
            .collect(),
        has_plan: false,
        usage: Vec::new(),
        rewards_url: None,
        perks: Vec::new(),
    }
}

fn closed_store(source: Source) -> ApiStore {
    ApiStore {
        source,
        available: false,
        credits: 0,
        catalog: Vec::new(),
        transactions: Vec::new(),
        has_plan: false,
        usage: Vec::new(),
        rewards_url: None,
        perks: Vec::new(),
    }
}

/// The store page with noprofit.space: its store for the account (`None` when it did not answer,
/// or there is no account: closed), with the account's plan, meters and perks.
async fn noprofit_page(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    accounts_url: &str,
    link: Option<&crate::noprofit::Link>,
    answer: Option<crate::noprofit::Store>,
) -> Result<ApiStore, anyhow::Error> {
    let (used, _) = crate::allowance::usage(&state.database, user_uuid).await?;
    let plan = link.and_then(|link| link.plan.as_ref());
    let discord = answer.as_ref().and_then(|store| store.discord.clone());

    let mut page = match answer {
        Some(store) => noprofit_store(store),
        None => ApiStore {
            credits: link.map_or(0, |link| link.credits),
            ..closed_store(Source::Noprofit)
        },
    };

    page.has_plan = plan.is_some();
    page.usage = store_usage(plan.map(Resources::from_plan).unwrap_or_default(), used);
    page.rewards_url = Some(format!("{accounts_url}/discord"));
    page.perks = perks(
        page.has_plan,
        &page.catalog,
        Some(DiscordRewards {
            accounts_url,
            linked: link
                .and_then(|link| link.discord.as_ref())
                .is_some_and(|discord| !discord.is_null()),
            status: discord.as_ref(),
        }),
    );

    Ok(page)
}

pub async fn store(state: &crate::State, user: &User) -> Result<ApiStore, EconomyError> {
    let (economy, noprofit, base) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.clone(),
            settings.noprofit.config(),
            settings.games.allowance(),
        )
    };

    if !economy.credits_enabled {
        return Err(EconomyError::Disabled);
    }

    match noprofit {
        Some(noprofit) => {
            let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid).await?;
            let answer = match &link {
                Some(link) => match noprofit.store(&state.client, link.account_id).await {
                    Ok(store) => Some(store),
                    Err(err) => {
                        tracing::warn!(user = %user.uuid, "noprofit.space's store did not answer: {err:#}");

                        None
                    }
                },
                None => None,
            };

            Ok(noprofit_page(
                state,
                user.uuid,
                &noprofit.accounts_url,
                link.as_ref(),
                answer,
            )
            .await?)
        }
        None => Ok(panel_store(state, user.uuid, &Pricing::from_settings(&economy), base).await?),
    }
}

/// Buys `steps` steps of a resource at `price`, the price the player saw.
pub async fn purchase(
    state: &crate::State,
    user: &User,
    resource: StoreResource,
    steps: i64,
    price: i64,
) -> Result<ApiStore, EconomyError> {
    let (economy, noprofit, base) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.clone(),
            settings.noprofit.config(),
            settings.games.allowance(),
        )
    };

    if !economy.credits_enabled {
        return Err(EconomyError::Disabled);
    }
    if !(1..=MAX_STEPS).contains(&steps) {
        return Err(EconomyError::BadRequest("buy between 1 and 50 steps"));
    }

    if let Some(noprofit) = noprofit {
        let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
            .await?
            .ok_or(EconomyError::Unavailable)?;

        let answer = noprofit
            .purchase(&state.client, link.account_id, resource, steps, price)
            .await
            .map_err(|err| {
                tracing::warn!(user = %user.uuid, "noprofit.space's store did not answer a purchase: {err:#}");
                EconomyError::Unavailable
            })?;

        return match answer {
            crate::noprofit::Purchase::Done(store) => {
                // The plan grew: take it on now rather than at the next sync.
                if let Err(err) = crate::noprofit::refresh_user(state, user.uuid).await {
                    tracing::warn!(user = %user.uuid, "failed to pull the account after a purchase: {err:#}");
                }
                // The plan as it is now, for the meters.
                let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
                    .await?
                    .unwrap_or(link);

                Ok(noprofit_page(
                    state,
                    user.uuid,
                    &noprofit.accounts_url,
                    Some(&link),
                    Some(store),
                )
                .await?)
            }
            crate::noprofit::Purchase::PriceChanged(Some(store)) => {
                Err(EconomyError::PriceChanged(Box::new(noprofit_store(store))))
            }
            crate::noprofit::Purchase::PriceChanged(None) => Err(EconomyError::Unavailable),
            crate::noprofit::Purchase::Refused(reason) => Err(EconomyError::Refused(reason)),
        };
    }

    let pricing = Pricing::from_settings(&economy);
    let base_of = match resource {
        StoreResource::Cpu => base.cpu,
        StoreResource::Memory => base.memory,
        StoreResource::Disk => base.disk,
    };
    let step = pricing.step(resource);
    if step <= 0 || base_of <= 0 {
        return Err(EconomyError::Unavailable);
    }

    let amount = steps * step;
    let mut transaction = state.database.write().begin().await?;
    let row = sqlx::query(
        "SELECT extra_cpu, extra_memory, extra_disk FROM users WHERE uuid = $1 FOR UPDATE",
    )
    .bind(user.uuid)
    .fetch_one(&mut *transaction)
    .await?;
    let extra = |column: &str| row.try_get::<i32, _>(column).map(i64::from);
    let extras = Resources::new(
        extra("extra_cpu")?,
        extra("extra_memory")?,
        extra("extra_disk")?,
    );
    let extra_of = match resource {
        StoreResource::Cpu => extras.cpu,
        StoreResource::Memory => extras.memory,
        StoreResource::Disk => extras.disk,
    };

    let cost = pricing.cost(
        resource,
        base_of + extra_of,
        amount,
        pricing.limit(resource),
    );
    if cost != price {
        drop(transaction);

        let store = panel_store(state, user.uuid, &pricing, base).await?;
        return Err(EconomyError::PriceChanged(Box::new(store)));
    }
    if cost >= UNAFFORDABLE || i32::try_from(extra_of + amount).is_err() {
        return Err(EconomyError::Short);
    }

    let unit = if resource == StoreResource::Cpu {
        "%"
    } else {
        " MiB"
    };
    let paid = change(
        &mut transaction,
        Currency::Credits,
        user.uuid,
        -cost,
        TYPE_STORE,
        &format!("{amount}{unit} {}", resource.as_str()),
        serde_json::json!({ "resource": resource, "amount": amount, "cost": cost }),
    )
    .await?;
    if paid.is_none() {
        return Err(EconomyError::Short);
    }

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE users SET extra_{0} = extra_{0} + $2 WHERE uuid = $1",
        resource.as_str()
    )))
    .bind(user.uuid)
    .bind(amount as i32)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(panel_store(state, user.uuid, &pricing, base).await?)
}

/// The membership row, made when missing and locked for the rest of the transaction.
async fn locked_membership(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
) -> Result<Membership, sqlx::Error> {
    sqlx::query("INSERT INTO user_memberships (user_uuid) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_uuid)
        .execute(&mut **transaction)
        .await?;

    let row = sqlx::query(
        "SELECT plus_since, plusplus_until, plusplus_purchases FROM user_memberships
        WHERE user_uuid = $1 FOR UPDATE",
    )
    .bind(user_uuid)
    .fetch_one(&mut **transaction)
    .await?;

    Ok(Membership {
        plus_since: row.try_get("plus_since")?,
        plusplus_until: row.try_get("plusplus_until")?,
        plusplus_purchases: row.try_get("plusplus_purchases")?,
    })
}

/// Buys Plus with tokens: the tokens and Plus change together or not at all.
pub async fn buy_plus(state: &crate::State, user: &User) -> Result<ApiEconomy, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.membership_enabled {
        return Err(EconomyError::Disabled);
    }
    let cost = economy.plus_tokens.max(0) as i64;
    if cost > 0 && !economy.tokens_enabled {
        return Err(EconomyError::Unavailable);
    }

    let mut transaction = state.database.write().begin().await?;
    if locked_membership(&mut transaction, user.uuid)
        .await?
        .has_plus()
    {
        return Err(EconomyError::Owned);
    }

    if cost > 0
        && change(
            &mut transaction,
            Currency::Tokens,
            user.uuid,
            -cost,
            TYPE_PLUS,
            &economy.plus_name,
            serde_json::json!({}),
        )
        .await?
        .is_none()
    {
        return Err(EconomyError::Short);
    }

    sqlx::query("UPDATE user_memberships SET plus_since = NOW() WHERE user_uuid = $1")
        .bind(user.uuid)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    sync_burst_now(state, user.uuid).await;
    User::invalidate_cached(&state.database, user.uuid).await;

    Ok(summary(state, user).await?)
}

/// Buys (or extends) PlusPlus with tokens or credits. Needs Plus. Bought time adds to what is
/// left, so buying early loses nothing.
pub async fn buy_plusplus(
    state: &crate::State,
    user: &User,
    pay: Pay,
) -> Result<ApiEconomy, EconomyError> {
    let (economy, noprofit) = {
        let settings = state.settings.get().await?;
        (settings.economy.clone(), settings.noprofit.config())
    };
    if !economy.membership_enabled {
        return Err(EconomyError::Disabled);
    }

    let (cost, days) = match pay {
        Pay::Tokens if economy.tokens_enabled => (
            economy.plusplus_tokens.max(0) as i64,
            economy.plusplus_token_days.max(1) as i64,
        ),
        Pay::Credits if economy.credits_enabled && economy.plusplus_credits > 0 => (
            economy.plusplus_credits as i64,
            economy.plusplus_credit_days.max(1) as i64,
        ),
        _ => return Err(EconomyError::Unavailable),
    };

    let mut transaction = state.database.write().begin().await?;
    let membership = locked_membership(&mut transaction, user.uuid).await?;
    if !membership.has_plus() {
        return Err(EconomyError::NeedsPlus);
    }

    let description = format!("{} ({days} days)", economy.plusplus_name);
    let data = serde_json::json!({ "days": days, "pay": pay });
    let mut spent_on_noprofit = false;

    if cost > 0 {
        match (pay, noprofit) {
            (Pay::Credits, Some(noprofit)) => {
                let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
                    .await?
                    .ok_or(EconomyError::Unavailable)?;
                // The same purchase sent again (the panel failed after noprofit.space took the
                // credits) is not charged twice.
                let key = format!(
                    "calagopus:plusplus:{}:{}:{cost}",
                    user.uuid,
                    membership.plusplus_purchases + 1
                );

                let spent = noprofit
                    .spend_credits(
                        &state.client,
                        link.account_id,
                        cost,
                        TYPE_PLUSPLUS,
                        &description,
                        &key,
                    )
                    .await
                    .map_err(|err| {
                        tracing::warn!(user = %user.uuid, "noprofit.space did not take the credits for PlusPlus: {err:#}");
                        EconomyError::Unavailable
                    })?;
                if spent.is_none() {
                    return Err(EconomyError::Short);
                }
                spent_on_noprofit = true;
            }
            (pay, _) => {
                let currency = match pay {
                    Pay::Tokens => Currency::Tokens,
                    Pay::Credits => Currency::Credits,
                };

                if change(
                    &mut transaction,
                    currency,
                    user.uuid,
                    -cost,
                    TYPE_PLUSPLUS,
                    &description,
                    data,
                )
                .await?
                .is_none()
                {
                    return Err(EconomyError::Short);
                }
            }
        }
    }

    let now = chrono::Utc::now().naive_utc();
    sqlx::query(
        "UPDATE user_memberships
        SET plusplus_until = $2, plusplus_purchases = plusplus_purchases + 1
        WHERE user_uuid = $1",
    )
    .bind(user.uuid)
    .bind(membership.extended_until(now, days))
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    sync_burst_now(state, user.uuid).await;
    User::invalidate_cached(&state.database, user.uuid).await;

    if spent_on_noprofit && let Err(err) = crate::noprofit::refresh_user(state, user.uuid).await {
        tracing::warn!(user = %user.uuid, "failed to pull the account after PlusPlus: {err:#}");
    }

    Ok(summary(state, user).await?)
}

/// What an admin sees of a user's economy.
#[derive(ToSchema, Serialize)]
pub struct ApiAdminEconomy {
    #[serde(flatten)]
    pub summary: ApiEconomy,
    /// The panel's own credits (without noprofit.space the balance, with it unused).
    pub panel_credits: i64,
    /// What the panel's store sold the account.
    pub extras: Resources,
    pub plusplus_purchases: i32,
    pub credit_transactions: Vec<ApiTransaction>,
    pub token_transactions: Vec<ApiTransaction>,
}

pub async fn admin_view(
    state: &crate::State,
    user: &User,
) -> Result<ApiAdminEconomy, anyhow::Error> {
    let summary = summary(state, user).await?;
    let (panel_credits, _, _) = balances(&state.database, user.uuid).await?;

    Ok(ApiAdminEconomy {
        summary,
        panel_credits,
        extras: extras(&state.database, user.uuid).await?,
        plusplus_purchases: membership(&state.database, user.uuid)
            .await?
            .plusplus_purchases,
        credit_transactions: transactions(&state.database, Currency::Credits, user.uuid, 20)
            .await?,
        token_transactions: transactions(&state.database, Currency::Tokens, user.uuid, 20).await?,
    })
}

/// An admin's change to a user's economy; every field optional.
#[derive(Default)]
pub struct AdminChange {
    /// Added to the panel's credits (negative takes away).
    pub credits: Option<i64>,
    pub tokens: Option<i64>,
    /// Shown in the user's history.
    pub reason: Option<String>,
    pub plus: Option<bool>,
    /// `Some(None)` ends PlusPlus now.
    pub plusplus_until: Option<Option<chrono::DateTime<chrono::Utc>>>,
}

pub async fn admin_change(
    state: &crate::State,
    user: &User,
    change_request: AdminChange,
) -> Result<(), EconomyError> {
    let connected = state.settings.get().await?.noprofit.connected();
    if change_request.credits.is_some_and(|credits| credits != 0) && connected {
        return Err(EconomyError::BadRequest(
            "credits are noprofit.space's while it is connected",
        ));
    }

    let reason = change_request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .unwrap_or("Changed by an admin");
    let mut transaction = state.database.write().begin().await?;

    for (currency, amount) in [
        (Currency::Credits, change_request.credits),
        (Currency::Tokens, change_request.tokens),
    ] {
        let Some(amount) = amount.filter(|amount| *amount != 0) else {
            continue;
        };

        if change(
            &mut transaction,
            currency,
            user.uuid,
            amount,
            TYPE_ADMIN,
            reason,
            serde_json::json!({}),
        )
        .await?
        .is_none()
        {
            return Err(EconomyError::BadRequest("the balance would go below zero"));
        }
    }

    if change_request.plus.is_some() || change_request.plusplus_until.is_some() {
        locked_membership(&mut transaction, user.uuid).await?;

        if let Some(plus) = change_request.plus {
            sqlx::query(
                "UPDATE user_memberships
                SET plus_since = CASE WHEN $2 THEN COALESCE(plus_since, NOW()) ELSE NULL END
                WHERE user_uuid = $1",
            )
            .bind(user.uuid)
            .bind(plus)
            .execute(&mut *transaction)
            .await?;
        }
        if let Some(until) = change_request.plusplus_until {
            // Ended by an admin: the servers leave the premium locations like after a lapse.
            let until = until
                .map(|until| until.naive_utc())
                .unwrap_or_else(|| chrono::Utc::now().naive_utc());

            sqlx::query("UPDATE user_memberships SET plusplus_until = $2 WHERE user_uuid = $1")
                .bind(user.uuid)
                .bind(until)
                .execute(&mut *transaction)
                .await?;
        }
    }
    transaction.commit().await?;
    sync_burst_now(state, user.uuid).await;
    User::invalidate_cached(&state.database, user.uuid).await;

    Ok(())
}

/// The order a server leaving a premium location tries the other locations in: the same country
/// (flag) first, Plus before free, then the fastest.
pub fn destination_order(
    current_flag: Option<&str>,
    location: &crate::server_creation::ApiCreateLocation,
) -> (u8, u8, i64) {
    (
        u8::from(current_flag.is_none() || location.flag.as_deref() != current_flag),
        u8::from(location.tier.as_deref() != Some(TIER_PLUS)),
        location.latency.map_or(i64::MAX, i64::from),
    )
}

/// Carries out every PlusPlus that ran out: every server on a premium location moves to another
/// location the owner may use (an admin move, a few at a time). A lapse is settled once no server is left on a premium
/// location; until then the task comes back, so a full location is tried again. Buying PlusPlus
/// again before that stops it. Then every account's burst cores are brought in line
/// ([`sync_burst`]).
pub async fn settle_memberships(state: &crate::State) -> Result<(), anyhow::Error> {
    let due = sqlx::query(
        "SELECT user_uuid, plusplus_until FROM user_memberships
        WHERE plusplus_until IS NOT NULL AND plusplus_until <= NOW()
            AND (plusplus_settled IS NULL OR plusplus_settled < plusplus_until)
        ORDER BY plusplus_until
        LIMIT 50",
    )
    .fetch_all(state.database.write())
    .await?;

    for row in due {
        let user_uuid: uuid::Uuid = row.try_get("user_uuid")?;

        if let Err(err) = settle_lapse(state, user_uuid).await {
            tracing::warn!(user = %user_uuid, "failed to settle a lapsed PlusPlus: {err:#}");
        }
    }

    // a PlusPlus that ran out drops to Plus's burst cores
    sync_burst(state, None).await
}

async fn settle_lapse(state: &crate::State, user_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let Some(user) = User::by_uuid_optional(&state.database, user_uuid).await? else {
        return Ok(());
    };

    // The owner's servers on premium locations, and whether a move of each is under way.
    let servers = sqlx::query(
        r#"
        SELECT servers.uuid, locations.flag,
            EXISTS (
                SELECT 1 FROM server_transfer_requests requests
                WHERE requests.server_uuid = servers.uuid
                    AND requests.status IN ('queued', 'running', 'cancelling')
            ) AS moving
        FROM servers
        JOIN nodes ON nodes.uuid = servers.node_uuid
        JOIN locations ON locations.uuid = nodes.location_uuid
        WHERE servers.owner_uuid = $1 AND locations.tier = $2
        "#,
    )
    .bind(user_uuid)
    .bind(TIER_PREMIUM)
    .fetch_all(state.database.write())
    .await?;

    if servers.is_empty() {
        sqlx::query("UPDATE user_memberships SET plusplus_settled = NOW() WHERE user_uuid = $1")
            .bind(user_uuid)
            .execute(state.database.write())
            .await?;

        return Ok(());
    }

    let locations = crate::server_creation::locations(state, &user).await?;

    for row in servers {
        if row.try_get::<bool, _>("moving")? {
            continue;
        }

        let server_uuid: uuid::Uuid = row.try_get("uuid")?;
        let flag: Option<String> = row.try_get("flag")?;
        let Some(destination) = locations
            .iter()
            .filter(|location| {
                location.unlocked
                    && location.available
                    && location.tier.as_deref() != Some(TIER_PREMIUM)
            })
            .min_by_key(|location| destination_order(flag.as_deref(), location))
        else {
            tracing::info!(server = %server_uuid, "PlusPlus ran out, but no other location takes the server yet");
            continue;
        };

        let Some(server) =
            crate::models::server::Server::by_uuid_optional(&state.database, server_uuid).await?
        else {
            continue;
        };

        crate::server_transfers::queue_admin_move(state, &server, destination.uuid).await?;
        tracing::info!(server = %server_uuid, location = %destination.uuid, "PlusPlus ran out, moving the server off its premium location");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: i64) -> chrono::NaiveDateTime {
        chrono::DateTime::from_timestamp(1_800_000_000 + hours * 3600, 0)
            .unwrap()
            .naive_utc()
    }

    #[test]
    fn burst_cores_follow_the_best_membership_running() {
        let settings = crate::settings::economy::AppSettingsEconomy::default();
        let now = at(0);
        let plus = Membership {
            plus_since: Some(at(-10)),
            ..Default::default()
        };
        let plusplus = Membership {
            plusplus_until: Some(at(5)),
            ..plus.clone()
        };
        let lapsed = Membership {
            plusplus_until: Some(at(-1)),
            ..plus.clone()
        };

        assert_eq!(Membership::default().burst_cores(&settings, now), 0);
        assert_eq!(plus.burst_cores(&settings, now), 1);
        // the total, not 1 + 2
        assert_eq!(plusplus.burst_cores(&settings, now), 2);
        assert_eq!(lapsed.burst_cores(&settings, now), 1);
    }

    #[test]
    fn burst_raises_the_limit_but_not_the_weight() {
        assert_eq!(cpu_limit_with_burst(200, 1), 300);
        assert_eq!(burst_cpu(200, 2), 200);
        // unlimited stays unlimited
        assert_eq!(cpu_limit_with_burst(0, 2), 0);
        assert_eq!(burst_cpu(0, 2), 0);

        assert_eq!(cpu_weight(100), Some(1024));
        assert_eq!(cpu_weight(250), Some(2560));
        assert_eq!(cpu_weight(0), None);
        assert_eq!(cpu_weight(1_000_000), Some(262_144));
    }

    #[test]
    fn tiers_open_with_the_memberships() {
        let now = at(0);
        let none = Membership::default();
        let plus = Membership {
            plus_since: Some(at(-10)),
            ..Default::default()
        };
        let plusplus = Membership {
            plus_since: Some(at(-10)),
            plusplus_until: Some(at(5)),
            plusplus_purchases: 1,
        };
        let lapsed = Membership {
            plusplus_until: Some(at(-1)),
            ..plusplus.clone()
        };

        for membership in [&none, &plus, &plusplus] {
            assert!(membership.unlocks(None, now));
            assert!(membership.unlocks(Some(""), now));
            assert!(!membership.unlocks(Some("gold"), now));
        }
        assert!(!none.unlocks(Some(TIER_PLUS), now));
        assert!(plus.unlocks(Some(TIER_PLUS), now));
        assert!(!plus.unlocks(Some(TIER_PREMIUM), now));
        assert!(plusplus.unlocks(Some(TIER_PREMIUM), now));
        assert!(!lapsed.unlocks(Some(TIER_PREMIUM), now));
        assert!(lapsed.unlocks(Some(TIER_PLUS), now));

        assert_eq!(none.priority(now), 0);
        assert_eq!(plus.priority(now), 1);
        assert_eq!(plusplus.priority(now), 2);
        assert_eq!(lapsed.priority(now), 1);
    }

    #[test]
    fn plusplus_bought_early_adds_to_what_is_left() {
        let now = at(0);
        let running = Membership {
            plus_since: Some(at(-10)),
            plusplus_until: Some(at(48)),
            plusplus_purchases: 1,
        };
        let lapsed = Membership {
            plusplus_until: Some(at(-48)),
            ..running.clone()
        };

        assert_eq!(running.extended_until(now, 7), at(48 + 7 * 24));
        assert_eq!(lapsed.extended_until(now, 7), at(7 * 24));
        assert_eq!(Membership::default().extended_until(now, 0), at(24));
    }

    #[test]
    fn unlimited_resources_are_not_sold() {
        let pricing =
            Pricing::from_settings(&crate::settings::economy::AppSettingsEconomy::default());
        let catalog = panel_catalog(
            &pricing,
            Resources::new(200, 0, 10240),
            Resources::new(50, 0, 0),
        );

        assert_eq!(catalog[0].total, 250);
        assert!(!catalog[0].offers.is_empty());
        assert!(catalog[1].offers.is_empty());
        assert_eq!(catalog[2].total, 10240);
        assert!(!catalog[2].offers.is_empty());
    }

    #[test]
    fn meters_show_the_servers_against_the_limit() {
        let usage = store_usage(
            Resources::new(400, 8192, 0),
            Resources::new(100, 9000, 2048),
        );

        assert_eq!(usage.len(), 3);
        assert_eq!(
            usage[0],
            ApiStoreUsage {
                resource: StoreResource::Cpu,
                used: 100,
                limit: 400
            }
        );
        // over the limit stays over; no limit stays 0
        assert_eq!((usage[1].used, usage[1].limit), (9000, 8192));
        assert_eq!((usage[2].used, usage[2].limit), (2048, 0));
    }

    #[test]
    fn a_shared_pool_shows_only_this_panels_share() {
        // noprofit.space's planFor(): this panel's share, the whole pool and what others use of it.
        // The sidebar counts plugged.host alone, so only cpu/memory/disk are read.
        let plan = serde_json::json!({
            "cpu": 300, "total_cpu": 400, "elsewhere_cpu": 100,
            "memory": 6144, "total_memory": 8192.0,
            "disk": 10240,
        });
        let usage = store_usage(Resources::from_plan(&plan), Resources::new(200, 4096, 1024));

        assert_eq!((usage[0].used, usage[0].limit), (200, 300));
        assert_eq!((usage[1].used, usage[1].limit), (4096, 6144));
        assert_eq!((usage[2].used, usage[2].limit), (1024, 10240));
    }

    fn store_catalog(memory_price: i64) -> Vec<CatalogEntry> {
        vec![CatalogEntry {
            resource: StoreResource::Memory,
            step: 1024,
            total: 4096,
            limit: 24576,
            soft_limit: 30720,
            offers: vec![crate::pricing::Offer {
                steps: 1,
                amount: 1024,
                price: memory_price,
            }],
        }]
    }

    #[test]
    fn the_store_perk_needs_a_plan_and_a_price() {
        assert_eq!(
            perks(true, &store_catalog(75), None),
            vec![ApiPerk::Store {
                price: 75,
                amount: 1024
            }]
        );
        assert!(perks(false, &store_catalog(75), None).is_empty());
        assert!(perks(true, &store_catalog(0), None).is_empty());
        assert!(perks(true, &store_catalog(UNAFFORDABLE), None).is_empty());
        assert!(perks(true, &[], None).is_empty());
    }

    #[test]
    fn discord_perks_follow_what_is_left_to_claim() {
        let status = crate::noprofit::StoreDiscord {
            bonus: Resources::new(0, 1024, 0),
            max_bonus: Resources::new(0, 3072, 0),
            credits_per_invite: 25,
            credits_per_boost: 100,
        };
        fn rewards(
            linked: bool,
            status: Option<&crate::noprofit::StoreDiscord>,
        ) -> DiscordRewards<'_> {
            DiscordRewards {
                accounts_url: "https://accounts.example.com",
                linked,
                status,
            }
        }

        // not linked: link it, for what all the servers add; no invites yet
        assert_eq!(
            perks(false, &[], Some(rewards(false, Some(&status)))),
            vec![ApiPerk::DiscordLink {
                gain: Resources::new(0, 2048, 0),
                url: "https://accounts.example.com/settings".into()
            }]
        );

        // linked: join the rest, then invite; the store last
        let linked = perks(true, &store_catalog(75), Some(rewards(true, Some(&status))));
        assert_eq!(linked.len(), 3);
        assert_eq!(
            linked[0],
            ApiPerk::DiscordJoin {
                gain: Resources::new(0, 2048, 0),
                url: "https://accounts.example.com/discord".into()
            }
        );
        assert_eq!(
            linked[1],
            ApiPerk::DiscordEarn {
                per_invite: 25,
                per_boost: 100,
                url: "https://accounts.example.com/discord".into()
            }
        );
        assert!(matches!(linked[2], ApiPerk::Store { .. }));

        // in every server, no invite credits: nothing from Discord
        let done = crate::noprofit::StoreDiscord {
            bonus: status.max_bonus,
            credits_per_invite: 0,
            ..status.clone()
        };
        assert!(perks(false, &[], Some(rewards(true, Some(&done)))).is_empty());
        // noprofit.space sent no Discord rewards
        assert!(perks(false, &[], Some(rewards(true, None))).is_empty());
    }

    #[test]
    fn discord_rewards_are_read_leniently() {
        let store: crate::noprofit::Store = serde_json::from_value(serde_json::json!({
            "credits": 10,
            "catalog": [],
            "discord": {"bonus": {"memory": 1024}, "max_bonus": {"cpu": 50, "memory": 3072}, "boosts": 2},
        }))
        .unwrap();
        let discord = store.discord.unwrap();

        assert_eq!(discord.bonus, Resources::new(0, 1024, 0));
        assert_eq!(discord.max_bonus, Resources::new(50, 3072, 0));
        assert_eq!(discord.credits_per_invite, 0);

        let without: crate::noprofit::Store =
            serde_json::from_value(serde_json::json!({"credits": 0, "catalog": []})).unwrap();
        assert!(without.discord.is_none());
    }

    #[test]
    fn descriptions_are_cut_on_characters() {
        assert_eq!(truncate("äöü", 2), "äö");
        assert_eq!(truncate("abc", 10), "abc");
    }
}
