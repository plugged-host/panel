//! plugged.host: neo's two usage windows while the panel meters neo itself (a port of
//! noprofit.space's NeoUsageService, on `neo_usage_windows` / `neo_usage_events`). Usage is input
//! plus output tokens, times the answering model's multiplier.
//!
//! Windows are anchored: one opens at the first charged turn while none is active and resets at
//! its start plus its length. The limit is checked before a turn and charged after it, so one
//! turn may overshoot. A Discord account nobody linked has windows of its own at the settings'
//! share of the limits.

use crate::settings::neo::AppSettingsNeo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::collections::BTreeMap;
use utoipa::ToSchema;

pub const SHORT: &str = "short";
pub const LONG: &str = "long";

/// Whose windows a turn uses.
#[derive(Debug, Clone)]
pub struct Subject {
    pub user_uuid: Option<uuid::Uuid>,
    pub discord_id: Option<String>,
    /// The share of the limits, in % (memberships raise it).
    pub usage_percent: i64,
}

impl Subject {
    pub fn user(user_uuid: uuid::Uuid, usage_percent: i64) -> Self {
        Self {
            user_uuid: Some(user_uuid),
            discord_id: None,
            usage_percent: usage_percent.max(100),
        }
    }

    pub fn unlinked_discord(discord_id: &str) -> Self {
        Self {
            user_uuid: None,
            discord_id: Some(discord_id.to_string()),
            usage_percent: 100,
        }
    }

    pub fn is_unlinked_discord(&self) -> bool {
        self.user_uuid.is_none()
    }

    pub fn lock_key(&self) -> String {
        match (self.user_uuid, &self.discord_id) {
            (Some(uuid), _) => format!("user::{uuid}"),
            (None, Some(discord_id)) => format!("discord::{discord_id}"),
            (None, None) => "nobody".into(),
        }
    }
}

/// One window as the chat shows it (noprofit.space's shape).
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Window {
    pub used: i64,
    pub limit: i64,
    pub percent: i64,
    pub active: bool,
    pub started_at: Option<String>,
    pub resets_at: Option<String>,
    pub window_seconds: i64,
}

/// The windows by kind: `short` and `long`, and from noprofit.space possibly more.
pub type Limits = BTreeMap<String, Window>;

pub fn length_seconds(settings: &AppSettingsNeo, kind: &str) -> i64 {
    if kind == SHORT {
        settings.short_hours.max(1) as i64 * 3600
    } else {
        settings.long_days.max(1) as i64 * 86_400
    }
}

pub fn unlinked_discord_percent(settings: &AppSettingsNeo) -> i64 {
    settings.discord_unlinked_percent.clamp(0, 100) as i64
}

pub fn limit(settings: &AppSettingsNeo, kind: &str, subject: &Subject) -> i64 {
    let limit = if kind == SHORT {
        settings.short_tokens
    } else {
        settings.long_tokens
    }
    .max(0);

    if subject.is_unlinked_discord() {
        limit * unlinked_discord_percent(settings) / 100
    } else {
        limit * subject.usage_percent / 100
    }
}

/// A window from its row (`started`, `resets`, `tokens_used`), or a closed one.
pub fn describe(
    settings: &AppSettingsNeo,
    kind: &str,
    subject: &Subject,
    row: Option<(chrono::NaiveDateTime, chrono::NaiveDateTime, i64)>,
    now: chrono::NaiveDateTime,
) -> Window {
    let limit = limit(settings, kind, subject);
    let active = row.filter(|(_, resets, _)| *resets > now);
    let used = active.map_or(0, |(_, _, used)| used);

    Window {
        used,
        limit,
        percent: if limit > 0 {
            (used * 100 / limit).min(100)
        } else {
            100
        },
        active: active.is_some(),
        started_at: active.map(|(started, _, _)| iso(started)),
        resets_at: active.map(|(_, resets, _)| iso(resets)),
        window_seconds: length_seconds(settings, kind),
    }
}

fn iso(time: chrono::NaiveDateTime) -> String {
    time.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The window that refuses new turns, if any (the weekly one first).
pub fn blocking_window(limits: &Limits) -> Option<String> {
    [LONG, SHORT].into_iter().find_map(|kind| {
        limits
            .get(kind)
            .filter(|window| window.used >= window.limit)
            .map(|_| kind.to_string())
    })
}

/// Matches the subject's windows: `$1` the user, else `$2` the Discord account.
const SUBJECT: &str = "(($1::uuid IS NOT NULL AND user_uuid = $1)
    OR ($1::uuid IS NULL AND user_uuid IS NULL AND discord_id = $2))";

pub async fn snapshot(
    database: &crate::database::Database,
    settings: &AppSettingsNeo,
    subject: &Subject,
) -> Result<Limits, sqlx::Error> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT kind, started, resets, tokens_used FROM neo_usage_windows WHERE {SUBJECT}"
    )))
    .bind(subject.user_uuid)
    .bind(&subject.discord_id)
    .fetch_all(database.write())
    .await?;

    let now = chrono::Utc::now().naive_utc();
    let mut limits = Limits::new();
    for kind in [SHORT, LONG] {
        let row = rows
            .iter()
            .find(|row| row.get::<String, _>("kind") == kind)
            .map(|row| {
                (
                    row.get::<chrono::NaiveDateTime, _>("started"),
                    row.get::<chrono::NaiveDateTime, _>("resets"),
                    row.get::<i64, _>("tokens_used"),
                )
            });
        limits.insert(kind.into(), describe(settings, kind, subject, row, now));
    }

    Ok(limits)
}

/// What one turn did, for the ledger.
#[derive(Debug, Clone, Default)]
pub struct Event {
    pub service: String,
    pub conversation: Option<String>,
    pub provider_slot: Option<i16>,
    pub provider_label: Option<String>,
    pub driver: Option<String>,
    pub model: Option<String>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub finish_reason: Option<String>,
    pub attempts: Option<Value>,
    pub duration_ms: i64,
}

/// Charges a turn to both windows and writes its ledger row, at once.
pub async fn charge(
    database: &crate::database::Database,
    settings: &AppSettingsNeo,
    subject: &Subject,
    tokens: i64,
    event: &Event,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().naive_utc();
    let mut transaction = database.write().begin().await?;

    for kind in [SHORT, LONG] {
        let window = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT uuid, resets FROM neo_usage_windows WHERE {SUBJECT} AND kind = $3 FOR UPDATE"
        )))
        .bind(subject.user_uuid)
        .bind(&subject.discord_id)
        .bind(kind)
        .fetch_optional(&mut *transaction)
        .await?;
        let resets = now + chrono::Duration::seconds(length_seconds(settings, kind));

        match window {
            None => {
                sqlx::query(
                    "INSERT INTO neo_usage_windows (user_uuid, discord_id, kind, started, resets, tokens_used)
                    VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(subject.user_uuid)
                .bind(
                    subject
                        .user_uuid
                        .is_none()
                        .then_some(subject.discord_id.as_deref())
                        .flatten(),
                )
                .bind(kind)
                .bind(now)
                .bind(resets)
                .bind(tokens)
                .execute(&mut *transaction)
                .await?;
            }
            Some(row) if row.get::<chrono::NaiveDateTime, _>("resets") <= now => {
                sqlx::query(
                    "UPDATE neo_usage_windows SET started = $2, resets = $3, tokens_used = $4
                    WHERE uuid = $1",
                )
                .bind(row.get::<uuid::Uuid, _>("uuid"))
                .bind(now)
                .bind(resets)
                .bind(tokens)
                .execute(&mut *transaction)
                .await?;
            }
            Some(row) => {
                sqlx::query(
                    "UPDATE neo_usage_windows SET tokens_used = tokens_used + $2 WHERE uuid = $1",
                )
                .bind(row.get::<uuid::Uuid, _>("uuid"))
                .bind(tokens)
                .execute(&mut *transaction)
                .await?;
            }
        }
    }

    insert_event(&mut *transaction, subject, tokens, "ok", event).await?;
    transaction.commit().await
}

/// Records a failed turn (or a provider test): nothing is charged, no window opens.
pub async fn record(
    database: &crate::database::Database,
    subject: Option<&Subject>,
    status: &str,
    event: &Event,
) -> Result<(), sqlx::Error> {
    let nobody = Subject {
        user_uuid: None,
        discord_id: None,
        usage_percent: 100,
    };

    insert_event(
        database.write(),
        subject.unwrap_or(&nobody),
        0,
        status,
        event,
    )
    .await
}

async fn insert_event<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    subject: &Subject,
    tokens: i64,
    status: &str,
    event: &Event,
) -> Result<(), sqlx::Error> {
    fn cut(value: &Option<String>, max: usize) -> Option<String> {
        value
            .as_ref()
            .map(|value| value.chars().take(max).collect::<String>())
    }

    sqlx::query(
        "INSERT INTO neo_usage_events (user_uuid, discord_id, service, conversation, provider_slot,
            provider_label, driver, model, prompt_tokens, completion_tokens, charged_tokens,
            finish_reason, status, attempts, duration_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(subject.user_uuid)
    .bind(&subject.discord_id)
    .bind(event.service.chars().take(32).collect::<String>())
    .bind(cut(&event.conversation, 64))
    .bind(event.provider_slot)
    .bind(cut(&event.provider_label, 64))
    .bind(cut(&event.driver, 16))
    .bind(cut(&event.model, 128))
    .bind(event.prompt_tokens.clamp(0, i32::MAX as i64) as i32)
    .bind(event.completion_tokens.clamp(0, i32::MAX as i64) as i32)
    .bind(tokens.max(0))
    .bind(cut(&event.finish_reason, 24))
    .bind(status)
    .bind(&event.attempts)
    .bind(event.duration_ms.clamp(0, i32::MAX as i64) as i32)
    .execute(executor)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn windows_scale_with_the_plan_and_close_when_due() {
        let settings = AppSettingsNeo::default();
        let plus = Subject::user(uuid::Uuid::nil(), 125);
        let stranger = Subject::unlinked_discord("123456789012345678");

        assert_eq!(limit(&settings, SHORT, &plus), 3_750_000);
        assert_eq!(limit(&settings, LONG, &stranger), 7_500_000);

        let open = describe(
            &settings,
            SHORT,
            &plus,
            Some((at(10), at(15), 1_875_000)),
            at(12),
        );
        assert!(open.active);
        assert_eq!(open.percent, 50);
        assert_eq!(open.resets_at.as_deref(), Some("2026-10-24T15:00:00Z"));

        let closed = describe(&settings, SHORT, &plus, Some((at(10), at(15), 9)), at(15));
        assert!(!closed.active);
        assert_eq!(closed.used, 0);
    }

    #[test]
    fn the_weekly_window_blocks_first() {
        let full = Window {
            used: 10,
            limit: 10,
            ..Default::default()
        };
        let mut limits = Limits::from([
            (SHORT.to_string(), full.clone()),
            (
                LONG.to_string(),
                Window {
                    limit: 10,
                    ..Default::default()
                },
            ),
        ]);
        assert_eq!(blocking_window(&limits).as_deref(), Some(SHORT));
        limits.insert(LONG.into(), full);
        assert_eq!(blocking_window(&limits).as_deref(), Some(LONG));
        limits.clear();
        assert_eq!(blocking_window(&limits), None);
    }
}
