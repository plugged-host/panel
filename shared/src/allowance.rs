//! plugged.host: how much a player may use for their own servers (docs/features/create-server.md).
//!
//! The limit is the account's noprofit.space plan when noprofit.space is connected, else the games
//! settings' allowance (the same for every account) plus what the account bought and the Discord
//! tier (docs/features/discord.md). What is used is the sum of the servers the account owns. A
//! limit of 0 puts no limit on that resource, like a node's memory and disk.

use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

/// CPU in percent of one thread, memory and disk in MiB. Read leniently: a missing one is 0.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(default)]
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

    /// What the account's other noprofit.space services (switched.cloud, the Pelican panel) use of
    /// the pool a plan shares with them: its `elsewhere_*`, or the pool (`total_*`) less this
    /// panel's share. `None` for a plan of this panel's alone.
    pub fn elsewhere_in_plan(plan: &serde_json::Value) -> Option<Self> {
        let number = |key: String| {
            plan.get(&key).and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_f64().map(|value| value as i64))
            })
        };
        let share = Self::from_plan(plan);
        let one = |resource: &str, share: i64| {
            number(format!("elsewhere_{resource}"))
                .or_else(|| number(format!("total_{resource}")).map(|total| total - share))
                .map(|elsewhere| elsewhere.max(0))
        };

        let (cpu, memory, disk) = (
            one("cpu", share.cpu),
            one("memory", share.memory),
            one("disk", share.disk),
        );

        (cpu.is_some() || memory.is_some() || disk.is_some())
            .then(|| Self::new(cpu.unwrap_or(0), memory.unwrap_or(0), disk.unwrap_or(0)))
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

/// With a pool the account shares with other noprofit.space services, `limit` and `used` are the
/// whole pool's, as the Pelican panel shows them: what the other services use (`elsewhere`) counts
/// as used and is part of the limit, so what is left is still this panel's share less its servers.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct Allowance {
    pub source: AllowanceSource,
    /// False when noprofit.space is connected and the account has no plan.
    pub has_plan: bool,
    pub limit: Resources,
    pub used: Resources,
    /// What the account's other noprofit.space services use of the pool (in `limit` and `used`).
    pub elsewhere: Resources,
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

/// `limit` grown by `extra`, where it limits anything (0 stays no limit).
pub fn add_where_limited(limit: Resources, extra: Resources) -> Resources {
    let add = |limit: i64, extra: i64| {
        if limit > 0 {
            limit + extra.max(0)
        } else {
            limit
        }
    };

    Resources::new(
        add(limit.cpu, extra.cpu),
        add(limit.memory, extra.memory),
        add(limit.disk, extra.disk),
    )
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
            settings.games.allowance(),
            settings.games.max_servers as i64,
        )
    };

    let mut elsewhere = Resources::default();
    let (source, has_plan, limit) = if connected {
        match crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
            .await?
            .and_then(|link| link.plan)
        {
            Some(plan) => {
                elsewhere = Resources::elsewhere_in_plan(&plan).unwrap_or_default();
                (AllowanceSource::Plan, true, Resources::from_plan(&plan))
            }
            None => (AllowanceSource::Plan, false, Resources::default()),
        }
    } else {
        // What the Resource Store sold the account (with noprofit.space it is in the plan).
        let extras = crate::economy::extras(&state.database, user_uuid).await?;
        // The Discord tier while the account's Discord is in the server (docs/features/discord.md).
        let discord = crate::discord::tier(state, user_uuid).await?;
        (
            AllowanceSource::Settings,
            true,
            add_where_limited(add_where_limited(settings_limit, extras), discord),
        )
    };

    let (used, servers) = usage(&state.database, user_uuid).await?;
    let (limit, used) = with_pool(limit, used, elsewhere);

    Ok(Allowance {
        source,
        has_plan,
        limit,
        used,
        elsewhere,
        servers,
        max_servers,
    })
}

/// `limit` and `used` grown by what other services use of the pool: every difference between
/// them (what is left, what is over) stays this panel's. A resource nothing else uses is as it was.
pub fn with_pool(
    limit: Resources,
    used: Resources,
    elsewhere: Resources,
) -> (Resources, Resources) {
    let one = |limit: i64, used: i64, elsewhere: i64| {
        if elsewhere > 0 {
            (limit.max(0) + elsewhere, used + elsewhere)
        } else {
            (limit, used)
        }
    };

    let (cpu, memory, disk) = (
        one(limit.cpu, used.cpu, elsewhere.cpu),
        one(limit.memory, used.memory, elsewhere.memory),
        one(limit.disk, used.disk, elsewhere.disk),
    );

    (
        Resources::new(cpu.0, memory.0, disk.0),
        Resources::new(cpu.1, memory.1, disk.1),
    )
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

    #[test]
    fn extras_only_grow_limited_resources() {
        assert_eq!(
            add_where_limited(Resources::new(100, 0, 2048), Resources::new(50, 1024, 1024)),
            Resources::new(150, 0, 3072)
        );
        assert_eq!(
            add_where_limited(Resources::new(100, 1, 1), Resources::new(-5, 0, 0)),
            Resources::new(100, 1, 1)
        );
    }

    fn allowance(limit: Resources, used: Resources) -> Allowance {
        Allowance {
            source: AllowanceSource::Settings,
            has_plan: true,
            limit,
            used,
            elsewhere: Resources::default(),
            servers: 1,
            max_servers: 0,
        }
    }

    #[test]
    fn a_shared_pool_shows_whole_and_leaves_this_panels_share() {
        // noprofit.space's planFor(): this panel's share, the whole pool and what others use of it
        let plan = serde_json::json!({
            "cpu": 300, "total_cpu": 400, "elsewhere_cpu": 100,
            "memory": 6144, "total_memory": 8192.0,
            "disk": 10240,
        });
        let elsewhere = Resources::elsewhere_in_plan(&plan).unwrap();
        assert_eq!(elsewhere, Resources::new(100, 2048, 0));
        assert_eq!(
            Resources::elsewhere_in_plan(&serde_json::json!({"cpu": 300})),
            None
        );

        let (limit, used) = with_pool(
            Resources::from_plan(&plan),
            Resources::new(200, 4096, 1024),
            elsewhere,
        );
        assert_eq!(limit, Resources::new(400, 8192, 10240));
        assert_eq!(used, Resources::new(300, 6144, 1024));

        // what is left is still the share less this panel's servers
        let mut pool = allowance(limit, used);
        pool.elsewhere = elsewhere;
        assert_eq!(pool.remaining(), (Some(100), Some(2048), Some(9216)));

        // everything used elsewhere: nothing left here (a share of 0 is no "no limit")
        let (limit, used) = with_pool(
            Resources::new(0, 0, 0),
            Resources::default(),
            Resources::new(400, 0, 0),
        );
        assert_eq!(allowance(limit, used).remaining().0, Some(0));
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
