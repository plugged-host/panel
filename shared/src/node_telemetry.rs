//! plugged.host: node telemetry, the panel's own view of its nodes, sampled every minute.
//!
//! The `sample_nodes` task asks every node's Wings for its system stats, its servers' states and
//! (on the Wings fork) its load, and records whether it answered, how fast, and how loaded it is:
//! the newest sample per node in `node_telemetry`, every sample for a week in
//! `node_telemetry_samples`. When Wings does not answer, a TCP connect tells a refused port (the
//! machine is up, Wings is not listening: stopped, crashed or still booting) from a machine that is
//! gone, so a failover only acts on the latter (`consecutive_outages`).
//!
//! One sampler for everything that needs it: the node's Statistics tab, the status page, placement
//! of new servers and the deployment pause (`pressure`), and node failover.

use serde::Serialize;
use sqlx::Row;
use std::{collections::HashMap, time::Duration};
use utoipa::ToSchema;

/// Days samples are kept.
pub const RETENTION_DAYS: i32 = 7;
/// Minutes `pressure` is averaged over.
pub const PRESSURE_MINUTES: i32 = 5;
/// A node's `pressure` for placement, in a query that joins `node_telemetry` as `telemetry`:
/// 100 while it does not answer, unknown (NULL) without a sample from the last 5 minutes.
pub const PRESSURE_SQL: &str = "(CASE WHEN telemetry.node_uuid IS NULL \
    OR telemetry.sampled < NOW() - INTERVAL '5 minutes' THEN NULL \
    WHEN NOT telemetry.reachable THEN 100 ELSE telemetry.pressure END)::REAL";

/// Nodes probed at once.
const CONCURRENCY: usize = 16;
/// How long a node has to answer.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the refused-port check waits.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// One probe of one node. The load figures are `None` when Wings did not answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    pub reachable: bool,
    pub refused: bool,
    pub response_time_ms: Option<i32>,
    /// All threads together, 0 to 100.
    pub cpu_percent: Option<f32>,
    pub cpu_threads: Option<i32>,
    pub memory_used: Option<i64>,
    pub memory_total: Option<i64>,
    pub disk_used: Option<i64>,
    pub disk_total: Option<i64>,
    /// The 1-minute load average; only from the Wings fork (`GET /api/system/load`).
    pub load1: Option<f32>,
    /// Servers not offline.
    pub servers_running: Option<i32>,
}

impl Sample {
    /// A sample from what Wings answered.
    pub fn answered(
        response_time: Duration,
        stats: &wings_api::SystemStats,
        servers_running: Option<i32>,
        load1: Option<f32>,
        data_disk: Option<&wings_api::fork::NodeDiskSpace>,
    ) -> Self {
        Self {
            reachable: true,
            refused: false,
            response_time_ms: Some(response_time.as_millis().min(i32::MAX as u128) as i32),
            cpu_percent: Some(stats.cpu.used.clamp(0.0, 100.0) as f32),
            cpu_threads: Some(stats.cpu.threads.min(i32::MAX as u64) as i32),
            memory_used: Some(clamp_i64(stats.memory.used)),
            memory_total: Some(clamp_i64(stats.memory.total)),
            // the fork's figure is the disk holding the servers' files; Wings' own is always `/`
            disk_used: Some(clamp_i64(
                data_disk.map_or(stats.disk.used, |disk| disk.used),
            )),
            disk_total: Some(clamp_i64(
                data_disk.map_or(stats.disk.total, |disk| disk.total),
            )),
            load1,
            servers_running,
        }
    }

    /// A sample of a node that did not answer.
    pub fn unreachable(refused: bool) -> Self {
        Self {
            refused,
            ..Default::default()
        }
    }
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// The newest sample of a node and what follows from the ones before it.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiNodeTelemetry {
    pub reachable: bool,
    pub refused: bool,
    pub response_time_ms: Option<i32>,
    pub cpu_percent: Option<f32>,
    pub cpu_threads: Option<i32>,
    pub memory_used: Option<i64>,
    pub memory_total: Option<i64>,
    pub disk_used: Option<i64>,
    pub disk_total: Option<i64>,
    pub load1: Option<f32>,
    pub servers_running: Option<i32>,
    /// The higher of CPU and memory use in percent, averaged over the last [`PRESSURE_MINUTES`]
    /// minutes of answered samples.
    pub pressure: Option<f32>,
    /// Samples in a row in which Wings did not answer.
    pub consecutive_failures: i32,
    /// Samples in a row in which the machine itself was gone (not even a refused port).
    pub consecutive_outages: i32,
    pub error: Option<String>,
    pub last_reachable: Option<chrono::DateTime<chrono::Utc>>,
    pub sampled: chrono::DateTime<chrono::Utc>,
}

/// One point of a node's history: the samples of one bucket, averaged.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiNodeTelemetryPoint {
    pub sampled: chrono::DateTime<chrono::Utc>,
    /// Share of the bucket's samples in which Wings answered, 0 to 1.
    pub reachable: f32,
    pub response_time_ms: Option<f32>,
    pub cpu_percent: Option<f32>,
    pub memory_used: Option<i64>,
    pub memory_total: Option<i64>,
    pub disk_used: Option<i64>,
    pub disk_total: Option<i64>,
    pub load1: Option<f32>,
    pub servers_running: Option<f32>,
}

const LATEST_COLUMNS: &str = "node_uuid, reachable, refused, response_time_ms, cpu_percent, \
    cpu_threads, memory_used, memory_total, disk_used, disk_total, load1, servers_running, \
    pressure, consecutive_failures, consecutive_outages, error, last_reachable, sampled";

fn map_latest(row: &sqlx::postgres::PgRow) -> Result<AdminApiNodeTelemetry, sqlx::Error> {
    Ok(AdminApiNodeTelemetry {
        reachable: row.try_get("reachable")?,
        refused: row.try_get("refused")?,
        response_time_ms: row.try_get("response_time_ms")?,
        cpu_percent: row.try_get("cpu_percent")?,
        cpu_threads: row.try_get("cpu_threads")?,
        memory_used: row.try_get("memory_used")?,
        memory_total: row.try_get("memory_total")?,
        disk_used: row.try_get("disk_used")?,
        disk_total: row.try_get("disk_total")?,
        load1: row.try_get("load1")?,
        servers_running: row.try_get("servers_running")?,
        pressure: row.try_get("pressure")?,
        consecutive_failures: row.try_get("consecutive_failures")?,
        consecutive_outages: row.try_get("consecutive_outages")?,
        error: row.try_get("error")?,
        last_reachable: row
            .try_get::<Option<chrono::NaiveDateTime>, _>("last_reachable")?
            .map(|at| at.and_utc()),
        sampled: row
            .try_get::<chrono::NaiveDateTime, _>("sampled")?
            .and_utc(),
    })
}

/// The newest sample of a node, if it was ever sampled.
pub async fn latest(
    database: &crate::database::Database,
    node_uuid: uuid::Uuid,
) -> Result<Option<AdminApiNodeTelemetry>, anyhow::Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {LATEST_COLUMNS} FROM node_telemetry WHERE node_uuid = $1"
    )))
    .bind(node_uuid)
    .fetch_optional(database.read())
    .await?;

    Ok(row.as_ref().map(map_latest).transpose()?)
}

/// The newest sample of every node that was sampled.
pub async fn latest_all(
    database: &crate::database::Database,
) -> Result<HashMap<uuid::Uuid, AdminApiNodeTelemetry>, anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {LATEST_COLUMNS} FROM node_telemetry"
    )))
    .fetch_all(database.read())
    .await?;

    let mut latest = HashMap::with_capacity(rows.len());
    for row in &rows {
        latest.insert(row.try_get("node_uuid")?, map_latest(row)?);
    }

    Ok(latest)
}

/// Minutes per point so that `hours` of history make at most 360 points.
pub fn bucket_minutes(hours: i32) -> i32 {
    ((hours.max(1) * 60 + 359) / 360).max(1)
}

/// A node's history over the last `hours`, oldest first, in buckets of [`bucket_minutes`].
pub async fn history(
    database: &crate::database::Database,
    node_uuid: uuid::Uuid,
    hours: i32,
) -> Result<Vec<AdminApiNodeTelemetryPoint>, anyhow::Error> {
    let hours = hours.clamp(1, RETENTION_DAYS * 24);

    let rows = sqlx::query(
        r#"
        SELECT
            date_bin(make_interval(mins => $3), sampled, TIMESTAMP '2000-01-01') AS bucket,
            AVG(CASE WHEN reachable THEN 1.0 ELSE 0.0 END)::REAL AS reachable,
            AVG(response_time_ms)::REAL AS response_time_ms,
            AVG(cpu_percent)::REAL AS cpu_percent,
            AVG(memory_used)::BIGINT AS memory_used,
            MAX(memory_total) AS memory_total,
            AVG(disk_used)::BIGINT AS disk_used,
            MAX(disk_total) AS disk_total,
            AVG(load1)::REAL AS load1,
            AVG(servers_running)::REAL AS servers_running
        FROM node_telemetry_samples
        WHERE node_uuid = $1 AND sampled > NOW() - make_interval(hours => $2)
        GROUP BY bucket
        ORDER BY bucket
        "#,
    )
    .bind(node_uuid)
    .bind(hours)
    .bind(bucket_minutes(hours))
    .fetch_all(database.read())
    .await?;

    rows.iter()
        .map(|row| {
            Ok(AdminApiNodeTelemetryPoint {
                sampled: row.try_get::<chrono::NaiveDateTime, _>("bucket")?.and_utc(),
                reachable: row.try_get("reachable")?,
                response_time_ms: row.try_get("response_time_ms")?,
                cpu_percent: row.try_get("cpu_percent")?,
                memory_used: row.try_get("memory_used")?,
                memory_total: row.try_get("memory_total")?,
                disk_used: row.try_get("disk_used")?,
                disk_total: row.try_get("disk_total")?,
                load1: row.try_get("load1")?,
                servers_running: row.try_get("servers_running")?,
            })
        })
        .collect()
}

/// Records a node's sample: one row in the history, and the node's newest sample with its counters.
pub async fn record(
    database: &crate::database::Database,
    node_uuid: uuid::Uuid,
    sample: &Sample,
    error: Option<&str>,
) -> Result<(), anyhow::Error> {
    let mut transaction = database.write().begin().await?;

    sqlx::query(
        r#"
        INSERT INTO node_telemetry_samples (node_uuid, reachable, refused, response_time_ms,
            cpu_percent, memory_used, memory_total, disk_used, disk_total, load1, servers_running,
            sampled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, date_trunc('second', NOW()))
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(node_uuid)
    .bind(sample.reachable)
    .bind(sample.refused)
    .bind(sample.response_time_ms)
    .bind(sample.cpu_percent)
    .bind(sample.memory_used)
    .bind(sample.memory_total)
    .bind(sample.disk_used)
    .bind(sample.disk_total)
    .bind(sample.load1)
    .bind(sample.servers_running)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO node_telemetry (node_uuid, reachable, refused, response_time_ms, cpu_percent,
            cpu_threads, memory_used, memory_total, disk_used, disk_total, load1, servers_running,
            pressure, consecutive_failures, consecutive_outages, error, last_reachable, sampled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
            (
                SELECT AVG(GREATEST(
                    COALESCE(cpu_percent, 0),
                    COALESCE(memory_used::REAL * 100 / NULLIF(memory_total, 0), 0)
                ))::REAL
                FROM node_telemetry_samples
                WHERE node_uuid = $1 AND reachable
                    AND sampled > NOW() - make_interval(mins => $14)
            ),
            CASE WHEN $2 THEN 0 ELSE 1 END,
            CASE WHEN $2 OR $3 THEN 0 ELSE 1 END,
            $13,
            CASE WHEN $2 THEN NOW() END,
            NOW())
        ON CONFLICT (node_uuid) DO UPDATE SET
            reachable = EXCLUDED.reachable,
            refused = EXCLUDED.refused,
            response_time_ms = EXCLUDED.response_time_ms,
            cpu_percent = EXCLUDED.cpu_percent,
            cpu_threads = COALESCE(EXCLUDED.cpu_threads, node_telemetry.cpu_threads),
            memory_used = EXCLUDED.memory_used,
            memory_total = COALESCE(EXCLUDED.memory_total, node_telemetry.memory_total),
            disk_used = EXCLUDED.disk_used,
            disk_total = COALESCE(EXCLUDED.disk_total, node_telemetry.disk_total),
            load1 = EXCLUDED.load1,
            servers_running = EXCLUDED.servers_running,
            pressure = EXCLUDED.pressure,
            consecutive_failures = CASE WHEN EXCLUDED.reachable THEN 0
                ELSE node_telemetry.consecutive_failures + 1 END,
            consecutive_outages = CASE WHEN EXCLUDED.reachable OR EXCLUDED.refused THEN 0
                ELSE node_telemetry.consecutive_outages + 1 END,
            error = EXCLUDED.error,
            last_reachable = COALESCE(EXCLUDED.last_reachable, node_telemetry.last_reachable),
            sampled = EXCLUDED.sampled
        "#,
    )
    .bind(node_uuid)
    .bind(sample.reachable)
    .bind(sample.refused)
    .bind(sample.response_time_ms)
    .bind(sample.cpu_percent)
    .bind(sample.cpu_threads)
    .bind(sample.memory_used)
    .bind(sample.memory_total)
    .bind(sample.disk_used)
    .bind(sample.disk_total)
    .bind(sample.load1)
    .bind(sample.servers_running)
    .bind(error)
    .bind(PRESSURE_MINUTES)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

/// Deletes samples older than [`RETENTION_DAYS`].
pub async fn prune(database: &crate::database::Database) -> Result<u64, anyhow::Error> {
    let result = sqlx::query(
        "DELETE FROM node_telemetry_samples WHERE sampled < NOW() - make_interval(days => $1)",
    )
    .bind(RETENTION_DAYS)
    .execute(database.write())
    .await?;

    Ok(result.rows_affected())
}

/// Probes every node and records the samples (the `sample_nodes` task).
pub async fn sample_all(state: &crate::State) -> Result<(), anyhow::Error> {
    use futures_util::StreamExt;

    let mut nodes = Vec::new();
    let mut page = 1;
    loop {
        let result =
            crate::models::node::Node::all_with_pagination(&state.database, page, 100, None)
                .await?;
        if result.data.is_empty() {
            break;
        }
        nodes.extend(result.data);
        page += 1;
    }

    let mut futures = Vec::with_capacity(nodes.len());
    for node in &nodes {
        futures.push(sample_one(&state.database, node));
    }

    let mut probes = futures_util::stream::iter(futures).buffer_unordered(CONCURRENCY);
    while probes.next().await.is_some() {}

    Ok(())
}

async fn sample_one(database: &crate::database::Database, node: &crate::models::node::Node) {
    let (sample, error) = probe(database, node).await;

    if let Err(err) = record(database, node.uuid, &sample, error.as_deref()).await {
        tracing::warn!(node = %node.uuid, "failed to record node telemetry: {err:#}");
    }
}

/// Asks one node; never fails, an error makes an unreachable sample.
pub async fn probe(
    database: &crate::database::Database,
    node: &crate::models::node::Node,
) -> (Sample, Option<String>) {
    let started = std::time::Instant::now();

    let answered = tokio::time::timeout(PROBE_TIMEOUT, async {
        let client = node.api_client(database).await?;
        let stats = client.get_system_stats().await?;
        let response_time = started.elapsed();

        let servers_running = node
            .fetch_server_resources(database)
            .await
            .ok()
            .map(|servers| running(servers.values().map(|usage| usage.state)));
        // A stock wings-rs node has no load endpoint (404).
        let load1 = client
            .get_system_load()
            .await
            .ok()
            .map(|load| load.load[0] as f32);
        // likewise only on the fork: the disk of the servers' files (a storage box keeps them apart from `/`)
        let data_disk = client
            .get_public_resources()
            .await
            .ok()
            .map(|resources| resources.disk);

        Ok::<_, anyhow::Error>(Sample::answered(
            response_time,
            &stats.stats,
            servers_running,
            load1,
            data_disk.as_ref(),
        ))
    })
    .await;

    match answered {
        Ok(Ok(sample)) => (sample, None),
        Ok(Err(err)) => (
            Sample::unreachable(refused(&node.url).await),
            Some(format!("{err:#}")),
        ),
        Err(_) => (
            Sample::unreachable(refused(&node.url).await),
            Some(format!("no answer within {}s", PROBE_TIMEOUT.as_secs())),
        ),
    }
}

/// How many servers are not offline.
fn running(states: impl Iterator<Item = wings_api::ServerState>) -> i32 {
    states
        .filter(|state| !matches!(state, wings_api::ServerState::Offline))
        .count()
        .min(i32::MAX as usize) as i32
}

/// Whether the node's host actively refused a TCP connection to Wings' port: the machine answered
/// and nothing listens. A connection that opens (Wings frozen but accepting), a timeout or an
/// unreachable host says nothing, so none of them count.
async fn refused(url: &reqwest::Url) -> bool {
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return false;
    };

    matches!(
        tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect((host, port))).await,
        Ok(Err(err)) if err.kind() == std::io::ErrorKind::ConnectionRefused
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_has_at_most_360_points() {
        assert_eq!(bucket_minutes(1), 1);
        assert_eq!(bucket_minutes(6), 1);
        assert_eq!(bucket_minutes(24), 4);
        assert_eq!(bucket_minutes(168), 28);
        assert_eq!(bucket_minutes(0), 1);
        for hours in 1..=168 {
            assert!(hours * 60 / bucket_minutes(hours) <= 360);
        }
    }

    #[test]
    fn running_counts_everything_not_offline() {
        use wings_api::ServerState::*;

        assert_eq!(
            running([Offline, Running, Starting, Stopping].into_iter()),
            3
        );
        assert_eq!(running(std::iter::empty()), 0);
    }

    #[test]
    fn an_unreachable_sample_has_no_figures() {
        let sample = Sample::unreachable(true);

        assert!(!sample.reachable);
        assert!(sample.refused);
        assert_eq!(sample.cpu_percent, None);
        assert_eq!(sample.response_time_ms, None);
    }

    #[test]
    fn huge_values_are_clamped() {
        assert_eq!(clamp_i64(u64::MAX), i64::MAX);
        assert_eq!(clamp_i64(42), 42);
    }

    #[tokio::test]
    async fn a_closed_local_port_is_refused() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("address").port();
        drop(listener);

        let url = reqwest::Url::parse(&format!("http://127.0.0.1:{port}")).expect("url");
        assert!(refused(&url).await);
    }
}
