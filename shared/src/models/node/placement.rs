//! plugged.host: what a node is for and how it is labelled: `nodes.role`, `nodes.tags`,
//! `nodes.srv_target` and `nodes.deployment_paused` (docs/features/node-telemetry.md), and the
//! automatic deployment pause (docs/features/placement.md).

use crate::settings::placement::AppSettingsPlacement;
use sqlx::Row;

/// The roles a node can have. A `game` node takes players' servers; an `internal` node is only
/// used by admins (tests, builds, a node being set up) and is never picked for players' servers.
pub const ROLES: &[&str] = &["game", "internal"];

pub const MAX_TAGS: usize = 32;
pub const MAX_TAG_LENGTH: usize = 64;

pub fn default_role() -> compact_str::CompactString {
    "game".into()
}

pub fn validate_role(role: &compact_str::CompactString, _context: &()) -> garde::Result {
    if ROLES.contains(&role.as_str()) {
        Ok(())
    } else {
        Err(garde::Error::new(compact_str::format_compact!(
            "the role must be one of {}",
            ROLES.join(", ")
        )))
    }
}

pub fn validate_optional_role(
    role: &Option<compact_str::CompactString>,
    context: &(),
) -> garde::Result {
    role.as_ref()
        .map_or(Ok(()), |role| validate_role(role, context))
}

/// Tags are short labels (`nvme`, `ddos-protected`): at most [`MAX_TAGS`], each 1 to
/// [`MAX_TAG_LENGTH`] characters of letters, digits, `-`, `_` and `.`.
pub fn validate_tags(tags: &[compact_str::CompactString], _context: &()) -> garde::Result {
    if tags.len() > MAX_TAGS {
        return Err(garde::Error::new(compact_str::format_compact!(
            "a node cannot have more than {MAX_TAGS} tags"
        )));
    }

    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty()
            || tag.chars().count() > MAX_TAG_LENGTH
            || !tag
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(garde::Error::new(compact_str::format_compact!(
                "tags are 1 to {MAX_TAG_LENGTH} letters, digits, dashes, underscores or dots"
            )));
        }
    }

    Ok(())
}

pub fn validate_optional_tags(
    tags: &Option<Vec<compact_str::CompactString>>,
    context: &(),
) -> garde::Result {
    tags.as_ref()
        .map_or(Ok(()), |tags| validate_tags(tags, context))
}

/// Tags as stored: trimmed, lower case, sorted, without duplicates.
pub fn normalize_tags(tags: &[compact_str::CompactString]) -> Vec<String> {
    let mut tags: Vec<String> = tags.iter().map(|tag| tag.trim().to_lowercase()).collect();
    tags.sort();
    tags.dedup();

    tags
}

/// Why the panel paused a node, in `nodes.deployment_paused_reason`.
pub const PAUSED_FOR_MEMORY: &str = "memory";
pub const PAUSED_FOR_CPU: &str = "cpu";
pub const PAUSED_UNREACHABLE: &str = "unreachable";

/// What the deployment pause does with a node this minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseDecision {
    Keep,
    Pause(&'static str),
    Open,
}

/// A node as the deployment pause sees it: its pause and its newest telemetry.
#[derive(Debug, Clone, Default)]
pub struct PauseReading<'a> {
    pub paused: bool,
    /// `None` on a node an admin paused.
    pub reason: Option<&'a str>,
    /// There is a sample from the last few minutes.
    pub fresh: bool,
    pub reachable: bool,
    pub consecutive_failures: i32,
    pub cpu_percent: Option<f32>,
    pub memory_used: Option<i64>,
    pub memory_total: Option<i64>,
    pub threads: Option<i32>,
    /// The higher of memory and CPU use, averaged over the last minutes.
    pub pressure: Option<f32>,
}

/// The line in percent: lower on a small node. Unknown sizes count as big.
pub fn pause_line(
    settings: &AppSettingsPlacement,
    memory_total: Option<i64>,
    threads: Option<i32>,
) -> f32 {
    let small_memory = settings.small_memory_gb as i64 * 1024 * 1024 * 1024;
    let small = memory_total.is_some_and(|total| total > 0 && total < small_memory)
        || threads.is_some_and(|threads| threads > 0 && threads < settings.small_threads);

    let offset = if small { settings.small_offset } else { 0 };

    ((settings.pause_percent - offset) as f32).max(1.0)
}

/// Pauses a node over the line (memory first, then CPU) or one that stopped answering, and reopens
/// it once it is under the line and its recent average is too. A node an admin paused is theirs.
pub fn decide(settings: &AppSettingsPlacement, reading: &PauseReading) -> PauseDecision {
    let ours = reading.paused && reading.reason.is_some();

    if !settings.pause_enabled {
        return if ours {
            PauseDecision::Open
        } else {
            PauseDecision::Keep
        };
    }
    if (reading.paused && !ours) || !reading.fresh {
        return PauseDecision::Keep;
    }

    if !reading.reachable {
        return if !reading.paused && reading.consecutive_failures >= settings.unreachable_after {
            PauseDecision::Pause(PAUSED_UNREACHABLE)
        } else {
            PauseDecision::Keep
        };
    }

    let line = pause_line(settings, reading.memory_total, reading.threads);
    let memory = match (reading.memory_used, reading.memory_total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 * 100.0 / total as f32),
        _ => None,
    };

    let over = if memory.is_some_and(|memory| memory > line) {
        Some(PAUSED_FOR_MEMORY)
    } else if reading.cpu_percent.is_some_and(|cpu| cpu > line) {
        Some(PAUSED_FOR_CPU)
    } else {
        None
    };

    match over {
        Some(reason) if reading.paused && reading.reason == Some(reason) => PauseDecision::Keep,
        Some(reason) => PauseDecision::Pause(reason),
        None if !reading.paused => PauseDecision::Keep,
        None if reading.reason == Some(PAUSED_UNREACHABLE) => PauseDecision::Open,
        None if reading.pressure.is_some_and(|pressure| pressure > line) => PauseDecision::Keep,
        None => PauseDecision::Open,
    }
}

/// The deployment pause, right after each telemetry sample (the `sample_nodes` task).
pub async fn pause_overloaded(state: &crate::State) -> Result<(), anyhow::Error> {
    let settings = state.settings.get().await?.placement.clone();

    let rows = sqlx::query(
        r#"
        SELECT nodes.uuid, nodes.name, nodes.deployment_paused, nodes.deployment_paused_reason,
            COALESCE(telemetry.sampled > NOW() - INTERVAL '3 minutes', FALSE) AS fresh,
            COALESCE(telemetry.reachable, FALSE) AS reachable,
            COALESCE(telemetry.consecutive_failures, 0) AS consecutive_failures,
            telemetry.cpu_percent, telemetry.memory_used, telemetry.memory_total,
            telemetry.cpu_threads, telemetry.pressure
        FROM nodes
        LEFT JOIN node_telemetry telemetry ON telemetry.node_uuid = nodes.uuid
        "#,
    )
    .fetch_all(state.database.write())
    .await?;

    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let name: String = row.try_get("name")?;
        let reason: Option<String> = row.try_get("deployment_paused_reason")?;

        let reading = PauseReading {
            paused: row.try_get("deployment_paused")?,
            reason: reason.as_deref(),
            fresh: row.try_get("fresh")?,
            reachable: row.try_get("reachable")?,
            consecutive_failures: row.try_get("consecutive_failures")?,
            cpu_percent: row.try_get("cpu_percent")?,
            memory_used: row.try_get("memory_used")?,
            memory_total: row.try_get("memory_total")?,
            threads: row.try_get("cpu_threads")?,
            pressure: row.try_get("pressure")?,
        };

        let changed = match decide(&settings, &reading) {
            PauseDecision::Keep => continue,
            PauseDecision::Pause(reason) => {
                tracing::info!(node = %name, reason, "pausing deployment on a busy node");

                // an admin's pause in the meantime stays theirs
                sqlx::query(
                    "UPDATE nodes SET deployment_paused = TRUE, deployment_paused_reason = $2
                    WHERE uuid = $1 AND (NOT deployment_paused OR deployment_paused_reason IS NOT NULL)",
                )
                .bind(uuid)
                .bind(reason)
                .execute(state.database.write())
                .await?
            }
            PauseDecision::Open => {
                tracing::info!(node = %name, "reopening deployment");

                sqlx::query(
                    "UPDATE nodes SET deployment_paused = FALSE, deployment_paused_reason = NULL
                    WHERE uuid = $1 AND deployment_paused_reason IS NOT NULL",
                )
                .bind(uuid)
                .execute(state.database.write())
                .await?
            }
        };

        if changed.rows_affected() > 0 {
            state.cache.invalidate(&format!("node::{uuid}")).await.ok();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> Vec<compact_str::CompactString> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn roles() {
        assert!(validate_role(&"game".into(), &()).is_ok());
        assert!(validate_role(&"internal".into(), &()).is_ok());
        assert!(validate_role(&"storage".into(), &()).is_err());
        assert!(validate_optional_role(&None, &()).is_ok());
    }

    #[test]
    fn tag_rules() {
        assert!(validate_tags(&tags(&["nvme", "ddos-protected", "eu_west.2"]), &()).is_ok());
        assert!(validate_tags(&tags(&[""]), &()).is_err());
        assert!(validate_tags(&tags(&["two words"]), &()).is_err());
        assert!(validate_tags(&tags(&["a,b"]), &()).is_err());
        assert!(validate_tags(&tags(&[&"x".repeat(65)]), &()).is_err());
        assert!(validate_tags(&vec!["t".into(); 33], &()).is_err());
    }

    #[test]
    fn tags_are_normalized() {
        assert_eq!(
            normalize_tags(&tags(&[" NVMe", "ssd", "nvme", "ssd "])),
            ["nvme", "ssd"]
        );
    }

    fn reading(paused: bool, reason: Option<&'static str>) -> PauseReading<'static> {
        PauseReading {
            paused,
            reason,
            fresh: true,
            reachable: true,
            consecutive_failures: 0,
            cpu_percent: Some(20.0),
            memory_used: Some(50),
            memory_total: Some(64 * 1024 * 1024 * 1024),
            threads: Some(16),
            pressure: Some(20.0),
        }
    }

    #[test]
    fn small_nodes_have_a_lower_line() {
        let settings = AppSettingsPlacement::default();
        let gib = 1024 * 1024 * 1024;

        assert_eq!(pause_line(&settings, Some(64 * gib), Some(16)), 85.0);
        assert_eq!(pause_line(&settings, Some(32 * gib), Some(16)), 75.0);
        assert_eq!(pause_line(&settings, Some(64 * gib), Some(8)), 75.0);
        assert_eq!(pause_line(&settings, None, None), 85.0);
    }

    #[test]
    fn a_busy_node_is_paused_and_reopens_on_its_average() {
        let settings = AppSettingsPlacement::default();

        let mut busy = reading(false, None);
        busy.cpu_percent = Some(90.0);
        assert_eq!(
            decide(&settings, &busy),
            PauseDecision::Pause(PAUSED_FOR_CPU)
        );

        let mut full = reading(false, None);
        full.memory_used = Some(full.memory_total.unwrap() / 10 * 9);
        full.cpu_percent = Some(99.0);
        assert_eq!(
            decide(&settings, &full),
            PauseDecision::Pause(PAUSED_FOR_MEMORY)
        );

        // under the line now, but the last minutes were not
        let mut calmer = reading(true, Some(PAUSED_FOR_CPU));
        calmer.pressure = Some(88.0);
        assert_eq!(decide(&settings, &calmer), PauseDecision::Keep);

        calmer.pressure = Some(60.0);
        assert_eq!(decide(&settings, &calmer), PauseDecision::Open);

        busy.paused = true;
        busy.reason = Some(PAUSED_FOR_CPU);
        assert_eq!(decide(&settings, &busy), PauseDecision::Keep);
    }

    #[test]
    fn unreachable_nodes_pause_after_some_misses() {
        let settings = AppSettingsPlacement::default();

        let mut gone = reading(false, None);
        gone.reachable = false;
        gone.consecutive_failures = 2;
        assert_eq!(decide(&settings, &gone), PauseDecision::Keep);

        gone.consecutive_failures = 3;
        assert_eq!(
            decide(&settings, &gone),
            PauseDecision::Pause(PAUSED_UNREACHABLE)
        );

        assert_eq!(
            decide(&settings, &reading(true, Some(PAUSED_UNREACHABLE))),
            PauseDecision::Open
        );
    }

    #[test]
    fn an_admins_pause_and_missing_samples_are_left_alone() {
        let mut settings = AppSettingsPlacement::default();

        assert_eq!(decide(&settings, &reading(true, None)), PauseDecision::Keep);

        let mut stale = reading(true, Some(PAUSED_FOR_MEMORY));
        stale.fresh = false;
        assert_eq!(decide(&settings, &stale), PauseDecision::Keep);

        settings.pause_enabled = false;
        assert_eq!(decide(&settings, &stale), PauseDecision::Open);
        assert_eq!(decide(&settings, &reading(true, None)), PauseDecision::Keep);
    }
}
