//! plugged.host: how much a player may use for their own servers (docs/features/create-server.md).
//!
//! The limit is the account's noprofit.space plan when noprofit.space is connected, else the games
//! settings' allowance (the same for every account). What is used is the sum of the servers the
//! account owns. A limit of 0 puts no limit on that resource, like a node's memory and disk.

use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// CPU in percent of one thread, memory and disk in MiB.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resources {
    pub cpu: i64,
    pub memory: i64,
    pub disk: i64,
}

impl Resources {
    pub const fn new(cpu: i64, memory: i64, disk: i64) -> Self {
        Self { cpu, memory, disk }
    }

    /// `{cpu, memory, disk}` out of a noprofit.space plan; a missing or broken value is 0.
    pub fn from_plan(plan: &serde_json::Value) -> Self {
        let value = |key: &str| {
            plan.get(key)
                .and_then(|value| {
                    value
                        .as_i64()
                        .or_else(|| value.as_f64().map(|value| value as i64))
                })
                .unwrap_or(0)
                .max(0)
        };

        Self::new(value("cpu"), value("memory"), value("disk"))
    }
}

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllowanceSource {
    /// The account's noprofit.space plan.
    Plan,
    /// The games settings.
    Settings,
}

/// Why a server of some size cannot be created or resized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No account plan (noprofit.space connected, the account has none).
    NoPlan,
    TooManyServers,
    Cpu,
    Memory,
    Disk,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Self::NoPlan => "your account has no plan to create servers with",
            Self::TooManyServers => "you have as many servers as your account allows",
            Self::Cpu => "not enough cpu left in your account",
            Self::Memory => "not enough memory left in your account",
            Self::Disk => "not enough disk left in your account",
        }
    }
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct Allowance {
    pub source: AllowanceSource,
    /// False when noprofit.space is connected and the account has no plan.
    pub has_plan: bool,
    pub limit: Resources,
    pub used: Resources,
    /// Servers the account owns, and how many it may have (0: no limit).
    pub servers: i64,
    pub max_servers: i64,
}

impl Allowance {
    /// What is left of each limited resource; `None` where there is no limit.
    pub fn remaining(&self) -> (Option<i64>, Option<i64>, Option<i64>) {
        let left = |limit: i64, used: i64| (limit > 0).then(|| (limit - used).max(0));

        (
            left(self.limit.cpu, self.used.cpu),
            left(self.limit.memory, self.used.memory),
            left(self.limit.disk, self.used.disk),
        )
    }

    /// Whether one more server of `size` fits.
    pub fn check_new(&self, size: Resources) -> Result<(), Refusal> {
        if !self.has_plan {
            return Err(Refusal::NoPlan);
        }
        if self.max_servers > 0 && self.servers >= self.max_servers {
            return Err(Refusal::TooManyServers);
        }

        self.check_change(Resources::default(), size)
    }

    /// Whether a server of size `from` may become `to`. Shrinking always fits, so an account over
    /// its limit (its plan got smaller, or it has none any more) can still make room.
    pub fn check_change(&self, from: Resources, to: Resources) -> Result<(), Refusal> {
        let grows = to.cpu > from.cpu || to.memory > from.memory || to.disk > from.disk;
        if !self.has_plan && (grows || from == Resources::default()) {
            return Err(Refusal::NoPlan);
        }

        let fits = |limit: i64, used: i64, from: i64, to: i64| {
            limit <= 0 || to <= from || used - from + to <= limit
        };

        if !fits(self.limit.cpu, self.used.cpu, from.cpu, to.cpu) {
            return Err(Refusal::Cpu);
        }
        if !fits(self.limit.memory, self.used.memory, from.memory, to.memory) {
            return Err(Refusal::Memory);
        }
        if !fits(self.limit.disk, self.used.disk, from.disk, to.disk) {
            return Err(Refusal::Disk);
        }

        Ok(())
    }
}

/// What the account's servers use, and how many it has.
pub async fn usage(
    database: &crate::database::Database,
    owner_uuid: uuid::Uuid,
) -> Result<(Resources, i64), sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS servers,
            COALESCE(SUM(cpu), 0)::BIGINT AS cpu,
            COALESCE(SUM(memory), 0)::BIGINT AS memory,
            COALESCE(SUM(disk), 0)::BIGINT AS disk
        FROM servers
        WHERE owner_uuid = $1
        "#,
    )
    .bind(owner_uuid)
    .fetch_one(database.write())
    .await?;

    Ok((
        Resources::new(
            row.try_get("cpu")?,
            row.try_get("memory")?,
            row.try_get("disk")?,
        ),
        row.try_get("servers")?,
    ))
}

/// The account's allowance now. Reads from the primary, so a create right before is counted.
pub async fn for_user(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<Allowance, anyhow::Error> {
    let (connected, settings_limit, max_servers) = {
        let settings = state.settings.get().await?;

        (
            settings.noprofit.connected(),
            Resources::new(
                settings.games.allowance_cpu as i64,
                settings.games.allowance_memory,
                settings.games.allowance_disk,
            ),
            settings.games.max_servers as i64,
        )
    };

    let (source, has_plan, limit) = if connected {
        match crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
            .await?
            .and_then(|link| link.plan)
        {
            Some(plan) => (AllowanceSource::Plan, true, Resources::from_plan(&plan)),
            None => (AllowanceSource::Plan, false, Resources::default()),
        }
    } else {
        (AllowanceSource::Settings, true, settings_limit)
    };

    let (used, servers) = usage(&state.database, user_uuid).await?;

    Ok(Allowance {
        source,
        has_plan,
        limit,
        used,
        servers,
        max_servers,
    })
}

/// Holds the account's create/resize lock: the allowance is read, checked, then spent, and two
/// requests at once would both pass the same check. `None` when another one did not finish in time.
pub async fn lock(state: &crate::State, user_uuid: uuid::Uuid) -> Option<crate::cache::CacheLock> {
    let key = compact_str::format_compact!("allowance::{user_uuid}");

    state.cache.lock(key, Some(120), Some(15_000)).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowance(limit: Resources, used: Resources) -> Allowance {
        Allowance {
            source: AllowanceSource::Settings,
            has_plan: true,
            limit,
            used,
            servers: 1,
            max_servers: 0,
        }
    }

    #[test]
    fn a_plan_is_read_leniently() {
        let plan = serde_json::json!({"cpu": 400, "memory": 12288.0, "disk": "x", "max_cpu": 9});
        assert_eq!(Resources::from_plan(&plan), Resources::new(400, 12288, 0));
        assert_eq!(
            Resources::from_plan(&serde_json::json!({"cpu": -5})),
            Resources::default()
        );
    }

    #[test]
    fn new_servers_fit_exactly_up_to_the_limit() {
        let allowance = allowance(
            Resources::new(400, 8192, 20480),
            Resources::new(200, 4096, 10240),
        );

        assert_eq!(
            allowance.check_new(Resources::new(200, 4096, 10240)),
            Ok(())
        );
        assert_eq!(
            allowance.check_new(Resources::new(201, 4096, 10240)),
            Err(Refusal::Cpu)
        );
        assert_eq!(
            allowance.check_new(Resources::new(100, 5000, 100)),
            Err(Refusal::Memory)
        );
        assert_eq!(
            allowance.check_new(Resources::new(100, 100, 20000)),
            Err(Refusal::Disk)
        );
        assert_eq!(allowance.remaining(), (Some(200), Some(4096), Some(10240)));
    }

    #[test]
    fn zero_is_no_limit_and_counts_are_enforced() {
        let mut allowance = allowance(Resources::new(0, 1024, 0), Resources::new(900, 0, 99999));
        assert_eq!(
            allowance.check_new(Resources::new(500, 1024, 50000)),
            Ok(())
        );
        assert_eq!(allowance.remaining(), (None, Some(1024), None));

        allowance.max_servers = 1;
        assert_eq!(
            allowance.check_new(Resources::new(1, 1, 1)),
            Err(Refusal::TooManyServers)
        );

        allowance.has_plan = false;
        assert_eq!(
            allowance.check_new(Resources::new(1, 1, 1)),
            Err(Refusal::NoPlan)
        );
        // without a plan a server can still shrink, never grow
        let from = Resources::new(200, 2048, 4096);
        assert_eq!(
            allowance.check_change(from, Resources::new(100, 2048, 4096)),
            Ok(())
        );
        assert_eq!(
            allowance.check_change(from, Resources::new(100, 4096, 1024)),
            Err(Refusal::NoPlan)
        );
    }

    #[test]
    fn shrinking_always_fits_and_growing_counts_the_old_size() {
        // over the limit after the plan got smaller
        let allowance = allowance(
            Resources::new(200, 4096, 10240),
            Resources::new(300, 8192, 20480),
        );
        let from = Resources::new(300, 8192, 20480);

        assert_eq!(
            allowance.check_change(from, Resources::new(100, 2048, 10240)),
            Ok(())
        );
        assert_eq!(
            allowance.check_change(from, Resources::new(300, 8192, 20480)),
            Ok(())
        );
        assert_eq!(
            allowance.check_change(from, Resources::new(301, 1024, 1024)),
            Err(Refusal::Cpu)
        );

        let roomy = self::allowance(
            Resources::new(400, 8192, 20480),
            Resources::new(200, 4096, 10240),
        );
        assert_eq!(
            roomy.check_change(
                Resources::new(200, 4096, 10240),
                Resources::new(400, 8192, 20480)
            ),
            Ok(())
        );
    }
}
