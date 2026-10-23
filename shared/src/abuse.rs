//! plugged.host: abuse protection (docs/features/abuse.md).
//!
//! Three checks feed one table (`abuse_findings`, one row per server or node and rule):
//!
//! - the full scan of a server (`scan_server`): its startup values, every file name in its tree
//!   and the likely file contents, and its console; every server every three hours
//!   (`scan_all`), and on demand;
//! - the per-minute watch (`watch`): servers pinned at their CPU limit get their console read
//!   and, after a while, the full scan; on nodes running the Wings fork, what runs in each
//!   container and what it connects to, and the node's own processes;
//! - an admin's "Scan now".
//!
//! The mode (Admin › Settings › Abuse) decides what happens: `suspend` suspends a server with a
//! critical finding that is not dismissed (kind `abuse`, deleted after the configured hours
//! unless it is unsuspended or a finding on it is dismissed), `report` records and alerts only,
//! `off` checks nothing. Review findings never suspend. The rules are all here and in
//! `signatures`; Wings only collects.

pub mod inspect;
pub mod signatures;

use crate::{
    models::server::{Server, suspension},
    settings::abuse::AbuseMode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::{collections::HashSet, time::Duration};
use tokio::io::AsyncReadExt;
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// What the Terms forbid outright; suspends in `suspend` mode.
    Critical,
    /// Shown to admins only.
    Review,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Review => "review",
        }
    }
}

/// One thing the checks found on a server (or a node). `rule` is the dedupe key: the same rule
/// on the same server updates one row instead of adding a new one on every scan.
#[derive(Debug, Clone)]
pub struct Finding {
    pub rule: String,
    pub category: String,
    pub severity: Severity,
    pub source: String,
    pub summary: String,
    pub evidence: Value,
}

impl Finding {
    pub fn is_critical(&self) -> bool {
        self.severity == Severity::Critical
    }
}

pub const STATUS_OPEN: &str = "open";
pub const STATUS_ACTIONED: &str = "actioned";
pub const STATUS_DISMISSED: &str = "dismissed";
pub const STATUS_RESOLVED: &str = "resolved";
pub const STATUSES: &[&str] = &[
    STATUS_OPEN,
    STATUS_ACTIONED,
    STATUS_DISMISSED,
    STATUS_RESOLVED,
];

/// Wings' whole-tree search may take this long before the bounded walk takes over.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Files the search lists at most.
const SEARCH_LIMIT: u64 = 50_000;
const WALK_MAX_DIRS: usize = 400;
const WALK_MAX_DEPTH: usize = 6;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const CONSOLE_TIMEOUT: Duration = Duration::from_secs(10);
const CONSOLE_MAX_BYTES: u64 = 1 << 20;
/// Open findings not seen again for this long are closed as resolved.
const STALE_DAYS: i64 = 7;
/// Servers checked at once by the full scan.
const SCAN_CONCURRENCY: usize = 4;
/// Nodes asked at once by the watch.
const WATCH_CONCURRENCY: usize = 8;
const WATCH_TIMEOUT: Duration = Duration::from_secs(20);

/// The CPU watch: console read at most this often while a server is pinned.
const CONSOLE_EVERY: i64 = 600;
/// The CPU watch: full scan at most this often while a server stays pinned.
const FULL_CHECK_EVERY: i64 = 7200;
/// One finding alert per server or node per this many seconds; suspensions always post.
const ALERT_EVERY: u64 = 1800;
/// The owner sweep after a catch, once per owner per this many seconds.
const SWEEP_EVERY: u64 = 3600;

const ALERT_RED: u32 = 15_548_997;
const ALERT_YELLOW: u32 = 16_705_372;

// --- Scanning ------------------------------------------------------------------------------

/// What a full scan of one server found.
pub struct ScanResult {
    /// Whether its node answered (the files and the console were checked).
    pub reachable: bool,
    pub findings: Vec<Finding>,
    pub recorded: Option<RecordResult>,
}

/// The full check of one server through the panel and Wings alone: startup values, every file
/// name and the likely file contents, and the console. `enforce` false records without
/// suspending; `sweep` checks the owner's other servers after a new critical finding.
pub async fn scan_server(
    state: &crate::State,
    server: &Server,
    enforce: bool,
    sweep: bool,
) -> Result<ScanResult, anyhow::Error> {
    let result = check_server(state, server, enforce).await?;
    if sweep
        && result
            .recorded
            .as_ref()
            .is_some_and(|recorded| recorded.new_critical)
    {
        sweep_owner(state, server, enforce).await;
    }

    Ok(result)
}

/// [`scan_server`] without the owner sweep (which itself runs this for each of the owner's
/// servers).
async fn check_server(
    state: &crate::State,
    server: &Server,
    enforce: bool,
) -> Result<ScanResult, anyhow::Error> {
    let variables: Vec<(String, String)> =
        crate::models::server_variable::ServerVariable::all_by_server_uuid_egg_uuid(
            &state.database,
            server.uuid,
            server.egg.uuid,
        )
        .await?
        .into_iter()
        .map(|variable| (variable.variable.env_variable.to_string(), variable.value))
        .collect();

    let mut findings = inspect::inspect_startup(&server.startup, &variables);

    let client = match server.node.fetch_cached(&state.database).await {
        Ok(node) => node.api_client(&state.database).await.ok(),
        Err(_) => None,
    };

    let mut reachable = false;
    if let Some(client) = client {
        let main_files: HashSet<String> = variables
            .iter()
            .filter(|(env, _)| inspect::MAIN_FILE_VARIABLES.contains(&env.as_str()))
            .map(|(_, value)| value.trim().trim_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .collect();

        if let Some(tree) = file_tree(&client, server.uuid).await {
            reachable = true;

            let mut plan =
                inspect::plan_files(&tree, main_files, inspect::is_generic_egg(&server.egg.name));
            for path in plan.reads.clone() {
                if let Some(content) = read_file(&client, server.uuid, &path).await {
                    plan.add_content(&path, &content);
                }
            }
            findings.extend(plan.findings());

            if let Some(lines) = console_lines(&client, server.uuid).await {
                findings.extend(inspect::inspect_console(&lines));
            }
        }
    }

    let recorded = record(state, Target::Server(server), &findings, enforce).await?;

    Ok(ScanResult {
        reachable,
        findings,
        recorded: Some(recorded),
    })
}

/// The whole file tree: Wings' search with no filter lists every file (with its size and the
/// type sniffed from its first bytes) in one call; the folders come from their paths. A server
/// too big to search in time gets a bounded walk instead. `None` when the node did not answer.
async fn file_tree(
    client: &wings_api::client::WingsClient,
    server: uuid::Uuid,
) -> Option<Vec<inspect::TreeEntry>> {
    let search = tokio::time::timeout(
        SEARCH_TIMEOUT,
        client.post_servers_server_files_search(
            server,
            &wings_api::servers_server_files_search::post::RequestBody {
                root: "/".into(),
                path_filter: None,
                size_filter: None,
                content_filter: None,
                per_page: SEARCH_LIMIT,
            },
        ),
    )
    .await;

    if let Ok(Ok(response)) = search {
        return Some(inspect::with_directories(
            response
                .results
                .into_iter()
                .map(|entry| inspect::TreeEntry {
                    path: entry.name.to_string(),
                    directory: entry.directory,
                    size: entry.size,
                    mime: entry.mime.to_string(),
                })
                .collect(),
        ));
    }

    walk(client, server).await
}

/// Breadth first, so the top of the tree is always covered: at most [`WALK_MAX_DIRS`] folders,
/// [`WALK_MAX_DEPTH`] deep, skipping bulk data. `world/` itself is opened (a nezha-agent was
/// hidden there once), its region data is not.
async fn walk(
    client: &wings_api::client::WingsClient,
    server: uuid::Uuid,
) -> Option<Vec<inspect::TreeEntry>> {
    let mut entries = Vec::new();
    let mut queue = std::collections::VecDeque::from([(String::new(), 0usize)]);
    let mut listed = 0;

    while let Some((dir, depth)) = queue.pop_front() {
        if listed >= WALK_MAX_DIRS {
            break;
        }

        let items = match client
            .get_servers_server_files_list_directory(
                server,
                &wings_api::servers_server_files_list_directory::get::Query {
                    directory: Some(format!("/{dir}").into()),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(items) => items,
            Err(_) if dir.is_empty() => return None,
            Err(_) => continue,
        };
        listed += 1;

        for item in items {
            let path = if dir.is_empty() {
                item.name.to_string()
            } else {
                format!("{dir}/{}", item.name)
            };

            if item.directory
                && depth < WALK_MAX_DEPTH
                && !item.symlink
                && !inspect::WALK_SKIP.contains(&item.name.to_ascii_lowercase().as_str())
            {
                queue.push_back((path.clone(), depth + 1));
            }

            entries.push(inspect::TreeEntry {
                path,
                directory: item.directory,
                size: item.size,
                mime: item.mime.to_string(),
            });
        }
    }

    Some(entries)
}

async fn read_file(
    client: &wings_api::client::WingsClient,
    server: uuid::Uuid,
    path: &str,
) -> Option<Vec<u8>> {
    tokio::time::timeout(READ_TIMEOUT, async {
        let reader = client
            .get_servers_server_files_contents(
                server,
                &wings_api::servers_server_files_contents::get::Query {
                    file: Some(format!("/{path}").into()),
                    max_size: Some(inspect::MAX_READ_BYTES),
                    ..Default::default()
                },
            )
            .await
            .ok()?;

        let mut content = Vec::new();
        reader
            .take(inspect::MAX_READ_BYTES)
            .read_to_end(&mut content)
            .await
            .ok()?;
        Some(content)
    })
    .await
    .ok()
    .flatten()
}

/// The last lines of the console Wings keeps.
async fn console_lines(
    client: &wings_api::client::WingsClient,
    server: uuid::Uuid,
) -> Option<Vec<String>> {
    tokio::time::timeout(CONSOLE_TIMEOUT, async {
        let reader = client
            .get_servers_server_logs(
                server,
                &wings_api::servers_server_logs::get::Query {
                    lines: Some(inspect::CONSOLE_LINES),
                    ..Default::default()
                },
            )
            .await
            .ok()?;

        let mut content = Vec::new();
        reader
            .take(CONSOLE_MAX_BYTES)
            .read_to_end(&mut content)
            .await
            .ok()?;
        Some(
            String::from_utf8_lossy(&content)
                .lines()
                .map(str::to_string)
                .collect(),
        )
    })
    .await
    .ok()
    .flatten()
}

// --- Recording and acting --------------------------------------------------------------------

/// Whose findings these are.
#[derive(Clone, Copy)]
pub enum Target<'a> {
    Server(&'a Server),
    /// What runs on the node itself.
    Node(uuid::Uuid),
}

#[derive(Debug, Default)]
pub struct RecordResult {
    pub recorded: usize,
    pub suspended: bool,
    /// A critical finding is new (or seen again after it was resolved).
    pub new_critical: bool,
    /// Summaries of the critical findings that are not dismissed.
    pub critical: Vec<String>,
    pub error: Option<String>,
}

struct Recorded {
    uuid: uuid::Uuid,
    status: String,
    reopened: bool,
    summary: String,
    critical: bool,
}

/// Records findings (one row per target and rule, reopened when it was resolved) and acts on
/// them as the mode says. Dismissing a finding is the one override: it stays dismissed when it is
/// seen again. Unsuspending a server without dismissing gets it suspended again on the next hit.
pub async fn record(
    state: &crate::State,
    target: Target<'_>,
    findings: &[Finding],
    enforce: bool,
) -> Result<RecordResult, anyhow::Error> {
    let mut result = RecordResult::default();
    let mode = state.settings.get().await?.abuse.mode;
    if findings.is_empty() || mode == AbuseMode::Off {
        return Ok(result);
    }

    let (server_uuid, node_uuid, fingerprint_prefix) = match target {
        Target::Server(server) => (
            Some(server.uuid),
            Some(server.node.uuid),
            format!("server:{}", server.uuid),
        ),
        Target::Node(node_uuid) => (None, Some(node_uuid), format!("node:{node_uuid}")),
    };

    let mut rows = Vec::with_capacity(findings.len());
    for finding in findings {
        let row = sqlx::query(
            r#"
            WITH previous AS (SELECT status FROM abuse_findings WHERE fingerprint = $1)
            INSERT INTO abuse_findings
                (fingerprint, server_uuid, node_uuid, rule, category, severity, source, summary, evidence, hits)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)
            ON CONFLICT (fingerprint) DO UPDATE SET
                node_uuid = EXCLUDED.node_uuid,
                rule = EXCLUDED.rule,
                category = EXCLUDED.category,
                severity = EXCLUDED.severity,
                source = EXCLUDED.source,
                summary = EXCLUDED.summary,
                evidence = EXCLUDED.evidence,
                hits = abuse_findings.hits + 1,
                last_seen = NOW(),
                status = CASE WHEN abuse_findings.status = 'resolved' THEN 'open' ELSE abuse_findings.status END,
                action = CASE WHEN abuse_findings.status = 'resolved' THEN NULL ELSE abuse_findings.action END,
                actioned = CASE WHEN abuse_findings.status = 'resolved' THEN NULL ELSE abuse_findings.actioned END,
                resolved_by = CASE WHEN abuse_findings.status = 'resolved' THEN NULL ELSE abuse_findings.resolved_by END,
                resolved = CASE WHEN abuse_findings.status = 'resolved' THEN NULL ELSE abuse_findings.resolved END
            RETURNING uuid, status, (SELECT status FROM previous) AS previous_status
            "#,
        )
        .bind(format!("{fingerprint_prefix}:{}", finding.rule))
        .bind(server_uuid)
        .bind(node_uuid)
        .bind(inspect::clip(&finding.rule, 64))
        .bind(inspect::clip(&finding.category, 32))
        .bind(finding.severity.as_str())
        .bind(inspect::clip(&finding.source, 16))
        .bind(inspect::clip(&finding.summary, 500))
        .bind(&finding.evidence)
        .fetch_one(state.database.write())
        .await?;

        let previous: Option<String> = row.try_get("previous_status")?;
        rows.push(Recorded {
            uuid: row.try_get("uuid")?,
            status: row.try_get("status")?,
            reopened: previous.is_none_or(|status| status == STATUS_RESOLVED),
            summary: finding.summary.clone(),
            critical: finding.is_critical(),
        });
        result.recorded += 1;
    }

    let live: Vec<&Recorded> = rows
        .iter()
        .filter(|row| row.status != STATUS_DISMISSED)
        .collect();
    let actionable: Vec<&Recorded> = live.iter().copied().filter(|row| row.critical).collect();
    let fresh_critical: Vec<&Recorded> = actionable
        .iter()
        .copied()
        .filter(|row| row.reopened)
        .collect();
    result.critical = actionable.iter().map(|row| row.summary.clone()).collect();
    result.new_critical = !fresh_critical.is_empty();

    if let Target::Server(server) = target
        && !actionable.is_empty()
        && enforce
        && mode == AbuseMode::Suspend
    {
        suspend_for(state, server, &actionable, &mut result).await;
    }

    if result.suspended || !fresh_critical.is_empty() {
        let summaries: Vec<&str> = if result.suspended {
            actionable.iter().map(|row| row.summary.as_str()).collect()
        } else {
            fresh_critical
                .iter()
                .map(|row| row.summary.as_str())
                .collect()
        };
        alert(state, target, &summaries, result.suspended, mode).await;
    }

    Ok(result)
}

/// [`record`] for a server, then the owner sweep after a new critical finding.
async fn record_and_sweep(
    state: &crate::State,
    server: &Server,
    findings: &[Finding],
) -> Result<RecordResult, anyhow::Error> {
    let result = record(state, Target::Server(server), findings, true).await?;
    if result.new_critical {
        sweep_owner(state, server, true).await;
    }

    Ok(result)
}

/// Suspends for the critical findings, under a per-server lock against a fresh copy: the watch
/// and the scan can catch the same server within seconds of each other. A server that is
/// already suspended only gets its findings marked as handled.
async fn suspend_for(
    state: &crate::State,
    server: &Server,
    actionable: &[&Recorded],
    result: &mut RecordResult,
) {
    use crate::models::ByUuid;

    let _lock = match state
        .cache
        .lock(
            format!("abuse::suspend::{}", server.uuid),
            Some(60),
            Some(15_000),
        )
        .await
    {
        Ok(lock) => lock,
        Err(_) => {
            result.error = Some("another check is suspending this server".into());
            return;
        }
    };

    let mut fresh = match Server::by_uuid_optional(&state.database, server.uuid).await {
        Ok(Some(fresh)) => fresh,
        Ok(None) => return,
        Err(err) => {
            result.error = Some(err.to_string());
            return;
        }
    };

    let action = if fresh.suspended {
        "already suspended"
    } else {
        let hours = state
            .settings
            .get()
            .await
            .map(|settings| settings.abuse.delete_after_hours)
            .unwrap_or(0);
        let reason = inspect::clip(
            &actionable
                .iter()
                .map(|row| row.summary.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            1000,
        );

        if let Err(err) = fresh
            .suspend(
                state,
                suspension::Suspend {
                    kind: suspension::SuspensionKind::Abuse,
                    reason: Some(reason),
                    by: None,
                    delete_at: (hours > 0)
                        .then(|| chrono::Utc::now() + chrono::Duration::hours(hours as i64)),
                },
            )
            .await
        {
            // A server mid-transfer cannot be suspended; the finding stays open and the next
            // check tries again.
            result.error = Some(err.to_string());
            return;
        }

        result.suspended = true;
        "suspended"
    };

    let uuids: Vec<uuid::Uuid> = actionable.iter().map(|row| row.uuid).collect();
    if let Err(err) = sqlx::query(
        r#"
        UPDATE abuse_findings SET status = 'actioned', action = $2, actioned = NOW()
        WHERE uuid = ANY($1) AND status = 'open'
        "#,
    )
    .bind(&uuids)
    .bind(action)
    .execute(state.database.write())
    .await
    {
        tracing::warn!(server = %server.uuid, "failed to mark abuse findings as actioned: {err:#}");
    }
}

/// Someone caught mining on one server usually runs the same kit on the rest: their other
/// servers are checked now, in the background, instead of at the next full scan. Once an hour
/// per owner.
async fn sweep_owner(state: &crate::State, server: &Server, enforce: bool) {
    let key = format!("abuse::sweep::{}", server.owner.uuid);
    if state.cache.exists(&key).await.unwrap_or(true) {
        return;
    }
    if state.cache.set(&key, SWEEP_EVERY, &true).await.is_err() {
        return;
    }

    let state = state.clone();
    let owner_uuid = server.owner.uuid;
    let caught = server.uuid;
    tokio::spawn(async move {
        let mut page = 1;
        loop {
            let servers = match Server::by_owner_uuid_with_pagination(
                &state.database,
                owner_uuid,
                page,
                50,
                None,
            )
            .await
            {
                Ok(servers) => servers,
                Err(err) => {
                    tracing::warn!(owner = %owner_uuid, "abuse owner sweep failed: {err:#}");
                    return;
                }
            };
            if servers.data.is_empty() {
                return;
            }

            for other in servers.data {
                if other.uuid == caught || other.suspended {
                    continue;
                }
                if let Err(err) = check_server(&state, &other, enforce).await {
                    tracing::warn!(server = %other.uuid, "abuse owner sweep scan failed: {err:#}");
                }
            }
            page += 1;
        }
    });
}

/// A Discord embed to the alert webhook (none set: no alerts). Never fails the caller.
async fn post_alert(state: &crate::State, embed: Value) {
    let (webhook, app_name) = match state.settings.get().await {
        Ok(settings) => (
            settings.abuse.alert_webhook.clone(),
            settings.app.name.clone(),
        ),
        Err(_) => return,
    };
    let Some(webhook) = webhook else {
        return;
    };

    if let Err(err) = state
        .client
        .post(webhook.as_str())
        .timeout(Duration::from_secs(5))
        .json(&json!({
            "username": format!("{app_name} abuse").trim(),
            "allowed_mentions": { "parse": [] },
            "embeds": [embed],
        }))
        .send()
        .await
    {
        tracing::warn!("failed to post an abuse alert: {err:#}");
    }
}

async fn alert(
    state: &crate::State,
    target: Target<'_>,
    summaries: &[&str],
    suspended: bool,
    mode: AbuseMode,
) {
    if summaries.is_empty() {
        return;
    }

    let key = match target {
        Target::Server(server) => format!("abuse::alert::server:{}", server.uuid),
        Target::Node(node) => format!("abuse::alert::node:{node}"),
    };
    if !suspended {
        if state.cache.exists(&key).await.unwrap_or(true) {
            return;
        }
        let _ = state.cache.set(&key, ALERT_EVERY, &true).await;
    }

    let node_name = match target {
        Target::Server(server) => server
            .node
            .fetch_cached(&state.database)
            .await
            .map(|node| node.name.to_string())
            .unwrap_or_else(|_| "?".into()),
        Target::Node(node_uuid) => {
            sqlx::query_scalar::<_, String>("SELECT name FROM nodes WHERE uuid = $1")
                .bind(node_uuid)
                .fetch_optional(state.database.read())
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "?".into())
        }
    };

    let title = match target {
        Target::Server(server) if suspended => {
            format!("Suspended {} ({})", server.name, short_id(server))
        }
        Target::Server(server) if mode == AbuseMode::Report => format!(
            "Would suspend {} ({}), report-only mode",
            server.name,
            short_id(server)
        ),
        Target::Server(server) => format!("Flagged {} ({})", server.name, short_id(server)),
        Target::Node(_) => format!("Suspicious process on node {node_name}"),
    };

    let mut fields = Vec::new();
    if let Target::Server(server) = target {
        fields.push(json!({ "name": "Owner", "value": server.owner.username, "inline": true }));
    }
    fields.push(json!({ "name": "Node", "value": node_name, "inline": true }));
    if let Target::Server(server) = target {
        fields.push(json!({ "name": "Egg", "value": server.egg.name, "inline": true }));
    }

    let app_url = state
        .settings
        .get()
        .await
        .map(|settings| settings.app.url.trim_end_matches('/').to_string())
        .unwrap_or_default();

    post_alert(
        state,
        json!({
            "title": inspect::clip(&title, 250),
            "url": format!("{app_url}/admin/abuse"),
            "description": inspect::clip(
                &summaries.iter().take(10).map(|summary| format!("• {summary}")).collect::<Vec<_>>().join("\n"),
                3900,
            ),
            "color": if suspended { ALERT_RED } else { ALERT_YELLOW },
            "fields": fields,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    )
    .await;
}

/// The alert for a server the purge is about to delete (`suspension::purge_due`).
pub async fn alert_deleted(state: &crate::State, server: &Server) {
    let reason = server
        .suspension
        .as_ref()
        .and_then(|suspension| suspension.reason.clone())
        .unwrap_or_else(|| "Suspended by the abuse checks.".into());
    let node_name = server
        .node
        .fetch_cached(&state.database)
        .await
        .map(|node| node.name.to_string())
        .unwrap_or_else(|_| "?".into());

    post_alert(
        state,
        json!({
            "title": inspect::clip(&format!("Deleted {} ({})", server.name, short_id(server)), 250),
            "description": inspect::clip(
                &reason.lines().map(|line| format!("• {line}")).collect::<Vec<_>>().join("\n"),
                3900,
            ),
            "color": ALERT_RED,
            "fields": [
                { "name": "Owner", "value": server.owner.username, "inline": true },
                { "name": "Node", "value": node_name, "inline": true },
            ],
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    )
    .await;
}

// --- Tasks -----------------------------------------------------------------------------------

/// The `scan_abuse` task: the full check of every server that is not suspended, then open
/// findings not seen for [`STALE_DAYS`] are closed.
pub async fn scan_all(state: &crate::State) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;
    use futures_util::StreamExt;

    if state.settings.get().await?.abuse.mode == AbuseMode::Off {
        return Ok(());
    }

    let uuids: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT uuid FROM servers WHERE NOT suspended ORDER BY created")
            .fetch_all(state.database.read())
            .await?;

    let mut scans = futures_util::stream::iter(uuids.into_iter().map(|uuid| async move {
        let server = match Server::by_uuid_optional(&state.database, uuid).await {
            Ok(Some(server)) if !server.suspended => server,
            Ok(_) => return,
            Err(err) => {
                tracing::warn!(server = %uuid, "abuse scan could not load the server: {err:#}");
                return;
            }
        };
        if let Err(err) = scan_server(state, &server, true, true).await {
            tracing::warn!(server = %uuid, "abuse scan failed: {err:#}");
        }
    }))
    .buffer_unordered(SCAN_CONCURRENCY);
    while scans.next().await.is_some() {}

    let closed = sqlx::query(
        r#"
        UPDATE abuse_findings SET status = 'resolved', resolved = NOW()
        WHERE status = 'open' AND last_seen < NOW() - make_interval(days => $1)
        "#,
    )
    .bind(STALE_DAYS as i32)
    .execute(state.database.write())
    .await?
    .rows_affected();
    if closed > 0 {
        tracing::info!("closed {closed} abuse findings not seen for {STALE_DAYS} days");
    }

    Ok(())
}

/// A server's CPU streak, kept in the cache between watch runs.
#[derive(Serialize, Deserialize, Default)]
struct CpuStreak {
    since: i64,
    samples: u32,
    sum: f64,
    console_at: i64,
    full_at: i64,
}

/// The `watch_abuse` task, every minute. On every node, which servers run at their CPU limit (a
/// miner pins its CPU for hours): one at the configured percent gets its console read at once
/// (every 10 minutes), one that stays there for the configured minutes gets a review finding
/// and the full check (every 2 hours), which is what suspends a miner. Full CPU alone never
/// suspends: a Minecraft server generating chunks looks the same. On nodes running the Wings
/// fork, also what runs in each container and on the node itself.
pub async fn watch(state: &crate::State) -> Result<(), anyhow::Error> {
    use futures_util::StreamExt;

    if state.settings.get().await?.abuse.mode == AbuseMode::Off {
        return Ok(());
    }

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

    let mut runs = futures_util::stream::iter(nodes.into_iter().map(|node| async move {
        let Ok(client) = node.api_client(&state.database).await else {
            return;
        };

        if let Err(err) = watch_cpu(state, node.uuid, &client).await {
            tracing::debug!(node = %node.uuid, "abuse CPU watch skipped the node: {err:#}");
        }
        if let Err(err) = watch_processes(state, node.uuid, &client).await {
            tracing::debug!(node = %node.uuid, "abuse process check skipped the node: {err:#}");
        }
    }))
    .buffer_unordered(WATCH_CONCURRENCY);
    while runs.next().await.is_some() {}

    Ok(())
}

async fn watch_cpu(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    client: &wings_api::client::WingsClient,
) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;

    let utilization =
        tokio::time::timeout(WATCH_TIMEOUT, client.get_servers_utilization()).await??;

    let (percent, minutes) = {
        let settings = state.settings.get().await?;
        (
            settings.abuse.cpu_percent.clamp(50, 100) as f64,
            settings.abuse.cpu_minutes.clamp(5, 1440) as i64,
        )
    };

    let limits: Vec<(uuid::Uuid, i32)> = sqlx::query(
        "SELECT uuid, cpu FROM servers WHERE node_uuid = $1 AND NOT suspended AND cpu > 0",
    )
    .bind(node_uuid)
    .fetch_all(state.database.read())
    .await?
    .iter()
    .map(|row| -> Result<(uuid::Uuid, i32), sqlx::Error> {
        Ok((row.try_get("uuid")?, row.try_get("cpu")?))
    })
    .collect::<Result<_, _>>()?;

    let now = chrono::Utc::now().timestamp();
    for (server_uuid, limit) in limits {
        let Some(usage) = utilization.get(&server_uuid) else {
            continue;
        };
        let cpu = usage.cpu_absolute;
        let threshold = limit as f64 * percent / 100.0;
        let key = format!("abuse::cpu::{server_uuid}");

        let streak: Option<CpuStreak> = state.cache.get(&key).await.ok().flatten();
        if cpu < threshold {
            // A little below the line keeps the streak (a miner wobbles); well below ends it.
            if streak.is_some() && cpu < threshold * 0.75 {
                let _ = state.cache.invalidate(&key).await;
            }
            continue;
        }

        let mut streak = streak.unwrap_or(CpuStreak {
            since: now,
            ..Default::default()
        });
        streak.samples += 1;
        streak.sum += cpu;

        let console_due = now - streak.console_at >= CONSOLE_EVERY;
        let pinned_for = (now - streak.since) / 60;
        let full_due = pinned_for >= minutes && now - streak.full_at >= FULL_CHECK_EVERY;
        if console_due {
            streak.console_at = now;
        }
        if full_due {
            streak.full_at = now;
        }
        state.cache.set(&key, 3600, &streak).await?;

        if !console_due && pinned_for < minutes {
            continue;
        }

        let Some(server) = Server::by_uuid_optional(&state.database, server_uuid).await? else {
            continue;
        };
        if server.suspended {
            continue;
        }

        if console_due && let Some(lines) = console_lines(client, server.uuid).await {
            let findings = inspect::inspect_console(&lines);
            if !findings.is_empty() {
                record_and_sweep(state, &server, &findings).await?;
            }
        }

        if pinned_for >= minutes {
            let average = streak.sum / streak.samples.max(1) as f64;
            let finding = Finding {
                rule: "cpu.pinned".into(),
                category: "resources".into(),
                severity: Severity::Review,
                source: "cpu".into(),
                summary: format!(
                    "CPU at {}% of its {}% limit for {} minutes",
                    (average * 100.0 / limit as f64).round(),
                    limit,
                    pinned_for
                ),
                evidence: json!({
                    "average_cpu": (average * 10.0).round() / 10.0,
                    "limit": limit,
                    "minutes": pinned_for,
                    "samples": streak.samples,
                }),
            };
            record_and_sweep(state, &server, &[finding]).await?;

            if full_due {
                scan_server(state, &server, true, true).await?;
            }
        }
    }

    Ok(())
}

/// What runs on a node running the Wings fork. A stock Wings answers 404 and is skipped.
async fn watch_processes(
    state: &crate::State,
    node_uuid: uuid::Uuid,
    client: &wings_api::client::WingsClient,
) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;

    let report = tokio::time::timeout(WATCH_TIMEOUT, client.get_system_processes()).await??;

    for container in &report.containers {
        let Some(server_uuid) = container.server else {
            continue;
        };
        let findings = inspect::inspect_container(container);
        if findings.is_empty() {
            continue;
        }

        let Some(server) = Server::by_uuid_optional(&state.database, server_uuid).await? else {
            continue;
        };
        if server.node.uuid != node_uuid {
            continue;
        }
        record_and_sweep(state, &server, &findings).await?;
    }

    let host = inspect::inspect_host(&report.host_processes);
    if !host.is_empty() {
        record(state, Target::Node(node_uuid), &host, true).await?;
    }

    Ok(())
}

// --- Admin -----------------------------------------------------------------------------------

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiAbuseFindingServer {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub owner_username: Option<String>,
    pub egg_name: Option<String>,
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiAbuseFindingNode {
    pub uuid: uuid::Uuid,
    pub name: String,
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct AdminApiAbuseFinding {
    pub uuid: uuid::Uuid,
    pub server: Option<AdminApiAbuseFindingServer>,
    pub node: Option<AdminApiAbuseFindingNode>,
    pub rule: String,
    pub category: String,
    pub category_label: String,
    pub severity: String,
    pub source: String,
    /// `open`, `actioned` (the server was suspended for it), `dismissed` (a false positive) or
    /// `resolved`.
    pub status: String,
    pub summary: String,
    pub evidence: Value,
    pub hits: i32,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub action: Option<String>,
    pub actioned: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved_by: Option<uuid::Uuid>,
    pub resolved: Option<chrono::DateTime<chrono::Utc>>,
}

/// Findings, newest first; `status` `None` for every status, `search` in the summary, the server
/// or node name.
pub async fn findings(
    database: &crate::database::Database,
    status: Option<&str>,
    search: Option<&str>,
    page: i64,
    per_page: i64,
) -> Result<crate::models::Pagination<AdminApiAbuseFinding>, anyhow::Error> {
    let rows = sqlx::query(
        r#"
        SELECT abuse_findings.*, COUNT(*) OVER () AS total_count,
            servers.name AS server_name, users.username AS owner_username, nest_eggs.name AS egg_name,
            nodes.name AS node_name
        FROM abuse_findings
        LEFT JOIN servers ON servers.uuid = abuse_findings.server_uuid
        LEFT JOIN users ON users.uuid = servers.owner_uuid
        LEFT JOIN nest_eggs ON nest_eggs.uuid = servers.egg_uuid
        LEFT JOIN nodes ON nodes.uuid = abuse_findings.node_uuid
        WHERE ($1::text IS NULL OR abuse_findings.status = $1)
            AND ($2::text IS NULL OR abuse_findings.summary ILIKE '%' || $2 || '%'
                OR servers.name ILIKE '%' || $2 || '%' OR nodes.name ILIKE '%' || $2 || '%'
                OR users.username ILIKE '%' || $2 || '%')
        ORDER BY abuse_findings.last_seen DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(status)
    .bind(search)
    .bind(per_page)
    .bind((page - 1) * per_page)
    .fetch_all(database.read())
    .await?;

    let total = rows
        .first()
        .map(|row| row.try_get::<i64, _>("total_count"))
        .transpose()?
        .unwrap_or(0);

    let data = rows
        .iter()
        .map(|row| -> Result<AdminApiAbuseFinding, sqlx::Error> {
            let utc = |column: &str| -> Result<Option<chrono::DateTime<chrono::Utc>>, sqlx::Error> {
                Ok(row
                    .try_get::<Option<chrono::NaiveDateTime>, _>(column)?
                    .map(|at| at.and_utc()))
            };
            let category: String = row.try_get("category")?;
            let server_uuid: Option<uuid::Uuid> = row.try_get("server_uuid")?;
            let node_uuid: Option<uuid::Uuid> = row.try_get("node_uuid")?;
            let server_name: Option<String> = row.try_get("server_name")?;
            let node_name: Option<String> = row.try_get("node_name")?;

            Ok(AdminApiAbuseFinding {
                uuid: row.try_get("uuid")?,
                server: server_uuid.zip(server_name).map(|(uuid, name)| {
                    AdminApiAbuseFindingServer {
                        uuid,
                        name,
                        owner_username: row.try_get("owner_username").ok().flatten(),
                        egg_name: row.try_get("egg_name").ok().flatten(),
                    }
                }),
                node: node_uuid
                    .zip(node_name)
                    .map(|(uuid, name)| AdminApiAbuseFindingNode { uuid, name }),
                rule: row.try_get("rule")?,
                category_label: signatures::category_label(&category),
                category,
                severity: row.try_get("severity")?,
                source: row.try_get("source")?,
                status: row.try_get("status")?,
                summary: row.try_get("summary")?,
                evidence: row.try_get("evidence")?,
                hits: row.try_get("hits")?,
                first_seen: row
                    .try_get::<chrono::NaiveDateTime, _>("first_seen")?
                    .and_utc(),
                last_seen: row
                    .try_get::<chrono::NaiveDateTime, _>("last_seen")?
                    .and_utc(),
                action: row.try_get("action")?,
                actioned: utc("actioned")?,
                resolved_by: row.try_get("resolved_by")?,
                resolved: utc("resolved")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    Ok(crate::models::Pagination {
        total,
        per_page,
        page,
        data,
    })
}

/// Marks a finding `dismissed` (a false positive: it stays dismissed when seen again, and the
/// server's abuse suspension no longer deletes it) or `resolved`. Returns the finding's server,
/// `None` when there is no such finding.
pub async fn close_finding(
    database: &crate::database::Database,
    finding_uuid: uuid::Uuid,
    status: &str,
    by: uuid::Uuid,
) -> Result<Option<Option<uuid::Uuid>>, anyhow::Error> {
    let mut transaction = database.write().begin().await?;

    let server_uuid: Option<Option<uuid::Uuid>> = sqlx::query_scalar(
        r#"
        UPDATE abuse_findings SET status = $2, resolved_by = $3, resolved = NOW()
        WHERE uuid = $1
        RETURNING server_uuid
        "#,
    )
    .bind(finding_uuid)
    .bind(status)
    .bind(by)
    .fetch_optional(&mut *transaction)
    .await?;

    if status == STATUS_DISMISSED
        && let Some(Some(server_uuid)) = server_uuid
    {
        sqlx::query(
            r#"
            UPDATE servers SET suspension_delete_at = NULL
            WHERE uuid = $1 AND suspended AND suspension_kind = 'abuse'
            "#,
        )
        .bind(server_uuid)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(server_uuid)
}

/// How many findings are open, for the admin sidebar and the health page.
pub async fn open_count(database: &crate::database::Database) -> Result<i64, anyhow::Error> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM abuse_findings WHERE status = 'open'")
            .fetch_one(database.read())
            .await?,
    )
}

/// The short id a server's page uses, shown in alerts.
fn short_id(server: &Server) -> String {
    format!("{:08x}", server.uuid_short as u32)
}
