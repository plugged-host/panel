//! plugged.host: credits, tokens, the memberships and the Resource Store
//! (docs/features/economy.md).
//!
//! Credits buy CPU, memory and disk for good. Without noprofit.space they are the `users.credits`
//! column, the store prices with [`crate::pricing`] and what was bought is `users.extra_*`, added
//! to the games allowance. With noprofit.space connected credits and the store are
//! noprofit.space's: the balance is the account's snapshot, the store is its store, and what was
//! bought arrives with the account's plan. Credits the panel gives or takes then (an admin's
//! change, Discord invite rewards) are sent to noprofit.space ([`crate::noprofit::queue_credit_change`]).
//!
//! Tokens are earned from sponsored links and surveys ([`crate::earn`]) and buy the memberships,
//! which are always the panel's, each for some days at a time: Plus (the `plus` locations) with
//! tokens, Premium (`plusplus` in code: the `premium` and `plus` locations, more CPU, everything
//! Plus gives) with tokens or credits. Neither needs the other. When one runs out the servers
//! leave the locations it opened and the CPU it added comes off ([`settle_memberships`]).
//!
//! Both memberships give **burst cores**: CPU above each server's paid limit (100 % per core) that
//! a server uses while its node has it to spare. They are outside the allowance and the store. The
//! account's count is kept in `users.burst_cores` ([`sync_burst`]), so every server read has it;
//! Wings gets the raised limit and a CPU weight from the paid limit alone, so paid CPU wins when
//! the node is busy ([`cpu_limit_with_burst`], [`cpu_weight`]).

use crate::{
    allowance::Resources,
    models::{ByUuid, user::User},
    pricing::{CatalogEntry, MAX_STEPS, Pricing, StoreResource, UNAFFORDABLE, discounted},
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
/// A membership bought for another account (the buyer's row), and the same seen by the account
/// that got it (a row of 0 tokens).
pub const TYPE_GIFT: &str = "membership_gift";
pub const TYPE_GIFT_RECEIVED: &str = "membership_gift_received";

/// An automatic renewal happens in the last minutes before a membership ends, or this long after
/// its end when the task could not run then.
const RENEW_BEFORE_MINUTES: i32 = 10;
const RENEW_AFTER_MINUTES: i32 = 60;
/// Looking up who a gift goes to, per account and hour (successful gifts have their daily limit).
const GIFT_LOOKUPS_PER_HOUR: u64 = 30;

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

/// One of the two memberships (`plus`, `plusplus`), as routes and settings name them.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MembershipKind {
    Plus,
    Plusplus,
}

impl MembershipKind {
    /// The column prefix in `user_memberships`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Plusplus => "plusplus",
        }
    }

    /// The name users see (a setting).
    pub fn name(self, economy: &crate::settings::economy::AppSettingsEconomy) -> &str {
        match self {
            Self::Plus => economy.plus_name.as_str(),
            Self::Plusplus => economy.plusplus_name.as_str(),
        }
    }

    fn ledger_type(self) -> &'static str {
        match self {
            Self::Plus => TYPE_PLUS,
            Self::Plusplus => TYPE_PLUSPLUS,
        }
    }
}

/// A membership as it is sold now: `cost` is `full_cost` with the membership sale taken off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MembershipOffer {
    pub cost: i64,
    pub full_cost: i64,
    pub days: i64,
}

/// What `kind` costs paid with `pay`, with `percent` off (the sale while it covers the
/// memberships, [`crate::settings::economy::AppSettingsEconomy::membership_discount_at`]);
/// `None` when it is not sold that way (the first only for tokens, the second for credits only
/// while it has a credit price, a currency turned off).
pub fn membership_offer(
    economy: &crate::settings::economy::AppSettingsEconomy,
    kind: MembershipKind,
    pay: Pay,
    percent: i64,
) -> Option<MembershipOffer> {
    let (full_cost, days) = match (kind, pay) {
        (MembershipKind::Plus, Pay::Tokens)
            if economy.tokens_enabled || economy.plus_tokens <= 0 =>
        {
            (
                economy.plus_tokens.max(0) as i64,
                economy.plus_days.max(1) as i64,
            )
        }
        (MembershipKind::Plusplus, Pay::Tokens) if economy.tokens_enabled => (
            economy.plusplus_tokens.max(0) as i64,
            economy.plusplus_token_days.max(1) as i64,
        ),
        (MembershipKind::Plusplus, Pay::Credits)
            if economy.credits_enabled && economy.plusplus_credits > 0 =>
        {
            (
                economy.plusplus_credits as i64,
                economy.plusplus_credit_days.max(1) as i64,
            )
        }
        _ => return None,
    };

    Some(MembershipOffer {
        cost: discounted(full_cost, percent),
        full_cost,
        days,
    })
}

/// How a membership renews by itself: with tokens, or the second with credits when only credits
/// buy it.
fn renewal_pay(
    economy: &crate::settings::economy::AppSettingsEconomy,
    kind: MembershipKind,
) -> Pay {
    if membership_offer(economy, kind, Pay::Tokens, 0).is_some() {
        Pay::Tokens
    } else {
        Pay::Credits
    }
}

#[derive(Clone, Debug, Default)]
pub struct Membership {
    /// When Plus was first bought.
    pub plus_since: Option<chrono::NaiveDateTime>,
    pub plus_until: Option<chrono::NaiveDateTime>,
    pub plus_purchases: i32,
    pub plusplus_until: Option<chrono::NaiveDateTime>,
    pub plusplus_purchases: i32,
    /// plugged.host: each membership renews by itself while on (opt-in).
    pub plus_auto_renew: bool,
    pub plusplus_auto_renew: bool,
    /// plugged.host: Plus that comes with the second membership of the bundle's other panel, until
    /// then (set by that panel, crate::membership_bundle).
    pub bundle_plus_until: Option<chrono::NaiveDateTime>,
}

impl Membership {
    /// Plus runs: bought (by the month) or with the other panel's second membership
    /// ([`Self::bundle_active`]). PlusPlus gives everything Plus does: ask [`Self::is_member`]
    /// for "any membership".
    pub fn has_plus(&self, now: chrono::NaiveDateTime) -> bool {
        self.plus_until.is_some_and(|until| until > now) || self.bundle_active(now)
    }

    /// plugged.host: Plus comes with the membership bundle's other panel now.
    pub fn bundle_active(&self, now: chrono::NaiveDateTime) -> bool {
        self.bundle_plus_until.is_some_and(|until| until > now)
    }

    /// plugged.host: until when Plus runs, bought or with the bundle: the later end.
    pub fn plus_runs_until(&self) -> Option<chrono::NaiveDateTime> {
        self.plus_until.max(self.bundle_plus_until)
    }

    pub fn has_plusplus(&self, now: chrono::NaiveDateTime) -> bool {
        self.plusplus_until.is_some_and(|until| until > now)
    }

    /// Either membership runs.
    pub fn is_member(&self, now: chrono::NaiveDateTime) -> bool {
        self.has_plus(now) || self.has_plusplus(now)
    }

    /// The account's place in the start queue: 2 with PlusPlus, 1 with Plus, 0 without.
    pub fn priority(&self, now: chrono::NaiveDateTime) -> u8 {
        if self.has_plusplus(now) {
            2
        } else if self.has_plus(now) {
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
            Some(TIER_PLUS) => self.is_member(now),
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
        } else if self.has_plus(now) {
            settings.plus_burst_cores
        } else {
            0
        };

        cores.clamp(0, i16::MAX as i32) as i16
    }

    /// When PlusPlus bought now runs out: bought time adds to what is left.
    pub fn extended_until(&self, now: chrono::NaiveDateTime, days: i64) -> chrono::NaiveDateTime {
        extend(self.plusplus_until, now, days)
    }

    /// The same for Plus.
    pub fn plus_extended_until(
        &self,
        now: chrono::NaiveDateTime,
        days: i64,
    ) -> chrono::NaiveDateTime {
        extend(self.plus_until, now, days)
    }

    /// When `kind` runs out (or ran out).
    pub fn until(&self, kind: MembershipKind) -> Option<chrono::NaiveDateTime> {
        match kind {
            MembershipKind::Plus => self.plus_until,
            MembershipKind::Plusplus => self.plusplus_until,
        }
    }

    /// Whether `kind` renews by itself (the account's switch).
    pub fn auto_renew(&self, kind: MembershipKind) -> bool {
        match kind {
            MembershipKind::Plus => self.plus_auto_renew,
            MembershipKind::Plusplus => self.plusplus_auto_renew,
        }
    }
}

/// `days` more from what is left of `until`, or from now when it ran out.
fn extend(
    until: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
    days: i64,
) -> chrono::NaiveDateTime {
    until.filter(|until| *until > now).unwrap_or(now) + chrono::Duration::days(days.max(1))
}

/// The columns [`Membership::from_row`] reads.
const MEMBERSHIP_COLUMNS: &str = concat!(
    "plus_since, plus_until, plus_purchases, plusplus_until, plusplus_purchases, ",
    "plus_auto_renew, plusplus_auto_renew, bundle_plus_until"
);

impl Membership {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            plus_since: row.try_get("plus_since")?,
            plus_until: row.try_get("plus_until")?,
            plus_purchases: row.try_get("plus_purchases")?,
            plusplus_until: row.try_get("plusplus_until")?,
            plusplus_purchases: row.try_get("plusplus_purchases")?,
            plus_auto_renew: row.try_get("plus_auto_renew")?,
            plusplus_auto_renew: row.try_get("plusplus_auto_renew")?,
            bundle_plus_until: row.try_get("bundle_plus_until")?,
        })
    }
}

pub async fn membership(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<Membership, sqlx::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {MEMBERSHIP_COLUMNS} FROM user_memberships WHERE user_uuid = $1"
    )))
    .bind(user_uuid)
    .fetch_optional(database.write())
    .await?;

    match row {
        Some(row) => Membership::from_row(&row),
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
        "SELECT users.uuid, users.burst_cores, user_memberships.plus_until,
            user_memberships.plusplus_until, user_memberships.bundle_plus_until
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
            plus_until: row.try_get("plus_until")?,
            plusplus_until: row.try_get("plusplus_until")?,
            bundle_plus_until: row.try_get("bundle_plus_until")?, // plugged.host: the bundle's Plus
            ..Default::default()
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
/// next minute's run. Its starts waiting in the start queue move up too
/// ([`crate::start_queue::restamp`]).
async fn sync_burst_now(state: &crate::State, user_uuid: uuid::Uuid) {
    if let Err(err) = sync_burst(state, Some(user_uuid)).await {
        tracing::warn!(user = %user_uuid, "failed to sync the burst cores: {err:#}");
    }
    if let Err(err) = crate::start_queue::restamp(state, user_uuid).await {
        tracing::warn!(user = %user_uuid, "failed to move the queued starts up: {err:#}");
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
    /// What it costs now, the membership sale applied.
    pub cost: i64,
    /// What it costs without the sale; `cost` when none runs.
    pub full_cost: i64,
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
    /// When Plus was first bought.
    pub plus_since: Option<chrono::DateTime<chrono::Utc>>,
    /// While Plus runs.
    pub plus_until: Option<chrono::DateTime<chrono::Utc>>,
    /// While PlusPlus runs.
    pub plusplus_until: Option<chrono::DateTime<chrono::Utc>>,
    /// The start queue's priority: 2 PlusPlus, 1 Plus, 0 none.
    pub priority: u8,
    /// Tokens Plus costs for `plus_days` days, the membership sale applied.
    pub plus_cost: i64,
    /// Without the sale; `plus_cost` when none runs.
    pub plus_full_cost: i64,
    pub plus_days: i64,
    pub plusplus_offers: Vec<ApiPlusPlusOffer>,
    /// The sale the membership prices are on, while one covers them.
    pub discount: Option<ApiDiscount>,
    /// Whether accounts may have a membership renew by itself, and whether each of this
    /// account's does.
    pub auto_renew_enabled: bool,
    pub plus_auto_renew: bool,
    pub plusplus_auto_renew: bool,
    /// Whether memberships can be bought for another account, and how many more today.
    pub gifts_enabled: bool,
    pub gifts_left: i64,
    /// Whether the panel costs tokens for accounts without a membership (both memberships
    /// include it).
    pub panel_access: bool,
    /// Whether a start may wait in the ad-funded waiting room: Plus keeps it, PlusPlus does not
    /// (while Fewer Ads for Members is on).
    pub waiting_room: bool,
    /// Whether accounts without a membership see ads in the panel that a membership takes away
    /// (ads on, on the dashboard or server pages, Fewer Ads for Members on).
    pub panel_ads: bool,
    /// Whether Minecraft servers get the in-game message: Plus too, PlusPlus not.
    pub game_ads: bool,
    /// Whether idle servers of members are stopped and parked too (limbo on, members not
    /// spared), and deleted after a while parked.
    pub idle_stop: bool,
    pub idle_delete: bool,
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
    /// What the second membership includes on another panel (its name, such as "switched.cloud
    /// Plus"), while the membership bundle runs (crate::membership_bundle).
    pub bundle_name: Option<compact_str::CompactString>,
    /// While Plus comes with the bundle's other panel (its second membership): until when, and
    /// what that membership is called (such as "switched.cloud Premium").
    pub bundle_plus_until: Option<chrono::DateTime<chrono::Utc>>,
    pub bundle_source_name: compact_str::CompactString,
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
    let (economy, connected, neo_percents, devbox, waiting_room, ads, idle_stop, idle_delete) = {
        let settings = state.settings.get().await?;
        let waiting_room = settings.ads.enabled && settings.ads.room_enabled;
        let ads = (
            settings.ads.enabled
                && (settings.ads.show_account || settings.ads.show_server)
                && settings.ads.member_free,
            settings.ads.in_game_enabled,
        );
        let idle_stop = settings.limbo.enabled
            && !(settings.limbo.exempt_members && settings.economy.membership_enabled);
        let idle_delete = idle_stop && settings.limbo.delete_after_days > 0;
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
            waiting_room,
            ads,
            idle_stop,
            idle_delete,
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
    let now_utc = chrono::Utc::now();
    let now = now_utc.naive_utc();

    // the membership sale, while it covers the memberships
    let discount = ApiDiscount::memberships_at(&economy, now_utc);
    let percent = ApiDiscount::percent(discount.as_ref());
    let plusplus_offers = [Pay::Tokens, Pay::Credits]
        .into_iter()
        .filter_map(|pay| {
            membership_offer(&economy, MembershipKind::Plusplus, pay, percent).map(|offer| {
                ApiPlusPlusOffer {
                    pay,
                    cost: offer.cost,
                    full_cost: offer.full_cost,
                    days: offer.days,
                }
            })
        })
        .collect();
    let plus_offer = membership_offer(&economy, MembershipKind::Plus, Pay::Tokens, percent)
        .unwrap_or(MembershipOffer {
            cost: economy.plus_tokens.max(0) as i64,
            full_cost: economy.plus_tokens.max(0) as i64,
            days: economy.plus_days.max(1) as i64,
        });
    let gifts_enabled = economy.gifts_enabled && economy.tokens_enabled;
    let gifts_left = if gifts_enabled {
        let given = gifts_today(state.database.read(), user.uuid).await?;
        (economy.gifts_per_day.max(1) as i64 - given).max(0)
    } else {
        0
    };

    let burst_cores = membership.burst_cores(&economy, now) as i64;
    let bundle_name =
        crate::membership_bundle::offered(&economy, connected).then(|| economy.bundle_name.clone());
    let bundle_plus_until = membership
        .bundle_plus_until
        .filter(|_| membership.bundle_active(now))
        .map(|until| until.and_utc());
    let bundle_source_name = economy.bundle_source_name.clone();

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
        // plugged.host: Plus bought, not the bundle's (that is `bundle_plus_until`)
        plus_until: membership
            .plus_until
            .filter(|until| *until > now)
            .map(|until| until.and_utc()),
        plusplus_until: membership
            .plusplus_until
            .filter(|_| membership.has_plusplus(now))
            .map(|until| until.and_utc()),
        priority: membership.priority(now),
        plus_cost: plus_offer.cost,
        plus_full_cost: plus_offer.full_cost,
        plus_days: plus_offer.days,
        plusplus_offers,
        discount,
        auto_renew_enabled: economy.auto_renew_enabled,
        plus_auto_renew: membership.plus_auto_renew,
        plusplus_auto_renew: membership.plusplus_auto_renew,
        gifts_enabled,
        gifts_left,
        panel_access: economy.panel_access_enabled,
        waiting_room,
        panel_ads: ads.0,
        game_ads: ads.1,
        idle_stop,
        idle_delete,
        plus_burst_cores: economy.plus_burst_cores.max(0) as i64,
        plusplus_burst_cores: economy.plusplus_burst_cores.max(0) as i64,
        burst_cores,
        neo_plus_percent: neo_percents.map(|(plus, _)| plus),
        neo_plusplus_percent: neo_percents.map(|(_, plusplus)| plusplus),
        devbox,
        bundle_name,
        bundle_plus_until,
        bundle_source_name,
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
    /// The cheapest membership the account does not have, which the tokens row shows progress
    /// toward; `None` with the second membership (it gives everything) or memberships off.
    pub membership_goal: Option<ApiMembershipGoal>,
    /// plugged.host: today's streak tokens wait to be claimed on the Tokens page (shared/src/streak.rs).
    pub streak_claimable: bool,
}

/// A membership the sidebar counts tokens toward.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ApiMembershipGoal {
    pub membership: MembershipKind,
    pub name: compact_str::CompactString,
    /// Tokens it costs now, the membership sale applied.
    pub cost: i64,
}

/// The cheapest membership bought with tokens that the account does not have: none while the
/// second runs (it gives everything the first does), and only one that costs something.
pub fn membership_goal(
    economy: &crate::settings::economy::AppSettingsEconomy,
    membership: &Membership,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<ApiMembershipGoal> {
    let naive = now.naive_utc();
    if !economy.membership_enabled || membership.has_plusplus(naive) {
        return None;
    }
    let percent = economy.membership_discount_at(now);

    [MembershipKind::Plus, MembershipKind::Plusplus]
        .into_iter()
        .filter(|kind| *kind == MembershipKind::Plusplus || !membership.has_plus(naive))
        .filter_map(|kind| {
            membership_offer(economy, kind, Pay::Tokens, percent)
                .filter(|offer| offer.cost > 0)
                .map(|offer| (kind, offer.cost))
        })
        .min_by_key(|(_, cost)| *cost)
        .map(|(kind, cost)| ApiMembershipGoal {
            membership: kind,
            name: kind.name(economy).into(),
            cost,
        })
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
    // plugged.host: progress toward a membership, in the tokens row
    let economy = state.settings.get().await?.economy.clone();
    let membership_goal = if tokens_enabled && economy.membership_enabled {
        membership_goal(
            &economy,
            &membership(&state.database, user.uuid).await?,
            chrono::Utc::now(),
        )
    } else {
        None
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
        membership_goal,
        streak_claimable: crate::streak::claimable(state, user.uuid).await?,
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
    Owned,
    /// The price moved since the player saw it; the store as it is now.
    PriceChanged(Box<ApiStore>),
    /// noprofit.space refused, with its reason.
    Refused(String),
    BadRequest(&'static str),
    /// A membership's price moved since the user saw it (a sale started or ended).
    PriceMoved,
    /// Too many tries (gifts a day, recipients looked up an hour).
    Throttled,
    /// A gift's recipient: no account has that username or email.
    NoRecipient,
    /// Refused as things stand, with the reason (claimed already, a task not done yet).
    Conflict(String),
    Other(anyhow::Error),
}

impl EconomyError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "this is turned off".into(),
            Self::Unavailable => "this is not for sale right now".into(),
            Self::Short => "your balance is too low".into(),
            Self::Owned => "you have this already".into(),
            Self::PriceChanged(_) => "the price changed, look at the new one".into(),
            Self::Refused(reason) if reason.is_empty() => "the purchase was refused".into(),
            Self::Refused(reason) => reason.clone(),
            Self::BadRequest(reason) => (*reason).into(),
            Self::PriceMoved => "the price changed, look at the new one".into(),
            Self::Throttled => "too many tries, try again later".into(),
            Self::NoRecipient => "no account has that username or email".into(),
            Self::Conflict(reason) => reason.clone(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled => 403,
            Self::Unavailable
            | Self::Short
            | Self::Owned
            | Self::PriceChanged(_)
            | Self::PriceMoved => 409,
            Self::Refused(_) => 422,
            Self::BadRequest(_) => 400,
            Self::Throttled => 429,
            Self::NoRecipient => 404,
            Self::Conflict(_) => 409,
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
    /// The sale the prices are on, while one runs.
    pub discount: Option<ApiDiscount>,
}

/// A sale on the Resource Store: percent off every price while its window is open (Admin ›
/// Settings › Economy › Resource Store).
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ApiDiscount {
    pub percent: i64,
    /// What the store calls it, such as "Halloween special"; `None`: a plain sale.
    pub name: Option<String>,
    /// When it ends; `None`: when an admin ends it.
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
}

impl ApiDiscount {
    /// The sale running at `now`, if one is.
    pub fn at(
        economy: &crate::settings::economy::AppSettingsEconomy,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<Self> {
        let percent = economy.store_discount_at(now);

        (percent > 0).then(|| Self {
            percent,
            name: economy
                .store_discount_name
                .as_ref()
                .map(ToString::to_string),
            ends: economy.store_discount_ends,
        })
    }

    /// The same sale on the memberships, while it covers them.
    pub fn memberships_at(
        economy: &crate::settings::economy::AppSettingsEconomy,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<Self> {
        let percent = economy.membership_discount_at(now);

        (percent > 0).then(|| Self {
            percent,
            name: economy
                .store_discount_name
                .as_ref()
                .map(ToString::to_string),
            ends: economy.store_discount_ends,
        })
    }

    fn percent(discount: Option<&Self>) -> i64 {
        discount.map_or(0, |discount| discount.percent)
    }
}

/// plugged.host: a double-token window: every sponsored link and survey pays `percent` more while
/// it runs (Admin › Settings › Economy › Tokens; docs/features/economy.md#double-tokens).
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ApiTokenBonus {
    /// 100: double.
    pub percent: i64,
    /// What it is called, such as "Halloween weekend"; `None`: a plain bonus.
    pub name: Option<String>,
    /// When it started; `None`: open on that side.
    pub starts: Option<chrono::DateTime<chrono::Utc>>,
    /// When it ends; `None`: when an admin ends it.
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
}

/// The double-token window running at `now`, if one is (`None` while tokens are off). What the
/// links and surveys pay goes through [`crate::earn::boosted`] with its percent.
pub fn token_bonus_at(
    economy: &crate::settings::economy::AppSettingsEconomy,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<ApiTokenBonus> {
    let percent = economy.token_bonus_at(now);

    (percent > 0).then(|| ApiTokenBonus {
        percent,
        name: economy.token_bonus_name.as_ref().map(ToString::to_string),
        starts: economy.token_bonus_starts,
        ends: economy.token_bonus_ends,
    })
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

/// The meters' numbers. A noprofit.space plan that shares its pool with other services is shown
/// whole, as the Pelican panel and the sidebar show it (`crate::allowance::with_pool`): what the
/// other services use counts as used, and the limit is the pool.
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
    discount: Option<&ApiDiscount>,
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
    let catalog = panel_catalog(pricing, base, extras, ApiDiscount::percent(discount));

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
        discount: discount.cloned(),
    })
}

fn panel_catalog(
    pricing: &Pricing,
    base: Resources,
    extras: Resources,
    discount: i64,
) -> Vec<CatalogEntry> {
    let mut pricing = *pricing;
    // An unlimited resource is not sold: there is nothing to add to.
    for (index, limit) in [base.cpu, base.memory, base.disk].into_iter().enumerate() {
        if limit <= 0 {
            pricing.steps[index] = 0;
        }
    }

    pricing.catalog(
        [
            base.cpu.max(0) + extras.cpu,
            base.memory.max(0) + extras.memory,
            base.disk.max(0) + extras.disk,
        ],
        discount,
    )
}

/// noprofit.space's answer as the page shows it. The sale is shown only when noprofit.space
/// applied it to the prices (`discount` in its answer).
fn noprofit_store(mut store: crate::noprofit::Store, discount: Option<&ApiDiscount>) -> ApiStore {
    for offer in store.catalog.iter_mut().flat_map(|entry| &mut entry.offers) {
        offer.fill_full_price();
    }

    ApiStore {
        source: Source::Noprofit,
        available: true,
        credits: store.credits,
        discount: discount
            .filter(|discount| store.discount > 0 && discount.percent == store.discount)
            .cloned(),
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
        discount: None,
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
    discount: Option<&ApiDiscount>,
) -> Result<ApiStore, anyhow::Error> {
    let (used, _) = crate::allowance::usage(&state.database, user_uuid).await?;
    let plan = link.and_then(|link| link.plan.as_ref());
    let discord = answer.as_ref().and_then(|store| store.discord.clone());

    let mut page = match answer {
        Some(store) => noprofit_store(store, discount),
        None => ApiStore {
            credits: link.map_or(0, |link| link.credits),
            ..closed_store(Source::Noprofit)
        },
    };

    page.has_plan = plan.is_some();
    let (limit, used) = crate::allowance::with_pool(
        plan.map(Resources::from_plan).unwrap_or_default(),
        used,
        plan.and_then(Resources::elsewhere_in_plan)
            .unwrap_or_default(),
    );
    page.usage = store_usage(limit, used);
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
    let discount = ApiDiscount::at(&economy, chrono::Utc::now());

    match noprofit {
        Some(noprofit) => {
            let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid).await?;
            let answer = match &link {
                Some(link) => match noprofit
                    .store(
                        &state.client,
                        link.account_id,
                        ApiDiscount::percent(discount.as_ref()),
                    )
                    .await
                {
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
                discount.as_ref(),
            )
            .await?)
        }
        None => Ok(panel_store(
            state,
            user.uuid,
            &Pricing::from_settings(&economy),
            base,
            discount.as_ref(),
        )
        .await?),
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
    // The sale as it is now: a price shown before it started or ended has moved.
    let discount = ApiDiscount::at(&economy, chrono::Utc::now());
    let percent = ApiDiscount::percent(discount.as_ref());

    if let Some(noprofit) = noprofit {
        let link = crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
            .await?
            .ok_or(EconomyError::Unavailable)?;

        let answer = noprofit
            .purchase(
                &state.client,
                link.account_id,
                resource,
                steps,
                price,
                percent,
            )
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
                    discount.as_ref(),
                )
                .await?)
            }
            crate::noprofit::Purchase::PriceChanged(Some(store)) => Err(
                EconomyError::PriceChanged(Box::new(noprofit_store(store, discount.as_ref()))),
            ),
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

    let full_cost = pricing.cost(
        resource,
        base_of + extra_of,
        amount,
        pricing.limit(resource),
    );
    let cost = discounted(full_cost, percent);
    if cost != price {
        drop(transaction);

        let store = panel_store(state, user.uuid, &pricing, base, discount.as_ref()).await?;
        return Err(EconomyError::PriceChanged(Box::new(store)));
    }
    if full_cost >= UNAFFORDABLE || i32::try_from(extra_of + amount).is_err() {
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
        serde_json::json!({
            "resource": resource,
            "amount": amount,
            "cost": cost,
            "full_cost": full_cost,
            "discount": percent,
        }),
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

    Ok(panel_store(state, user.uuid, &pricing, base, discount.as_ref()).await?)
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

    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {MEMBERSHIP_COLUMNS} FROM user_memberships WHERE user_uuid = $1 FOR UPDATE"
    )))
    .bind(user_uuid)
    .fetch_one(&mut **transaction)
    .await?;

    Membership::from_row(&row)
}

/// Why a membership is bought, which the ledger and the checks follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// The account buys it for itself.
    Purchase,
    /// It renews by itself as it runs out: only while still switched on and still ending at
    /// `until` (not bought or renewed meanwhile).
    Renewal { until: chrono::NaiveDateTime },
    /// Bought for another account, within the buyer's gifts a day.
    Gift,
}

/// Memberships the account bought for others in the last 24 hours (the gifts' daily limit).
async fn gifts_today(
    executor: impl sqlx::PgExecutor<'_>,
    user_uuid: uuid::Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_token_transactions
        WHERE user_uuid = $1 AND type = $2 AND created > NOW() - INTERVAL '1 day'",
    )
    .bind(user_uuid)
    .bind(TYPE_GIFT)
    .fetch_one(executor)
    .await
}

/// The balance a membership is paid from: the tokens, or the credits (noprofit.space's while it
/// is connected).
async fn balance_in(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    pay: Pay,
) -> Result<i64, anyhow::Error> {
    let (credits, tokens, _) = balances(&state.database, user_uuid).await?;
    if pay == Pay::Tokens {
        return Ok(tokens);
    }

    let connected = state.settings.get().await?.noprofit.connected();
    if connected {
        Ok(
            crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
                .await?
                .map_or(0, |link| link.credits),
        )
    } else {
        Ok(credits)
    }
}

/// Buys (or extends) `kind` for `recipient`, paid by `payer` with `pay`, at the price it has now
/// (the membership sale applied). The balance, the ledger and the membership change together or
/// not at all, with the membership row locked; bought time adds to what is left, so buying early
/// loses nothing. `price` is the price the user saw: one that moved buys nothing (409). With
/// noprofit.space, credits are taken by noprofit.space (`/credits/spend`) with an idempotency key
/// per purchase, so a retry after a failure is not charged twice.
async fn buy_membership(
    state: &crate::State,
    payer: &User,
    recipient: &User,
    kind: MembershipKind,
    pay: Pay,
    price: Option<i64>,
    why: Why,
) -> Result<MembershipOffer, EconomyError> {
    let (economy, noprofit) = {
        let settings = state.settings.get().await?;
        (settings.economy.clone(), settings.noprofit.config())
    };
    if !economy.membership_enabled {
        return Err(EconomyError::Disabled);
    }

    let percent = economy.membership_discount_at(chrono::Utc::now());
    let offer = membership_offer(&economy, kind, pay, percent).ok_or(EconomyError::Unavailable)?;
    // a price shown before a sale started or ended has moved
    if price.is_some_and(|price| price != offer.cost) {
        return Err(EconomyError::PriceMoved);
    }

    let name = kind.name(&economy).to_string();
    let days = offer.days;
    let (ledger_type, description) = match why {
        Why::Purchase => (kind.ledger_type(), format!("{name} ({days} days)")),
        Why::Renewal { .. } => (
            kind.ledger_type(),
            format!("{name} ({days} days), renewed automatically"),
        ),
        Why::Gift => (
            TYPE_GIFT,
            format!("{name} ({days} days) for {}", recipient.username),
        ),
    };
    let data = serde_json::json!({
        "days": days,
        "pay": pay,
        "membership": kind,
        "cost": offer.cost,
        "full_cost": offer.full_cost,
        "discount": percent,
        "automatic": matches!(why, Why::Renewal { .. }),
        "to": (why == Why::Gift).then_some(recipient.uuid),
    });

    let mut transaction = state.database.write().begin().await?;
    let membership = locked_membership(&mut transaction, recipient.uuid).await?;

    if let Why::Renewal { until } = why
        && (!membership.auto_renew(kind) || membership.until(kind) != Some(until))
    {
        // switched off, bought or renewed meanwhile
        return Err(EconomyError::Owned);
    }
    if why == Why::Gift {
        // The buyer's row, so two gifts at once cannot both pass the day's limit. NO KEY UPDATE,
        // like the balance change after it, so the ledger rows of a gift going the other way at
        // the same time (which only need the key) never wait on it.
        sqlx::query("SELECT 1 FROM users WHERE uuid = $1 FOR NO KEY UPDATE")
            .bind(payer.uuid)
            .execute(&mut *transaction)
            .await?;
        if gifts_today(&mut *transaction, payer.uuid).await? >= economy.gifts_per_day.max(1) as i64
        {
            return Err(EconomyError::Throttled);
        }
    }

    let mut spent_on_noprofit = false;
    if offer.cost > 0 || why == Why::Gift {
        match (pay, noprofit) {
            (Pay::Credits, Some(noprofit)) => {
                let link = crate::noprofit::Link::by_user_uuid(&state.database, payer.uuid)
                    .await?
                    .ok_or(EconomyError::Unavailable)?;
                // The same purchase sent again (the panel failed after noprofit.space took the
                // credits) is not charged twice.
                let purchases = match kind {
                    MembershipKind::Plus => membership.plus_purchases,
                    MembershipKind::Plusplus => membership.plusplus_purchases,
                };
                let key = format!(
                    "calagopus:{}:{}:{}:{}",
                    kind.as_str(),
                    payer.uuid,
                    purchases + 1,
                    offer.cost
                );

                let spent = noprofit
                    .spend_credits(
                        &state.client,
                        link.account_id,
                        offer.cost,
                        ledger_type,
                        &description,
                        &key,
                    )
                    .await
                    .map_err(|err| {
                        tracing::warn!(user = %payer.uuid, "noprofit.space did not take the credits for a membership: {err:#}");
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
                    payer.uuid,
                    -offer.cost,
                    ledger_type,
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
    let (extend_sql, until) = match kind {
        MembershipKind::Plus => (
            "UPDATE user_memberships
            SET plus_since = COALESCE(plus_since, NOW()), plus_until = $2,
                plus_purchases = plus_purchases + 1
            WHERE user_uuid = $1",
            membership.plus_extended_until(now, days),
        ),
        MembershipKind::Plusplus => (
            "UPDATE user_memberships
            SET plusplus_until = $2, plusplus_purchases = plusplus_purchases + 1
            WHERE user_uuid = $1",
            membership.extended_until(now, days),
        ),
    };
    sqlx::query(extend_sql)
        .bind(recipient.uuid)
        .bind(until)
        .execute(&mut *transaction)
        .await?;

    if why == Why::Gift {
        // the account that got it sees it in its token history too, as a row of 0 tokens
        let received = format!("{name} ({days} days) from {}", payer.username);
        sqlx::query(
            "INSERT INTO user_token_transactions
                (user_uuid, amount, balance_after, type, description, data)
            SELECT uuid, 0, tokens, $2, $3, $4 FROM users WHERE uuid = $1",
        )
        .bind(recipient.uuid)
        .bind(TYPE_GIFT_RECEIVED)
        .bind(truncate(&received, 255))
        .bind(serde_json::json!({ "days": days, "membership": kind, "from": payer.uuid }))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    sync_burst_now(state, recipient.uuid).await;
    User::invalidate_cached(&state.database, recipient.uuid).await;

    if spent_on_noprofit && let Err(err) = crate::noprofit::refresh_user(state, payer.uuid).await {
        tracing::warn!(user = %payer.uuid, "failed to pull the account after a membership: {err:#}");
    }

    Ok(offer)
}

/// Buys (or extends) Plus with tokens. `price` is the price the user saw (`None`: what it costs
/// now).
pub async fn buy_plus(
    state: &crate::State,
    user: &User,
    price: Option<i64>,
) -> Result<ApiEconomy, EconomyError> {
    buy_membership(
        state,
        user,
        user,
        MembershipKind::Plus,
        Pay::Tokens,
        price,
        Why::Purchase,
    )
    .await?;

    Ok(summary(state, user).await?)
}

/// Buys (or extends) PlusPlus with tokens or credits. It gives everything Plus does, so it needs
/// no Plus.
pub async fn buy_plusplus(
    state: &crate::State,
    user: &User,
    pay: Pay,
    price: Option<i64>,
) -> Result<ApiEconomy, EconomyError> {
    buy_membership(
        state,
        user,
        user,
        MembershipKind::Plusplus,
        pay,
        price,
        Why::Purchase,
    )
    .await?;

    Ok(summary(state, user).await?)
}

/// A membership bought for another account.
pub struct GiftSent {
    pub recipient: User,
    pub offer: MembershipOffer,
    /// The buyer's economy after it.
    pub economy: ApiEconomy,
}

/// Buys `kind` with tokens, at its normal (sale) price, for the account whose username or email
/// is exactly `to` (not the buyer's own): its membership is extended like after a purchase, it
/// hears who sent it, and both accounts get a ledger row and an activity entry. Only memberships
/// are given this way, never tokens. The day's gifts are limited per buyer, and so are lookups of
/// who a gift is for; the balance is checked before anyone is looked up, so an account that
/// cannot pay learns nothing about who exists.
pub async fn gift_membership(
    state: &crate::State,
    user: &User,
    to: &str,
    kind: MembershipKind,
    price: Option<i64>,
) -> Result<GiftSent, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.membership_enabled || !economy.gifts_enabled {
        return Err(EconomyError::Disabled);
    }

    let percent = economy.membership_discount_at(chrono::Utc::now());
    let offer =
        membership_offer(&economy, kind, Pay::Tokens, percent).ok_or(EconomyError::Unavailable)?;
    if price.is_some_and(|price| price != offer.cost) {
        return Err(EconomyError::PriceMoved);
    }
    if balances(&state.database, user.uuid).await?.1 < offer.cost {
        return Err(EconomyError::Short);
    }
    if gifts_today(state.database.read(), user.uuid).await? >= economy.gifts_per_day.max(1) as i64 {
        return Err(EconomyError::Throttled);
    }
    if state
        .cache
        .ratelimit(
            "economy::gift",
            GIFT_LOOKUPS_PER_HOUR,
            3600,
            user.uuid.to_string(),
        )
        .await
        .is_err()
    {
        return Err(EconomyError::Throttled);
    }

    let to = to.trim();
    if to.is_empty() {
        return Err(EconomyError::BadRequest(
            "enter the username or email of who it is for",
        ));
    }
    let mut recipient = User::by_username(&state.database, to).await?;
    if recipient.is_none() && to.contains('@') {
        recipient = User::by_email(&state.database, to).await?;
    }
    let Some(recipient) = recipient.filter(|recipient| !recipient.suspended) else {
        return Err(EconomyError::NoRecipient);
    };
    if recipient.uuid == user.uuid {
        return Err(EconomyError::BadRequest(
            "a gift is for someone else, buy it for yourself instead",
        ));
    }

    // charged at the price checked above
    let offer = buy_membership(
        state,
        user,
        &recipient,
        kind,
        Pay::Tokens,
        Some(offer.cost),
        Why::Gift,
    )
    .await?;

    let until = membership(&state.database, recipient.uuid)
        .await?
        .until(kind);
    crate::notifications::membership_gifted(
        state,
        recipient.uuid,
        kind.name(&economy),
        offer.days,
        &user.username,
        until,
    )
    .await;

    {
        use crate::models::{
            CreatableModel,
            user_activity::{CreateUserActivityOptions, UserActivity},
        };

        // the buyer's entry is the route's; this is the one of the account that got it
        let options = CreateUserActivityOptions {
            user_uuid: recipient.uuid,
            impersonator_uuid: None,
            api_key_uuid: None,
            event: "account:membership.gifted".into(),
            ip: None,
            data: serde_json::json!({
                "membership": kind,
                "days": offer.days,
                "from": user.username,
            }),
            created: None,
        };
        if let Err(err) = UserActivity::create(state, options).await {
            tracing::warn!(user = %recipient.uuid, "failed to log a gift in the activity: {err:#?}");
        }
    }

    Ok(GiftSent {
        recipient,
        offer,
        economy: summary(state, user).await?,
    })
}

/// Switches the account's automatic renewal of `kind` on or off. On needs the membership running
/// and clears a failed renewal, so the next minutes try again.
pub async fn set_auto_renew(
    state: &crate::State,
    user: &User,
    kind: MembershipKind,
    enabled: bool,
) -> Result<ApiEconomy, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if enabled {
        if !economy.membership_enabled || !economy.auto_renew_enabled {
            return Err(EconomyError::Disabled);
        }

        let now = chrono::Utc::now().naive_utc();
        if !membership(&state.database, user.uuid)
            .await?
            .until(kind)
            .is_some_and(|until| until > now)
        {
            return Err(EconomyError::BadRequest("the membership is not running"));
        }
    }

    let column = kind.as_str();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE user_memberships SET {column}_auto_renew = $2, {column}_renew_failed = NULL
        WHERE user_uuid = $1"
    )))
    .bind(user.uuid)
    .bind(enabled)
    .execute(state.database.write())
    .await?;

    Ok(summary(state, user).await?)
}

/// Days of a membership given away (rux's giveaways, shared/src/rux/tools/giveaways), in the
/// caller's transaction: they add to what is left, like a purchase, without counting as one.
/// Call [`membership_changed`] once the transaction is committed.
pub async fn give_membership_days(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
    plusplus: bool,
    days: i64,
) -> Result<(), sqlx::Error> {
    let membership = locked_membership(transaction, user_uuid).await?;
    let now = chrono::Utc::now().naive_utc();

    if plusplus {
        sqlx::query("UPDATE user_memberships SET plusplus_until = $2 WHERE user_uuid = $1")
            .bind(user_uuid)
            .bind(membership.extended_until(now, days))
            .execute(&mut **transaction)
            .await?;
    } else {
        sqlx::query(
            "UPDATE user_memberships
            SET plus_since = COALESCE(plus_since, NOW()), plus_until = $2
            WHERE user_uuid = $1",
        )
        .bind(user_uuid)
        .bind(membership.plus_extended_until(now, days))
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

/// What follows a membership change made outside this module ([`give_membership_days`]): the
/// burst cores, the start queue and the cached user.
pub async fn membership_changed(state: &crate::State, user_uuid: uuid::Uuid) {
    sync_burst_now(state, user_uuid).await;
    User::invalidate_cached(&state.database, user_uuid).await;
}

/// What an admin sees of a user's economy.
#[derive(ToSchema, Serialize)]
pub struct ApiAdminEconomy {
    #[serde(flatten)]
    pub summary: ApiEconomy,
    /// The panel's own credits (without noprofit.space the balance, with it unused).
    pub panel_credits: i64,
    /// Whether an admin can change the credits here: always without noprofit.space; with it
    /// while its account sync is on and the user has an account there (the change is sent to it).
    pub credits_changeable: bool,
    /// What the panel's store sold the account.
    pub extras: Resources,
    pub plusplus_purchases: i32,
    pub credit_transactions: Vec<ApiTransaction>,
    pub token_transactions: Vec<ApiTransaction>,
    /// plugged.host: who brought the account and how many it brought (docs/features/referrals.md).
    pub referrals: crate::referrals::ApiAdminReferrals,
}

pub async fn admin_view(
    state: &crate::State,
    user: &User,
) -> Result<ApiAdminEconomy, anyhow::Error> {
    let summary = summary(state, user).await?;
    let (panel_credits, _, _) = balances(&state.database, user.uuid).await?;
    let noprofit = {
        let settings = state.settings.get().await?;
        settings
            .noprofit
            .config()
            .map(|noprofit| noprofit.sync_enabled())
    };
    let (credits_changeable, credit_transactions) = match noprofit {
        None => (
            true,
            transactions(&state.database, Currency::Credits, user.uuid, 20).await?,
        ),
        Some(sync) => (
            sync && crate::noprofit::Link::by_user_uuid(&state.database, user.uuid)
                .await?
                .is_some_and(|link| link.deleted.is_none()),
            noprofit_credit_changes(&state.database, user.uuid, 20).await?,
        ),
    };

    Ok(ApiAdminEconomy {
        summary,
        panel_credits,
        credits_changeable,
        extras: extras(&state.database, user.uuid).await?,
        plusplus_purchases: membership(&state.database, user.uuid)
            .await?
            .plusplus_purchases,
        credit_transactions,
        token_transactions: transactions(&state.database, Currency::Tokens, user.uuid, 20).await?,
        referrals: crate::referrals::admin_view(&state.database, user.uuid).await?,
    })
}

/// The credit changes made here that noprofit.space applied (with noprofit.space connected the
/// panel's credit history), newest first.
async fn noprofit_credit_changes(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
    limit: i64,
) -> Result<Vec<ApiTransaction>, sqlx::Error> {
    let rows: Vec<(i64, i64, String, String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT applied, balance_after, type, description, created
        FROM user_noprofit_credit_changes
        WHERE user_uuid = $1 AND applied IS NOT NULL AND balance_after IS NOT NULL
        ORDER BY created DESC
        LIMIT $2",
    )
    .bind(user_uuid)
    .bind(limit)
    .fetch_all(database.read())
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(amount, balance_after, kind, description, created)| ApiTransaction {
                amount,
                balance_after,
                kind: kind.into(),
                description,
                created: Some(created.and_utc()),
            },
        )
        .collect())
}

/// An admin's change to a user's economy; every field optional.
#[derive(Default)]
pub struct AdminChange {
    /// Added to the panel's credits (negative takes away); with noprofit.space connected sent to
    /// the account there.
    pub credits: Option<i64>,
    pub tokens: Option<i64>,
    /// Shown in the user's history.
    pub reason: Option<String>,
    /// `Some(None)` ends Plus now.
    pub plus_until: Option<Option<chrono::DateTime<chrono::Utc>>>,
    /// `Some(None)` ends PlusPlus now.
    pub plusplus_until: Option<Option<chrono::DateTime<chrono::Utc>>>,
}

pub async fn admin_change(
    state: &crate::State,
    user: &User,
    change_request: AdminChange,
) -> Result<(), EconomyError> {
    // plugged.host: with noprofit.space the credits are its own; a change goes to it
    let noprofit = state
        .settings
        .get()
        .await?
        .noprofit
        .config()
        .map(|noprofit| noprofit.sync_enabled());
    let credits_change = change_request.credits.is_some_and(|credits| credits != 0);
    let credits_to_noprofit = match noprofit {
        Some(true) => credits_change,
        Some(false) if credits_change => {
            return Err(EconomyError::BadRequest(
                "credits are noprofit.space's and its account sync is off: change them there",
            ));
        }
        _ => false,
    };

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

        if currency == Currency::Credits && credits_to_noprofit {
            let Some((account_id, balance)) =
                crate::noprofit::pending_balance(&mut transaction, user.uuid).await?
            else {
                return Err(EconomyError::BadRequest(
                    "the user has no noprofit.space account",
                ));
            };
            if balance + amount < 0 {
                return Err(EconomyError::BadRequest("the balance would go below zero"));
            }

            crate::noprofit::queue_credit_change(
                &mut transaction,
                user.uuid,
                account_id,
                amount,
                TYPE_ADMIN,
                reason,
                serde_json::json!({}),
            )
            .await?;
            continue;
        }

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

    if change_request.plus_until.is_some() || change_request.plusplus_until.is_some() {
        locked_membership(&mut transaction, user.uuid).await?;

        if let Some(until) = change_request.plus_until {
            // Ended by an admin: the servers leave the plus locations like after a lapse.
            let until = until
                .map(|until| until.naive_utc())
                .unwrap_or_else(|| chrono::Utc::now().naive_utc());

            sqlx::query(
                "UPDATE user_memberships
                SET plus_until = $2, plus_since = COALESCE(plus_since, NOW())
                WHERE user_uuid = $1",
            )
            .bind(user.uuid)
            .bind(until)
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

    // sent now, so the admin sees the balance noprofit.space answers; the account sync retries
    if credits_to_noprofit && let Err(err) = crate::noprofit::push_credit_changes(state).await {
        tracing::warn!(user = %user.uuid, "failed to send an admin's credit change to noprofit.space: {err:#}");
    }

    Ok(())
}

/// The order a server leaving a location its owner lost tries the other locations in: the same
/// country (flag) first, Plus before free (when Plus still runs), then the fastest.
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

/// Renews every membership switched to renew by itself that ends in the next minutes (or ended a
/// little while ago, when the task could not run then), at the price it has now (the sale
/// applied), paid like a purchase. When the balance is short the owner hears so once per end, and
/// the lapse takes its course.
async fn renew_memberships(state: &crate::State) -> Result<(), anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.membership_enabled || !economy.auto_renew_enabled {
        return Ok(());
    }

    for kind in [MembershipKind::Plusplus, MembershipKind::Plus] {
        let column = kind.as_str();
        let due: Vec<(uuid::Uuid, chrono::NaiveDateTime)> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT user_uuid, {column}_until FROM user_memberships
                WHERE {column}_auto_renew AND {column}_until IS NOT NULL
                    AND {column}_until <= NOW() + make_interval(mins => $1)
                    AND {column}_until > NOW() - make_interval(mins => $2)
                    AND {column}_renew_failed IS DISTINCT FROM {column}_until
                ORDER BY {column}_until
                LIMIT 50"
            )))
            .bind(RENEW_BEFORE_MINUTES)
            .bind(RENEW_AFTER_MINUTES)
            .fetch_all(state.database.write())
            .await?;

        for (user_uuid, until) in due {
            if let Err(err) = renew(state, &economy, user_uuid, kind, until).await {
                tracing::warn!(user = %user_uuid, membership = column, "failed to renew a membership: {err:#}");
            }
        }
    }

    Ok(())
}

async fn renew(
    state: &crate::State,
    economy: &crate::settings::economy::AppSettingsEconomy,
    user_uuid: uuid::Uuid,
    kind: MembershipKind,
    until: chrono::NaiveDateTime,
) -> Result<(), anyhow::Error> {
    // plugged.host: Plus bought is not renewed while the bundle's Plus covers its end (the
    // account loses nothing; crate::membership_bundle)
    if kind == MembershipKind::Plus
        && membership(&state.database, user_uuid)
            .await?
            .bundle_active(until)
    {
        return Ok(());
    }

    let pay = renewal_pay(economy, kind);
    let user = User::by_uuid_optional(&state.database, user_uuid).await?;

    // a suspended account is not charged; it is marked like a short one, without a notice
    let failed = match user.as_ref().filter(|user| !user.suspended) {
        Some(user) => {
            match buy_membership(state, user, user, kind, pay, None, Why::Renewal { until }).await {
                Ok(offer) => {
                    tracing::info!(user = %user_uuid, membership = kind.as_str(), cost = offer.cost, "renewed a membership automatically");
                    return Ok(());
                }
                // switched off, bought or renewed meanwhile
                Err(EconomyError::Owned) => return Ok(()),
                Err(EconomyError::Other(err)) => return Err(err),
                Err(err) => Some(err),
            }
        }
        None => None,
    };

    // tried once per end
    let column = kind.as_str();
    let marked = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE user_memberships SET {column}_renew_failed = {column}_until
        WHERE user_uuid = $1 AND {column}_until = $2"
    )))
    .bind(user_uuid)
    .bind(until)
    .execute(state.database.write())
    .await?
    .rows_affected();

    if marked > 0 && matches!(failed, Some(EconomyError::Short)) {
        let percent = economy.membership_discount_at(chrono::Utc::now());
        let cost = membership_offer(economy, kind, pay, percent).map_or(0, |offer| offer.cost);
        let missing = (cost - balance_in(state, user_uuid, pay).await?).max(1);

        crate::notifications::membership_renew_failed(
            state,
            user_uuid,
            kind.name(economy),
            missing,
            pay,
            until,
        )
        .await;
    } else if let Some(err) = failed {
        tracing::info!(user = %user_uuid, membership = column, "a membership did not renew: {}", err.message());
    }

    Ok(())
}

/// Tells each account, once per end, that a membership ends within the notice's hours: when,
/// whether it renews by itself and whether the balance covers it; then the same for paid panel
/// access ([`crate::panel_access::remind_ending`]).
async fn remind_ending(state: &crate::State) -> Result<(), anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.ends_soon_enabled {
        return Ok(());
    }
    let hours = economy.ends_soon_hours.clamp(1, 720);

    if economy.membership_enabled {
        for kind in [MembershipKind::Plus, MembershipKind::Plusplus] {
            let column = kind.as_str();
            let due: Vec<(uuid::Uuid, chrono::NaiveDateTime)> =
                sqlx::query_as(sqlx::AssertSqlSafe(format!(
                    "SELECT user_uuid, {column}_until FROM user_memberships
                    WHERE {column}_until > NOW()
                        AND {column}_until <= NOW() + make_interval(hours => $1)
                        AND {column}_reminded IS DISTINCT FROM {column}_until
                    ORDER BY {column}_until
                    LIMIT 100"
                )))
                .bind(hours)
                .fetch_all(state.database.write())
                .await?;

            for (user_uuid, until) in due {
                if let Err(err) = remind_membership(state, &economy, user_uuid, kind, until).await {
                    tracing::warn!(user = %user_uuid, membership = column, "failed to send an ends-soon notice: {err:#}");
                }
            }
        }
    }

    crate::panel_access::remind_ending(state, &economy, hours).await
}

async fn remind_membership(
    state: &crate::State,
    economy: &crate::settings::economy::AppSettingsEconomy,
    user_uuid: uuid::Uuid,
    kind: MembershipKind,
    until: chrono::NaiveDateTime,
) -> Result<(), anyhow::Error> {
    // marked first, so it goes out once even when sending it fails
    let column = kind.as_str();
    let marked = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE user_memberships SET {column}_reminded = {column}_until
        WHERE user_uuid = $1 AND {column}_until = $2"
    )))
    .bind(user_uuid)
    .bind(until)
    .execute(state.database.write())
    .await?
    .rows_affected();
    if marked == 0 {
        return Ok(());
    }

    let membership = membership(&state.database, user_uuid).await?;
    // the second runs past the first's end: the first ending changes nothing
    if kind == MembershipKind::Plus && membership.has_plusplus(until) {
        return Ok(());
    }
    // plugged.host: so does the bundle's Plus running past it (crate::membership_bundle)
    if kind == MembershipKind::Plus && membership.bundle_active(until) {
        return Ok(());
    }

    let pay = renewal_pay(economy, kind);
    let percent = economy.membership_discount_at(chrono::Utc::now());
    let Some(offer) = membership_offer(economy, kind, pay, percent) else {
        // not for sale any more: nothing to renew
        return Ok(());
    };
    let covered = balance_in(state, user_uuid, pay).await? >= offer.cost;

    crate::notifications::membership_ends_soon(
        state,
        user_uuid,
        kind.name(economy),
        until,
        economy.auto_renew_enabled && membership.auto_renew(kind),
        covered,
        offer.cost,
        pay,
    )
    .await;

    Ok(())
}

/// Carries out every membership that ran out: every server on a location the owner may no longer
/// use (`premium` once PlusPlus ran out, `plus` once both did) moves to another location the
/// owner may use (an admin move, a few at a time). A lapse is settled once no such server is
/// left; until then the task comes back, so a full location is tried again. Buying again before
/// that stops it. Then every account's burst cores are brought in line ([`sync_burst`]).
///
/// First, memberships switched to renew by itself renew as they run out ([`renew_memberships`]),
/// so a renewed one never lapses, and ends-soon notices go out ([`remind_ending`]).
pub async fn settle_memberships(state: &crate::State) -> Result<(), anyhow::Error> {
    if let Err(err) = renew_memberships(state).await {
        tracing::warn!("failed to renew memberships: {err:#}");
    }
    if let Err(err) = remind_ending(state).await {
        tracing::warn!("failed to send ends-soon notices: {err:#}");
    }

    let due = sqlx::query(
        "SELECT user_uuid FROM user_memberships
        WHERE (plusplus_until IS NOT NULL AND plusplus_until <= NOW()
                AND (plusplus_settled IS NULL OR plusplus_settled < plusplus_until))
            OR (plus_until IS NOT NULL AND plus_until <= NOW()
                AND (plus_settled IS NULL OR plus_settled < plus_until))
            -- plugged.host: the bundle's Plus ending is a lapse of Plus too (crate::membership_bundle)
            OR (bundle_plus_until IS NOT NULL AND bundle_plus_until <= NOW()
                AND (plus_settled IS NULL OR plus_settled < bundle_plus_until))
        ORDER BY LEAST(plusplus_until, plus_until, bundle_plus_until)
        LIMIT 50",
    )
    .fetch_all(state.database.write())
    .await?;

    for row in due {
        let user_uuid: uuid::Uuid = row.try_get("user_uuid")?;

        if let Err(err) = settle_lapse(state, user_uuid).await {
            tracing::warn!(user = %user_uuid, "failed to settle a lapsed membership: {err:#}");
        }
    }

    // a membership that ran out drops to the other's burst cores, or none
    sync_burst(state, None).await
}

/// The location tiers a lapse takes away from an account with `membership` now.
pub fn lost_tiers(membership: &Membership, now: chrono::NaiveDateTime) -> Vec<&'static str> {
    [TIER_PREMIUM, TIER_PLUS]
        .into_iter()
        .filter(|tier| !membership.unlocks(Some(tier), now))
        .collect()
}

async fn settle_lapse(state: &crate::State, user_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let Some(user) = User::by_uuid_optional(&state.database, user_uuid).await? else {
        return Ok(());
    };
    let now = chrono::Utc::now().naive_utc();
    let membership = membership(&state.database, user_uuid).await?;
    let lost: Vec<String> = lost_tiers(&membership, now)
        .into_iter()
        .map(str::to_string)
        .collect();
    // which one the owner is told ended: the second when it ran out, else the first
    let second = membership.plusplus_until.is_some_and(|until| until <= now);

    // The owner's servers on locations it lost, and whether a move of each is under way.
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
        WHERE servers.owner_uuid = $1 AND locations.tier = ANY($2)
        "#,
    )
    .bind(user_uuid)
    .bind(&lost)
    .fetch_all(state.database.write())
    .await?;

    if servers.is_empty() {
        sqlx::query(
            "UPDATE user_memberships
            SET plusplus_settled = CASE WHEN plusplus_until <= NOW() THEN NOW()
                    ELSE plusplus_settled END,
                plus_settled = CASE WHEN plus_until <= NOW() OR bundle_plus_until <= NOW()
                    THEN NOW() ELSE plus_settled END
            WHERE user_uuid = $1",
        )
        .bind(user_uuid)
        .execute(state.database.write())
        .await?;

        return Ok(());
    }

    let locations = crate::server_creation::locations(state, &user).await?;

    let mut moving = Vec::new(); // plugged.host: for the owner's notification
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
                    && !location
                        .tier
                        .as_deref()
                        .is_some_and(|tier| lost.iter().any(|lost| lost == tier))
            })
            .min_by_key(|location| destination_order(flag.as_deref(), location))
        else {
            tracing::info!(server = %server_uuid, "a membership ran out, but no other location takes the server yet");
            continue;
        };

        let Some(server) =
            crate::models::server::Server::by_uuid_optional(&state.database, server_uuid).await?
        else {
            continue;
        };

        crate::server_transfers::queue_admin_move(state, &server, destination.uuid).await?;
        tracing::info!(server = %server_uuid, location = %destination.uuid, "a membership ran out, moving the server off a location it opened");
        moving.push(server.name.to_string());
    }
    crate::notifications::membership_ended(state, user_uuid, second, &moving).await;

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
            plus_until: Some(at(10)),
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
        let plusplus_alone = Membership {
            plusplus_until: Some(at(5)),
            ..Default::default()
        };
        let plus_lapsed = Membership {
            plus_until: Some(at(-1)),
            ..plus.clone()
        };

        assert_eq!(Membership::default().burst_cores(&settings, now), 0);
        assert_eq!(plus.burst_cores(&settings, now), 1);
        // the total, not 1 + 2
        assert_eq!(plusplus.burst_cores(&settings, now), 2);
        assert_eq!(plusplus_alone.burst_cores(&settings, now), 2);
        assert_eq!(lapsed.burst_cores(&settings, now), 1);
        assert_eq!(plus_lapsed.burst_cores(&settings, now), 0);
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
            plus_until: Some(at(10)),
            plus_purchases: 1,
            ..Default::default()
        };
        let plusplus = Membership {
            plusplus_until: Some(at(5)),
            plusplus_purchases: 1,
            ..plus.clone()
        };
        let lapsed = Membership {
            plusplus_until: Some(at(-1)),
            ..plusplus.clone()
        };
        // PlusPlus gives everything Plus does, without it
        let plusplus_alone = Membership {
            plusplus_until: Some(at(5)),
            plusplus_purchases: 1,
            ..Default::default()
        };
        // Plus bought by the month runs out like PlusPlus
        let plus_lapsed = Membership {
            plus_until: Some(at(-1)),
            ..plus.clone()
        };

        for membership in [&none, &plus, &plusplus, &plusplus_alone] {
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
        assert!(plusplus_alone.unlocks(Some(TIER_PLUS), now));
        assert!(plusplus_alone.unlocks(Some(TIER_PREMIUM), now));
        assert!(!plus_lapsed.unlocks(Some(TIER_PLUS), now));
        assert!(!plus_lapsed.is_member(now));
        assert!(plusplus_alone.is_member(now));

        assert_eq!(none.priority(now), 0);
        assert_eq!(plus.priority(now), 1);
        assert_eq!(plusplus.priority(now), 2);
        assert_eq!(plusplus_alone.priority(now), 2);
        assert_eq!(lapsed.priority(now), 1);
        assert_eq!(plus_lapsed.priority(now), 0);

        // what a lapse takes away
        assert!(lost_tiers(&plusplus, now).is_empty());
        assert!(lost_tiers(&plusplus_alone, now).is_empty());
        assert_eq!(lost_tiers(&lapsed, now), vec![TIER_PREMIUM]);
        assert_eq!(lost_tiers(&plus_lapsed, now), vec![TIER_PREMIUM, TIER_PLUS]);
        assert_eq!(lost_tiers(&none, now), vec![TIER_PREMIUM, TIER_PLUS]);
    }

    #[test]
    fn memberships_bought_early_add_to_what_is_left() {
        let now = at(0);
        let running = Membership {
            plus_since: Some(at(-10)),
            plus_until: Some(at(24)),
            plus_purchases: 1,
            plusplus_until: Some(at(48)),
            plusplus_purchases: 1,
            ..Default::default()
        };
        let lapsed = Membership {
            plus_until: Some(at(-24)),
            plusplus_until: Some(at(-48)),
            ..running.clone()
        };

        assert_eq!(running.extended_until(now, 7), at(48 + 7 * 24));
        assert_eq!(lapsed.extended_until(now, 7), at(7 * 24));
        assert_eq!(Membership::default().extended_until(now, 0), at(24));

        assert_eq!(running.plus_extended_until(now, 30), at(24 + 30 * 24));
        assert_eq!(lapsed.plus_extended_until(now, 30), at(30 * 24));
        assert_eq!(
            Membership::default().plus_extended_until(now, 30),
            at(30 * 24)
        );
    }

    #[test]
    fn unlimited_resources_are_not_sold() {
        let pricing =
            Pricing::from_settings(&crate::settings::economy::AppSettingsEconomy::default());
        let catalog = panel_catalog(
            &pricing,
            Resources::new(200, 0, 10240),
            Resources::new(50, 0, 0),
            0,
        );

        assert_eq!(catalog[0].total, 250);
        assert!(!catalog[0].offers.is_empty());
        assert!(catalog[1].offers.is_empty());
        assert_eq!(catalog[2].total, 10240);
        assert!(!catalog[2].offers.is_empty());
    }

    #[test]
    fn a_sale_noprofit_applied_reaches_the_page() {
        // what accounts answers during a 50 % sale
        let answer: crate::noprofit::Store = serde_json::from_value(serde_json::json!({
            "credits": 500,
            "catalog": [{"resource": "memory", "step": 1024, "total": 4096, "limit": 24576,
                "soft_limit": 30720, "offers": [{"steps": 1, "amount": 1024, "price": 38, "full_price": 75}]}],
            "discount": 50,
            "transactions": [],
        }))
        .unwrap();
        let sale = ApiDiscount {
            percent: 50,
            name: Some("Halloween special".into()),
            ends: None,
        };

        let page = serde_json::to_value(noprofit_store(answer.clone(), Some(&sale))).unwrap();
        assert_eq!(page["discount"]["percent"], 50);
        assert_eq!(page["catalog"][0]["offers"][0]["full_price"], 75);
        assert_eq!(page["catalog"][0]["offers"][0]["price"], 38);

        // a sale noprofit.space did not apply is not shown
        let other = ApiDiscount {
            percent: 25,
            ..sale
        };
        assert!(noprofit_store(answer, Some(&other)).discount.is_none());
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
    fn a_shared_pool_shows_whole() {
        // noprofit.space's planFor(): this panel's share, the whole pool and what others use of it
        let plan = serde_json::json!({
            "cpu": 300, "total_cpu": 400, "elsewhere_cpu": 100,
            "memory": 6144, "total_memory": 8192.0,
            "disk": 10240,
        });
        let (limit, used) = crate::allowance::with_pool(
            Resources::from_plan(&plan),
            Resources::new(200, 4096, 1024),
            Resources::elsewhere_in_plan(&plan).unwrap_or_default(),
        );
        let usage = store_usage(limit, used);

        assert_eq!((usage[0].used, usage[0].limit), (300, 400));
        // without elsewhere_*: the pool less this panel's share
        assert_eq!((usage[1].used, usage[1].limit), (6144, 8192));
        // no pool: as it is
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
                full_price: memory_price,
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

    #[test]
    fn membership_offers_follow_the_currencies_and_the_sale() {
        let mut economy = crate::settings::economy::AppSettingsEconomy {
            plus_tokens: 15,
            plusplus_tokens: 30,
            plusplus_credits: 0,
            ..Default::default()
        };

        let plus = membership_offer(&economy, MembershipKind::Plus, Pay::Tokens, 0).unwrap();
        assert_eq!((plus.cost, plus.full_cost, plus.days), (15, 15, 30));
        // the first is never sold for credits, the second only with a credit price
        assert!(membership_offer(&economy, MembershipKind::Plus, Pay::Credits, 0).is_none());
        assert!(membership_offer(&economy, MembershipKind::Plusplus, Pay::Credits, 0).is_none());

        // 25 % off, rounded half up like the store: 15 → 11, 30 → 23
        let plus = membership_offer(&economy, MembershipKind::Plus, Pay::Tokens, 25).unwrap();
        assert_eq!((plus.cost, plus.full_cost), (11, 15));
        let plusplus =
            membership_offer(&economy, MembershipKind::Plusplus, Pay::Tokens, 25).unwrap();
        assert_eq!((plusplus.cost, plusplus.full_cost), (23, 30));

        economy.plusplus_credits = 400;
        let credits =
            membership_offer(&economy, MembershipKind::Plusplus, Pay::Credits, 50).unwrap();
        assert_eq!(credits.cost, 200);
        assert_eq!(renewal_pay(&economy, MembershipKind::Plusplus), Pay::Tokens);

        // tokens off: the second renews with credits, the first is not sold
        economy.tokens_enabled = false;
        assert!(membership_offer(&economy, MembershipKind::Plus, Pay::Tokens, 0).is_none());
        assert_eq!(
            renewal_pay(&economy, MembershipKind::Plusplus),
            Pay::Credits
        );
    }

    #[test]
    fn the_goal_is_the_cheapest_membership_not_had() {
        let economy = crate::settings::economy::AppSettingsEconomy {
            plus_tokens: 15,
            plusplus_tokens: 30,
            ..Default::default()
        };
        let now = at(0).and_utc();
        let goal = |membership: &Membership| {
            membership_goal(&economy, membership, now).map(|goal| (goal.membership, goal.cost))
        };

        assert_eq!(
            goal(&Membership::default()),
            Some((MembershipKind::Plus, 15))
        );
        let plus = Membership {
            plus_until: Some(at(24)),
            ..Default::default()
        };
        assert_eq!(goal(&plus), Some((MembershipKind::Plusplus, 30)));
        // the second gives everything the first does
        let plusplus = Membership {
            plusplus_until: Some(at(24)),
            ..Default::default()
        };
        assert_eq!(goal(&plusplus), None);
        // a lapsed first counts as not had
        let lapsed = Membership {
            plus_until: Some(at(-1)),
            ..Default::default()
        };
        assert_eq!(goal(&lapsed), Some((MembershipKind::Plus, 15)));

        let off = crate::settings::economy::AppSettingsEconomy {
            membership_enabled: false,
            ..economy.clone()
        };
        assert_eq!(membership_goal(&off, &Membership::default(), now), None);
    }
}
