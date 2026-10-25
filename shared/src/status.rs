//! plugged.host: the public status page (docs/features/status.md).
//!
//! Every minute `collect_status` probes the website and the panel (when their health URLs are
//! set) and reads every node's state from node telemetry, turns that into a public status per
//! component and counts it into the day's uptime. The panel serves the result as
//! `/data/{current,history,incidents,targets}.json` and `/data/feed.xml`, in the shapes the
//! status page and its Cloudflare Worker read.

use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::{collections::HashMap, time::Duration};
use utoipa::ToSchema;

/// Component statuses, worst last.
pub const STATUSES: &[&str] = &[
    "unknown",
    "operational",
    "maintenance",
    "degraded",
    "partial_outage",
    "major_outage",
    "offline",
];
pub const INCIDENT_IMPACTS: &[&str] = &["minor", "major", "critical", "maintenance"];
pub const INCIDENT_STATUSES: &[&str] = &["investigating", "identified", "monitoring", "resolved"];

/// Probes of the website and the panel.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// A node whose newest telemetry is older than this counts as unreachable.
const TELEMETRY_STALE_SECONDS: i64 = 150;
/// Days the public uptime covers.
const UPTIME_DAYS: i64 = 90;

/// How bad a status is: the worse one wins when two meet.
pub fn priority(status: &str) -> i32 {
    match status {
        "major_outage" | "offline" => 5,
        "partial_outage" => 4,
        "degraded" => 3,
        "maintenance" => 2,
        "operational" => 1,
        _ => 0,
    }
}

pub fn label(status: &str) -> &'static str {
    match status {
        "operational" => "All systems operational",
        "degraded" => "Degraded performance",
        "maintenance" => "Maintenance in progress",
        "partial_outage" => "Partial system outage",
        "major_outage" => "Major system outage",
        _ => "Status unknown",
    }
}

/// A component's status and the counters it is derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluated {
    pub status: &'static str,
    pub failure_count: i32,
    pub healthy_count: i32,
    pub pressure_count: i32,
}

/// What a component looked like after the previous check.
#[derive(Debug, Clone, Default)]
pub struct Previous {
    pub status: String,
    pub failure_count: i32,
    pub healthy_count: i32,
    pub pressure_count: i32,
}

fn known(status: &str) -> &'static str {
    STATUSES
        .iter()
        .find(|known| **known == status)
        .copied()
        .unwrap_or("operational")
}

/// A node's public status. Unreachable: degraded from the second missed check in a row, offline
/// from the third, unless Wings refused the connection (`host_up`: the machine is up, its servers
/// keep running), which is at most degraded. Maintenance shows as maintenance. CPU at or over the
/// threshold for five checks in a row: degraded; full memory or disk is not slow, so it does not
/// count. Coming back from degraded or offline takes two good checks.
pub fn evaluate_node(
    previous: &Previous,
    reachable: bool,
    under_pressure: bool,
    maintenance: bool,
    host_up: bool,
) -> Evaluated {
    if !reachable {
        let failures = previous.failure_count + 1;
        let status = if failures >= 3 && !host_up {
            "offline"
        } else if failures >= 2 {
            "degraded"
        } else if previous.status.is_empty() || previous.status == "unknown" {
            "operational"
        } else {
            known(&previous.status)
        };

        return Evaluated {
            status,
            failure_count: failures,
            healthy_count: 0,
            pressure_count: 0,
        };
    }

    if maintenance {
        return Evaluated {
            status: "maintenance",
            failure_count: 0,
            healthy_count: 0,
            pressure_count: 0,
        };
    }

    if under_pressure {
        let pressure = previous.pressure_count + 1;

        return Evaluated {
            status: if pressure >= 5 {
                "degraded"
            } else {
                "operational"
            },
            failure_count: 0,
            healthy_count: 0,
            pressure_count: pressure,
        };
    }

    let recovering = matches!(previous.status.as_str(), "degraded" | "offline");
    let healthy = if recovering {
        previous.healthy_count + 1
    } else {
        previous.healthy_count.max(2)
    };

    Evaluated {
        status: if recovering && healthy < 2 {
            "degraded"
        } else {
            "operational"
        },
        failure_count: 0,
        healthy_count: healthy,
        pressure_count: 0,
    }
}

/// The website's or the panel's status: one failed probe is degraded, three in a row offline;
/// the first good probe after a failure is still degraded.
pub fn evaluate_endpoint(previous: &Previous, reachable: bool) -> Evaluated {
    if reachable {
        return Evaluated {
            status: if previous.failure_count > 0 {
                "degraded"
            } else {
                "operational"
            },
            failure_count: 0,
            healthy_count: 0,
            pressure_count: 0,
        };
    }

    let failures = previous.failure_count + 1;
    Evaluated {
        status: if failures >= 3 { "offline" } else { "degraded" },
        failure_count: failures,
        healthy_count: 0,
        pressure_count: 0,
    }
}

/// The overall status from every listed component (`kind`, `status`).
pub fn overall(components: &[(&str, &str)]) -> &'static str {
    let endpoint_down = components
        .iter()
        .any(|(kind, status)| *kind != "node" && matches!(*status, "offline" | "major_outage"));
    if endpoint_down
        || components
            .iter()
            .any(|(_, status)| *status == "major_outage")
    {
        return "major_outage";
    }

    let nodes: Vec<&str> = components
        .iter()
        .filter(|(kind, _)| *kind == "node")
        .map(|(_, status)| *status)
        .collect();
    if !nodes.is_empty() && nodes.iter().all(|status| *status == "offline") {
        return "major_outage";
    }

    for (wanted, result) in [
        (&["offline", "partial_outage"][..], "partial_outage"),
        (&["degraded"][..], "degraded"),
        (&["maintenance"][..], "maintenance"),
        (&["unknown"][..], "unknown"),
    ] {
        if components.iter().any(|(_, status)| wanted.contains(status)) {
            return result;
        }
    }

    "operational"
}

/// The status an incident's impact puts on the components it affects.
pub fn incident_status(impact: &str) -> &'static str {
    match impact {
        "critical" => "major_outage",
        "major" => "partial_outage",
        "maintenance" => "maintenance",
        _ => "degraded",
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// --- Components ---------------------------------------------------------------------------

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiStatusComponent {
    pub uuid: uuid::Uuid,
    /// `site`, `panel` or `node`.
    pub kind: String,
    pub node_uuid: Option<uuid::Uuid>,
    pub label: String,
    pub published: bool,
    pub sort: i32,
    pub status: String,
    pub last_reported: Option<chrono::DateTime<chrono::Utc>>,
    pub checked: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
struct Component {
    uuid: uuid::Uuid,
    kind: String,
    node_uuid: Option<uuid::Uuid>,
    label: String,
    published: bool,
    previous: Previous,
    last_metrics: Option<Value>,
    last_reported: Option<chrono::NaiveDateTime>,
    checked: Option<chrono::NaiveDateTime>,
}

const COMPONENT_COLUMNS: &str = "uuid, kind, node_uuid, label, published, sort, status, \
    failure_count, healthy_count, pressure_count, last_metrics, last_reported, checked";

fn map_component(row: &sqlx::postgres::PgRow) -> Result<Component, sqlx::Error> {
    Ok(Component {
        uuid: row.try_get("uuid")?,
        kind: row.try_get("kind")?,
        node_uuid: row.try_get("node_uuid")?,
        label: row.try_get("label")?,
        published: row.try_get("published")?,
        previous: Previous {
            status: row.try_get("status")?,
            failure_count: row.try_get("failure_count")?,
            healthy_count: row.try_get("healthy_count")?,
            pressure_count: row.try_get("pressure_count")?,
        },
        last_metrics: row.try_get("last_metrics")?,
        last_reported: row.try_get("last_reported")?,
        checked: row.try_get("checked")?,
    })
}

/// Published components in their order (the order the status page lists them in).
async fn published_components(
    database: &crate::database::Database,
) -> Result<Vec<Component>, anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COMPONENT_COLUMNS} FROM status_components WHERE published \
         ORDER BY CASE kind WHEN 'site' THEN 0 WHEN 'panel' THEN 1 ELSE 2 END, sort, created"
    )))
    .fetch_all(database.read())
    .await?;

    Ok(rows.iter().map(map_component).collect::<Result<_, _>>()?)
}

/// Every component, for Admin › Status.
pub async fn components(
    database: &crate::database::Database,
) -> Result<Vec<AdminApiStatusComponent>, anyhow::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COMPONENT_COLUMNS} FROM status_components \
         ORDER BY CASE kind WHEN 'site' THEN 0 WHEN 'panel' THEN 1 ELSE 2 END, sort, created"
    )))
    .fetch_all(database.read())
    .await?;

    rows.iter()
        .map(|row| {
            let component = map_component(row)?;
            Ok(AdminApiStatusComponent {
                uuid: component.uuid,
                kind: component.kind,
                node_uuid: component.node_uuid,
                label: component.label,
                published: component.published,
                sort: row.try_get("sort")?,
                status: component.previous.status,
                last_reported: component.last_reported.map(|at| at.and_utc()),
                checked: component.checked.map(|at| at.and_utc()),
            })
        })
        .collect()
}

/// Changes what an admin may change: published, order, and the label of the website and the
/// panel (a node's label follows the node's name).
pub async fn update_component(
    database: &crate::database::Database,
    component_uuid: uuid::Uuid,
    published: Option<bool>,
    sort: Option<i32>,
    label: Option<&str>,
) -> Result<bool, anyhow::Error> {
    let result = sqlx::query(
        r#"
        UPDATE status_components
        SET published = COALESCE($2, published),
            sort = COALESCE($3, sort),
            label = CASE WHEN node_uuid IS NULL THEN COALESCE($4, label) ELSE label END
        WHERE uuid = $1
        "#,
    )
    .bind(component_uuid)
    .bind(published)
    .bind(sort)
    .bind(label)
    .execute(database.write())
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Makes the components match the nodes and the configured endpoints: one per node (published
/// when it is a game node, labelled with its name), one for the website and the panel.
async fn sync_components(
    database: &crate::database::Database,
    settings: &crate::settings::status::AppSettingsStatus,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        r#"
        INSERT INTO status_components (kind, node_uuid, label, published, sort)
        SELECT 'node', nodes.uuid, nodes.name, nodes.role = 'game',
            (SELECT COALESCE(MAX(sort), 0) FROM status_components) + ROW_NUMBER() OVER (ORDER BY nodes.created)
        FROM nodes
        WHERE NOT EXISTS (SELECT 1 FROM status_components WHERE status_components.node_uuid = nodes.uuid)
        "#,
    )
    .execute(database.write())
    .await?;

    sqlx::query(
        r#"
        UPDATE status_components SET label = nodes.name
        FROM nodes
        WHERE status_components.node_uuid = nodes.uuid AND status_components.label <> nodes.name
        "#,
    )
    .execute(database.write())
    .await?;

    for (kind, label, configured) in [
        ("site", "Website", settings.site_url.is_some()),
        ("panel", "Panel", settings.panel_url.is_some()),
    ] {
        if configured {
            sqlx::query(
                r#"
                INSERT INTO status_components (kind, label)
                VALUES ($1, $2)
                ON CONFLICT (kind) WHERE node_uuid IS NULL DO NOTHING
                "#,
            )
            .bind(kind)
            .bind(label)
            .execute(database.write())
            .await?;
        } else {
            // Not probed: not listed, and its uptime goes with it.
            sqlx::query("DELETE FROM status_components WHERE kind = $1 AND node_uuid IS NULL")
                .bind(kind)
                .execute(database.write())
                .await?;
        }
    }

    Ok(())
}

// --- Collect --------------------------------------------------------------------------------

/// The `collect_status` task: one check of every component. A no-op while the status page is off.
pub async fn collect(state: &crate::State) -> Result<(), anyhow::Error> {
    let settings = state.settings.get().await?.status.clone();
    if !settings.enabled {
        return Ok(());
    }

    sync_components(&state.database, &settings).await?;

    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {COMPONENT_COLUMNS} FROM status_components"
    )))
    .fetch_all(state.database.read())
    .await?;
    let components: Vec<Component> = rows.iter().map(map_component).collect::<Result<_, _>>()?;

    let telemetry = crate::node_telemetry::latest_all(&state.database).await?;
    let maintenance: HashMap<uuid::Uuid, bool> =
        sqlx::query("SELECT uuid, maintenance_enabled FROM nodes")
            .fetch_all(state.database.read())
            .await?
            .iter()
            .map(|row| Ok((row.try_get("uuid")?, row.try_get("maintenance_enabled")?)))
            .collect::<Result<_, sqlx::Error>>()?;

    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .connect_timeout(PROBE_CONNECT_TIMEOUT)
        .user_agent(format!(
            "{} status monitor",
            state.settings.get().await?.app.name
        ))
        .build()?;

    let now = chrono::Utc::now();
    for component in components {
        let (evaluated, metrics) = match (component.kind.as_str(), component.node_uuid) {
            ("node", Some(node_uuid)) => {
                let sample = telemetry.get(&node_uuid).filter(|sample| {
                    (now - sample.sampled).num_seconds() <= TELEMETRY_STALE_SECONDS
                });
                check_node(
                    &component,
                    sample,
                    maintenance.get(&node_uuid).copied().unwrap_or(false),
                    settings.cpu_threshold,
                )
            }
            ("site", _) => {
                let reachable = probe(
                    &client,
                    settings.site_url.as_deref(),
                    settings.site_host.as_deref(),
                )
                .await;
                (
                    evaluate_endpoint(&component.previous, reachable.0),
                    reachable.1,
                )
            }
            ("panel", _) => {
                let reachable = probe(&client, settings.panel_url.as_deref(), None).await;
                (
                    evaluate_endpoint(&component.previous, reachable.0),
                    reachable.1,
                )
            }
            _ => continue,
        };

        if let Err(err) = record(&state.database, &component, &evaluated, metrics).await {
            tracing::warn!(component = %component.uuid, "failed to record a status check: {err:#}");
        }
    }

    Ok(())
}

/// A node's check from its newest telemetry. The metrics are kept only when it answered.
fn check_node(
    component: &Component,
    sample: Option<&crate::node_telemetry::AdminApiNodeTelemetry>,
    maintenance: bool,
    cpu_threshold: i32,
) -> (Evaluated, Option<Value>) {
    let metrics = sample.filter(|sample| sample.reachable).and_then(|sample| {
        let threads = sample.cpu_threads.filter(|threads| *threads > 0)?;
        let memory_total = sample.memory_total.filter(|total| *total > 0)?;
        let cpu_percent = sample.cpu_percent.unwrap_or(0.0);

        Some((
            cpu_percent,
            json!({
                // CPU in percent points, 100 per thread, as the status page and the site read it
                "cpu_used": ((cpu_percent as f64 * threads as f64) * 100.0).round() / 100.0,
                "cpu_total": threads * 100,
                "memory_used": sample.memory_used.unwrap_or(0),
                "memory_total": memory_total,
                "storage_used": sample.disk_used.unwrap_or(0),
                "storage_total": sample.disk_total.unwrap_or(0),
                "response_time_ms": sample.response_time_ms,
            }),
        ))
    });

    let reachable = metrics.is_some();
    let under_pressure = metrics
        .as_ref()
        .is_some_and(|(cpu_percent, _)| *cpu_percent >= cpu_threshold as f32);
    let host_up = !reachable && sample.is_some_and(|sample| sample.refused);

    (
        evaluate_node(
            &component.previous,
            reachable,
            under_pressure,
            maintenance,
            host_up,
        ),
        metrics.map(|(_, metrics)| metrics),
    )
}

/// Probes a health URL: reachable when it answers 2xx, with the time it took.
async fn probe(
    client: &reqwest::Client,
    url: Option<&str>,
    host: Option<&str>,
) -> (bool, Option<Value>) {
    let Some(url) = url else {
        return (false, None);
    };

    let started = std::time::Instant::now();
    let mut request = client.get(url);
    if let Some(host) = host {
        request = request.header(reqwest::header::HOST, host);
    }

    let reachable = request
        .send()
        .await
        .is_ok_and(|response| response.status().is_success());

    (
        reachable,
        Some(json!({ "response_time_ms": started.elapsed().as_millis() as u64 })),
    )
}

async fn record(
    database: &crate::database::Database,
    component: &Component,
    evaluated: &Evaluated,
    metrics: Option<Value>,
) -> Result<(), anyhow::Error> {
    let mut transaction = database.write().begin().await?;
    let node = component.kind == "node";

    sqlx::query(
        r#"
        UPDATE status_components
        SET status = $2, failure_count = $3, healthy_count = $4, pressure_count = $5,
            last_metrics = CASE WHEN $6::jsonb IS NOT NULL OR NOT $7 THEN $6::jsonb ELSE last_metrics END,
            last_reported = CASE WHEN $6::jsonb IS NOT NULL THEN NOW() ELSE last_reported END,
            checked = NOW()
        WHERE uuid = $1
        "#,
    )
    .bind(component.uuid)
    .bind(evaluated.status)
    .bind(evaluated.failure_count)
    .bind(evaluated.healthy_count)
    .bind(evaluated.pressure_count)
    .bind(metrics.as_ref())
    .bind(node)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO status_daily_uptime (component_uuid, date, total_checks, operational_checks, worst_status)
        VALUES ($1, CURRENT_DATE, 1, $2, $3)
        ON CONFLICT (component_uuid, date) DO UPDATE SET
            total_checks = status_daily_uptime.total_checks + 1,
            operational_checks = status_daily_uptime.operational_checks + EXCLUDED.operational_checks,
            worst_status = CASE WHEN $4 > (CASE status_daily_uptime.worst_status
                    WHEN 'major_outage' THEN 5 WHEN 'offline' THEN 5 WHEN 'partial_outage' THEN 4
                    WHEN 'degraded' THEN 3 WHEN 'maintenance' THEN 2 WHEN 'operational' THEN 1 ELSE 0 END)
                THEN EXCLUDED.worst_status ELSE status_daily_uptime.worst_status END
        "#,
    )
    .bind(component.uuid)
    .bind(if evaluated.status == "operational" { 1 } else { 0 })
    .bind(evaluated.status)
    .bind(priority(evaluated.status))
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

/// The `prune_status` task: daily uptime older than the history setting.
pub async fn prune(state: &crate::State) -> Result<u64, anyhow::Error> {
    let days = state.settings.get().await?.status.history_days.max(1);

    Ok(sqlx::query(
        "DELETE FROM status_daily_uptime WHERE date < CURRENT_DATE - make_interval(days => $1)",
    )
    .bind(days)
    .execute(state.database.write())
    .await?
    .rows_affected())
}

// --- Incidents and overrides ----------------------------------------------------------------

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiStatusIncidentUpdate {
    pub uuid: uuid::Uuid,
    pub status: String,
    pub message: String,
    pub created_by: Option<uuid::Uuid>,
    pub published: chrono::DateTime<chrono::Utc>,
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiStatusIncident {
    pub uuid: uuid::Uuid,
    pub title: String,
    pub impact: String,
    pub status: String,
    /// The affected components; empty: all of them.
    pub components: Vec<uuid::Uuid>,
    pub created_by: Option<uuid::Uuid>,
    pub started: chrono::DateTime<chrono::Utc>,
    pub resolved: Option<chrono::DateTime<chrono::Utc>>,
    /// Newest first.
    pub updates: Vec<AdminApiStatusIncidentUpdate>,
}

/// The newest incidents with their updates.
pub async fn incidents(
    database: &crate::database::Database,
    limit: i64,
) -> Result<Vec<AdminApiStatusIncident>, anyhow::Error> {
    let rows = sqlx::query(
        r#"
        SELECT uuid, title, impact, status, components, created_by, started, resolved
        FROM status_incidents
        ORDER BY started DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(database.read())
    .await?;

    let uuids: Vec<uuid::Uuid> = rows
        .iter()
        .map(|row| row.try_get("uuid"))
        .collect::<Result<_, _>>()?;
    let mut updates: HashMap<uuid::Uuid, Vec<AdminApiStatusIncidentUpdate>> = HashMap::new();
    for row in sqlx::query(
        r#"
        SELECT uuid, incident_uuid, status, message, created_by, published
        FROM status_incident_updates
        WHERE incident_uuid = ANY($1)
        ORDER BY published DESC
        "#,
    )
    .bind(&uuids)
    .fetch_all(database.read())
    .await?
    {
        updates
            .entry(row.try_get("incident_uuid")?)
            .or_default()
            .push(AdminApiStatusIncidentUpdate {
                uuid: row.try_get("uuid")?,
                status: row.try_get("status")?,
                message: row.try_get("message")?,
                created_by: row.try_get("created_by")?,
                published: row
                    .try_get::<chrono::NaiveDateTime, _>("published")?
                    .and_utc(),
            });
    }

    rows.iter()
        .map(|row| {
            let uuid: uuid::Uuid = row.try_get("uuid")?;
            Ok(AdminApiStatusIncident {
                uuid,
                title: row.try_get("title")?,
                impact: row.try_get("impact")?,
                status: row.try_get("status")?,
                components: row.try_get("components")?,
                created_by: row.try_get("created_by")?,
                started: row
                    .try_get::<chrono::NaiveDateTime, _>("started")?
                    .and_utc(),
                resolved: row
                    .try_get::<Option<chrono::NaiveDateTime>, _>("resolved")?
                    .map(|at| at.and_utc()),
                updates: updates.remove(&uuid).unwrap_or_default(),
            })
        })
        .collect()
}

/// Posts an incident with its first update.
pub async fn create_incident(
    database: &crate::database::Database,
    title: &str,
    impact: &str,
    status: &str,
    components: &[uuid::Uuid],
    message: &str,
    by: Option<uuid::Uuid>,
) -> Result<uuid::Uuid, anyhow::Error> {
    let mut transaction = database.write().begin().await?;

    let uuid: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO status_incidents (title, impact, status, components, created_by, resolved)
        VALUES ($1, $2, $3, $4, $5, CASE WHEN $3 = 'resolved' THEN NOW() END)
        RETURNING uuid
        "#,
    )
    .bind(title)
    .bind(impact)
    .bind(status)
    .bind(components)
    .bind(by)
    .fetch_one(&mut *transaction)
    .await?;

    sqlx::query(
        "INSERT INTO status_incident_updates (incident_uuid, status, message, created_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(uuid)
    .bind(status)
    .bind(message)
    .bind(by)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(uuid)
}

/// Changes an incident's title, impact or components.
pub async fn update_incident(
    database: &crate::database::Database,
    incident_uuid: uuid::Uuid,
    title: Option<&str>,
    impact: Option<&str>,
    components: Option<&[uuid::Uuid]>,
) -> Result<bool, anyhow::Error> {
    Ok(sqlx::query(
        r#"
        UPDATE status_incidents
        SET title = COALESCE($2, title), impact = COALESCE($3, impact),
            components = COALESCE($4, components)
        WHERE uuid = $1
        "#,
    )
    .bind(incident_uuid)
    .bind(title)
    .bind(impact)
    .bind(components)
    .execute(database.write())
    .await?
    .rows_affected()
        > 0)
}

/// Posts an update: the incident takes its status, and is resolved (or reopened) with it.
pub async fn add_incident_update(
    database: &crate::database::Database,
    incident_uuid: uuid::Uuid,
    status: &str,
    message: &str,
    by: Option<uuid::Uuid>,
) -> Result<bool, anyhow::Error> {
    let mut transaction = database.write().begin().await?;

    let updated = sqlx::query(
        r#"
        UPDATE status_incidents
        SET status = $2,
            resolved = CASE WHEN $2 = 'resolved' THEN COALESCE(resolved, NOW()) END
        WHERE uuid = $1
        "#,
    )
    .bind(incident_uuid)
    .bind(status)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        > 0;

    if updated {
        sqlx::query(
            "INSERT INTO status_incident_updates (incident_uuid, status, message, created_by) VALUES ($1, $2, $3, $4)",
        )
        .bind(incident_uuid)
        .bind(status)
        .bind(message)
        .bind(by)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(updated)
}

pub async fn delete_incident(
    database: &crate::database::Database,
    incident_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    Ok(sqlx::query("DELETE FROM status_incidents WHERE uuid = $1")
        .bind(incident_uuid)
        .execute(database.write())
        .await?
        .rows_affected()
        > 0)
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiStatusOverride {
    pub uuid: uuid::Uuid,
    /// The component; `None`: all of them.
    pub component_uuid: Option<uuid::Uuid>,
    pub status: String,
    pub note: Option<String>,
    pub starts: Option<chrono::DateTime<chrono::Utc>>,
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: Option<uuid::Uuid>,
    pub created: chrono::DateTime<chrono::Utc>,
    /// Whether it applies now.
    pub active: bool,
}

pub async fn overrides(
    database: &crate::database::Database,
) -> Result<Vec<AdminApiStatusOverride>, anyhow::Error> {
    sqlx::query(
        r#"
        SELECT uuid, component_uuid, status, note, starts, ends, created_by, created,
            (starts IS NULL OR starts <= NOW()) AND (ends IS NULL OR ends > NOW()) AS active
        FROM status_overrides
        ORDER BY created DESC
        "#,
    )
    .fetch_all(database.read())
    .await?
    .iter()
    .map(|row| {
        Ok(AdminApiStatusOverride {
            uuid: row.try_get("uuid")?,
            component_uuid: row.try_get("component_uuid")?,
            status: row.try_get("status")?,
            note: row.try_get("note")?,
            starts: row
                .try_get::<Option<chrono::NaiveDateTime>, _>("starts")?
                .map(|at| at.and_utc()),
            ends: row
                .try_get::<Option<chrono::NaiveDateTime>, _>("ends")?
                .map(|at| at.and_utc()),
            created_by: row.try_get("created_by")?,
            created: row
                .try_get::<chrono::NaiveDateTime, _>("created")?
                .and_utc(),
            active: row.try_get("active")?,
        })
    })
    .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn create_override(
    database: &crate::database::Database,
    component_uuid: Option<uuid::Uuid>,
    status: &str,
    note: Option<&str>,
    starts: Option<chrono::DateTime<chrono::Utc>>,
    ends: Option<chrono::DateTime<chrono::Utc>>,
    by: Option<uuid::Uuid>,
) -> Result<uuid::Uuid, anyhow::Error> {
    Ok(sqlx::query_scalar(
        r#"
        INSERT INTO status_overrides (component_uuid, status, note, starts, ends, created_by)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING uuid
        "#,
    )
    .bind(component_uuid)
    .bind(status)
    .bind(note)
    .bind(starts.map(|at| at.naive_utc()))
    .bind(ends.map(|at| at.naive_utc()))
    .bind(by)
    .fetch_one(database.write())
    .await?)
}

pub async fn delete_override(
    database: &crate::database::Database,
    override_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    Ok(sqlx::query("DELETE FROM status_overrides WHERE uuid = $1")
        .bind(override_uuid)
        .execute(database.write())
        .await?
        .rows_affected()
        > 0)
}

// --- The public files -----------------------------------------------------------------------

fn iso(at: chrono::DateTime<chrono::Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// `current.json`: the overall status, every published component, the nodes' load.
pub async fn current(state: &crate::State) -> Result<Value, anyhow::Error> {
    let components = published_components(&state.database).await?;
    let incidents = incidents(&state.database, 50).await?;
    let overrides = overrides(&state.database).await?;
    let now = chrono::Utc::now();

    let mut statuses: Vec<(String, Option<String>)> = components
        .iter()
        .map(|component| (component.previous.status.clone(), None))
        .collect();

    for incident in incidents
        .iter()
        .filter(|incident| incident.status != "resolved")
    {
        let status = incident_status(&incident.impact);
        for (component, (current, _)) in components.iter().zip(statuses.iter_mut()) {
            let affected =
                incident.components.is_empty() || incident.components.contains(&component.uuid);
            if affected && priority(status) > priority(current) {
                *current = status.to_string();
            }
        }
    }

    // An admin's override is the last word, over probes and incidents.
    for control in overrides.iter().filter(|control| control.active) {
        for (component, (current, note)) in components.iter().zip(statuses.iter_mut()) {
            if control
                .component_uuid
                .is_none_or(|uuid| uuid == component.uuid)
            {
                *current = control.status.clone();
                if let Some(text) = control.note.as_ref().filter(|text| !text.is_empty()) {
                    *note = Some(text.clone());
                }
            }
        }
    }

    let empty_metrics = json!({
        "cpu_used": 0.0, "cpu_total": 0, "memory_used": 0, "memory_total": 0,
        "storage_used": 0, "storage_total": 0,
    });
    let mut global = [
        ("cpu_used", 0.0f64),
        ("cpu_total", 0.0),
        ("memory_used", 0.0),
        ("memory_total", 0.0),
        ("storage_used", 0.0),
        ("storage_total", 0.0),
    ];
    let mut reporting = 0;
    let mut total_nodes = 0;
    let mut nodes = Vec::new();

    for (component, (status, _)) in components.iter().zip(statuses.iter()) {
        if component.kind != "node" {
            continue;
        }
        total_nodes += 1;

        let metrics = component
            .last_metrics
            .clone()
            .unwrap_or_else(|| empty_metrics.clone());
        for (field, sum) in global.iter_mut() {
            *sum += metrics.get(*field).and_then(Value::as_f64).unwrap_or(0.0);
        }
        let last_reported = component.last_reported.map(|at| at.and_utc());
        if last_reported.is_some_and(|at| (now - at).num_minutes() < 3)
            && component.previous.status != "offline"
        {
            reporting += 1;
        }

        nodes.push(json!({
            "label": component.label,
            "status": status,
            "last_reported_at": last_reported.map(iso),
            "latency_ms": metrics.get("response_time_ms").and_then(Value::as_i64),
            "metrics": metrics,
        }));
    }

    let kinds_statuses: Vec<(&str, &str)> = components
        .iter()
        .zip(statuses.iter())
        .map(|(component, (status, _))| (component.kind.as_str(), status.as_str()))
        .collect();
    let overall_status = overall(&kinds_statuses);

    let mut global_json = serde_json::Map::new();
    global_json.insert("reporting_nodes".into(), json!(reporting));
    global_json.insert("total_nodes".into(), json!(total_nodes));
    for (field, sum) in global {
        global_json.insert(
            field.into(),
            if field == "cpu_used" {
                json!((sum * 100.0).round() / 100.0)
            } else {
                json!(sum as i64)
            },
        );
    }

    Ok(json!({
        "schema_version": 1,
        "generated_at": iso(now),
        "release": crate::RELEASE,
        "overall": {
            "status": overall_status,
            "label": label(overall_status),
            "message": if overall_status == "operational" {
                "All monitored systems are reporting normally."
            } else {
                "One or more monitored systems require attention."
            },
        },
        "components": components.iter().zip(statuses.iter()).map(|(component, (status, note))| {
            let mut entry = json!({ "label": component.label, "status": status });
            if let Some(note) = note {
                entry["note"] = json!(note);
            }
            entry
        }).collect::<Vec<_>>(),
        "global": global_json,
        "nodes": nodes,
    }))
}

/// `history.json`: the last hour of every published node's CPU and memory, and 90 days of uptime
/// per published component.
pub async fn history(state: &crate::State) -> Result<Value, anyhow::Error> {
    let components = published_components(&state.database).await?;
    let node_uuids: Vec<uuid::Uuid> = components
        .iter()
        .filter_map(|component| component.node_uuid)
        .collect();

    let mut recent_rows: HashMap<uuid::Uuid, Vec<(String, f64, i64)>> = HashMap::new();
    for row in sqlx::query(
        r#"
        SELECT samples.node_uuid, samples.sampled, samples.cpu_percent,
            node_telemetry.cpu_threads, samples.memory_used
        FROM node_telemetry_samples samples
        LEFT JOIN node_telemetry ON node_telemetry.node_uuid = samples.node_uuid
        WHERE samples.node_uuid = ANY($1) AND samples.reachable
            AND samples.sampled > NOW() - INTERVAL '60 minutes'
        ORDER BY samples.sampled
        "#,
    )
    .bind(&node_uuids)
    .fetch_all(state.database.read())
    .await?
    {
        let threads = row.try_get::<Option<i32>, _>("cpu_threads")?.unwrap_or(0) as f64;
        let cpu = row.try_get::<Option<f32>, _>("cpu_percent")?.unwrap_or(0.0) as f64;
        recent_rows
            .entry(row.try_get("node_uuid")?)
            .or_default()
            .push((
                iso(row
                    .try_get::<chrono::NaiveDateTime, _>("sampled")?
                    .and_utc()),
                ((cpu * threads) * 100.0).round() / 100.0,
                row.try_get::<Option<i64>, _>("memory_used")?.unwrap_or(0),
            ));
    }

    let recent: Vec<Value> = components
        .iter()
        .filter(|component| component.kind == "node")
        .map(|component| {
            let rows = component
                .node_uuid
                .and_then(|uuid| recent_rows.get(&uuid))
                .cloned()
                .unwrap_or_default();
            json!({
                "label": component.label,
                "times": rows.iter().map(|(time, _, _)| time.clone()).collect::<Vec<_>>(),
                "cpu": rows.iter().map(|(_, cpu, _)| *cpu).collect::<Vec<_>>(),
                "memory": rows.iter().map(|(_, _, memory)| *memory).collect::<Vec<_>>(),
            })
        })
        .collect();

    let uuids: Vec<uuid::Uuid> = components.iter().map(|component| component.uuid).collect();
    let mut daily: HashMap<(uuid::Uuid, chrono::NaiveDate), (i32, i32, String)> = HashMap::new();
    for row in sqlx::query(
        r#"
        SELECT component_uuid, date, total_checks, operational_checks, worst_status
        FROM status_daily_uptime
        WHERE component_uuid = ANY($1) AND date > CURRENT_DATE - $2::integer
        "#,
    )
    .bind(&uuids)
    .bind(UPTIME_DAYS as i32)
    .fetch_all(state.database.read())
    .await?
    {
        daily.insert(
            (row.try_get("component_uuid")?, row.try_get("date")?),
            (
                row.try_get("total_checks")?,
                row.try_get("operational_checks")?,
                row.try_get("worst_status")?,
            ),
        );
    }

    let today = chrono::Utc::now().date_naive();
    let uptime: Vec<Value> = components
        .iter()
        .map(|component| {
            let (mut checks, mut operational) = (0i64, 0i64);
            let days: Vec<Value> = (0..UPTIME_DAYS)
                .rev()
                .map(|back| {
                    let date = today - chrono::Duration::days(back);
                    let mut day = json!({ "date": date.to_string(), "status": "unknown" });
                    if let Some((total, ok, worst)) = daily.get(&(component.uuid, date)) {
                        day["status"] = json!(worst);
                        day["total_checks"] = json!(total);
                        day["operational_checks"] = json!(ok);
                        checks += *total as i64;
                        operational += *ok as i64;
                    }
                    day
                })
                .collect();

            json!({
                "label": component.label,
                "uptime_percent": if checks > 0 {
                    ((operational as f64 / checks as f64) * 100_000.0).round() / 1000.0
                } else {
                    0.0
                },
                "days": days,
            })
        })
        .collect();

    Ok(json!({
        "schema_version": 1,
        "generated_at": iso(chrono::Utc::now()),
        "recent": recent,
        "uptime": uptime,
    }))
}

/// `incidents.json`: the newest 50 incidents, affected components by label.
pub async fn incidents_file(state: &crate::State) -> Result<Value, anyhow::Error> {
    let components = published_components(&state.database).await?;
    let labels: HashMap<uuid::Uuid, &str> = components
        .iter()
        .map(|component| (component.uuid, component.label.as_str()))
        .collect();

    let incidents: Vec<Value> = incidents(&state.database, 50)
        .await?
        .into_iter()
        .map(|incident| {
            let affected: Vec<&str> = if incident.components.is_empty() {
                vec!["All systems"]
            } else {
                incident
                    .components
                    .iter()
                    .map(|uuid| labels.get(uuid).copied().unwrap_or("Service component"))
                    .collect()
            };

            json!({
                "title": incident.title,
                "impact": incident.impact,
                "status": incident.status,
                "components": affected,
                "started_at": iso(incident.started),
                "resolved_at": incident.resolved.map(iso),
                "updates": incident.updates.iter().map(|update| json!({
                    "status": update.status,
                    "message": update.message,
                    "published_at": iso(update.published),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(json!({
        "schema_version": 1,
        "generated_at": iso(chrono::Utc::now()),
        "incidents": incidents,
    }))
}

/// `targets.json`: what the status Worker probes from outside (each published node's label and
/// the host and port its Wings answers on), so the page knows which nodes are down when the
/// panel is. Node addresses are not secret: players connect to them.
pub async fn targets(state: &crate::State) -> Result<Value, anyhow::Error> {
    let components = published_components(&state.database).await?;
    let urls: HashMap<uuid::Uuid, String> = sqlx::query("SELECT uuid, url FROM nodes")
        .fetch_all(state.database.read())
        .await?
        .iter()
        .map(|row| Ok((row.try_get("uuid")?, row.try_get("url")?)))
        .collect::<Result<_, sqlx::Error>>()?;

    let nodes: Vec<Value> = components
        .iter()
        .filter_map(|component| {
            let url = reqwest::Url::parse(urls.get(&component.node_uuid?)?).ok()?;
            Some(json!({
                "label": component.label,
                "host": url.host_str()?,
                "port": url.port_or_known_default()?,
            }))
        })
        .collect();

    Ok(json!({
        "schema_version": 1,
        "generated_at": iso(chrono::Utc::now()),
        "nodes": nodes,
    }))
}

/// `feed.xml`: the incidents as RSS 2.0.
pub async fn feed(state: &crate::State) -> Result<String, anyhow::Error> {
    let settings = state.settings.get().await?;
    let name = escape_xml(&settings.app.name);
    let base = escape_xml(&format!(
        "{}/",
        settings
            .status
            .page_url
            .as_deref()
            .unwrap_or(&settings.app.url)
            .trim_end_matches('/')
    ));
    drop(settings);

    let file = incidents_file(state).await?;
    let mut items = String::new();
    for incident in file["incidents"].as_array().into_iter().flatten() {
        let title = incident["title"].as_str().unwrap_or_default();
        let started = incident["started_at"]
            .as_str()
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
            .map(|at| at.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);
        let description = incident["updates"][0]["message"]
            .as_str()
            .or_else(|| incident["status"].as_str())
            .unwrap_or_default();
        let guid = {
            use sha2::Digest;
            hex::encode(sha2::Sha256::digest(format!(
                "{title}{}",
                started.to_rfc2822()
            )))
        };

        items.push_str(&format!(
            "<item><title>{}</title><link>{base}</link><guid isPermaLink=\"false\">incident-{}</guid><pubDate>{}</pubDate><description>{}</description></item>",
            escape_xml(title),
            guid.get(..40).unwrap_or(&guid),
            started.to_rfc2822(),
            escape_xml(description),
        ));
    }

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel><title>{name} incidents</title><link>{base}</link><description>Operational updates for {name}.</description><lastBuildDate>{}</lastBuildDate>{items}</channel></rss>",
        chrono::Utc::now().to_rfc2822(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn previous(status: &str, failures: i32, healthy: i32, pressure: i32) -> Previous {
        Previous {
            status: status.into(),
            failure_count: failures,
            healthy_count: healthy,
            pressure_count: pressure,
        }
    }

    #[test]
    fn one_missed_check_is_a_blip() {
        let state = evaluate_node(
            &previous("operational", 0, 2, 0),
            false,
            false,
            false,
            false,
        );
        assert_eq!(state.status, "operational");
        assert_eq!(state.failure_count, 1);
    }

    #[test]
    fn missed_checks_degrade_then_go_offline() {
        let second = evaluate_node(
            &previous("operational", 1, 0, 0),
            false,
            false,
            false,
            false,
        );
        assert_eq!(second.status, "degraded");
        let third = evaluate_node(&previous("degraded", 2, 0, 0), false, false, false, false);
        assert_eq!(third.status, "offline");
    }

    #[test]
    fn a_refused_port_is_at_most_degraded() {
        let state = evaluate_node(&previous("degraded", 5, 0, 0), false, false, false, true);
        assert_eq!(state.status, "degraded");
    }

    #[test]
    fn recovery_takes_two_good_checks() {
        let first = evaluate_node(&previous("offline", 4, 0, 0), true, false, false, false);
        assert_eq!(first.status, "degraded");
        let second = evaluate_node(
            &previous("degraded", 0, first.healthy_count, 0),
            true,
            false,
            false,
            false,
        );
        assert_eq!(second.status, "operational");
    }

    #[test]
    fn cpu_pressure_degrades_after_five_checks() {
        let fourth = evaluate_node(&previous("operational", 0, 2, 3), true, true, false, false);
        assert_eq!(fourth.status, "operational");
        let fifth = evaluate_node(&previous("operational", 0, 0, 4), true, true, false, false);
        assert_eq!(fifth.status, "degraded");
    }

    #[test]
    fn maintenance_shows() {
        let state = evaluate_node(&previous("operational", 0, 2, 0), true, true, true, false);
        assert_eq!(state.status, "maintenance");
    }

    #[test]
    fn endpoints() {
        assert_eq!(
            evaluate_endpoint(&previous("", 0, 0, 0), true).status,
            "operational"
        );
        assert_eq!(
            evaluate_endpoint(&previous("", 1, 0, 0), true).status,
            "degraded"
        );
        assert_eq!(
            evaluate_endpoint(&previous("", 0, 0, 0), false).status,
            "degraded"
        );
        assert_eq!(
            evaluate_endpoint(&previous("", 2, 0, 0), false).status,
            "offline"
        );
    }

    #[test]
    fn overall_status() {
        assert_eq!(
            overall(&[("panel", "operational"), ("node", "operational")]),
            "operational"
        );
        assert_eq!(
            overall(&[("panel", "offline"), ("node", "operational")]),
            "major_outage"
        );
        assert_eq!(
            overall(&[("node", "offline"), ("node", "offline")]),
            "major_outage"
        );
        assert_eq!(
            overall(&[("node", "offline"), ("node", "operational")]),
            "partial_outage"
        );
        assert_eq!(
            overall(&[("node", "degraded"), ("node", "maintenance")]),
            "degraded"
        );
        assert_eq!(overall(&[("node", "maintenance")]), "maintenance");
        assert_eq!(overall(&[]), "operational");
    }

    #[test]
    fn xml_is_escaped() {
        assert_eq!(escape_xml("a < b & \"c\""), "a &lt; b &amp; &quot;c&quot;");
    }
}
