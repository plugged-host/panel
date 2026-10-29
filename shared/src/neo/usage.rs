//! plugged.host: neo's two usage windows while the panel meters neo itself (a port of
//! noprofit.space's NeoUsageService, on `neo_usage_windows` / `neo_usage_events`). Usage is input
//! plus output tokens, times the answering model's multiplier.
//!
//! Windows are anchored: one opens at the first charged turn while none is active and resets at
//! its start plus its length. The limit is checked before a turn and charged after it, so one
//! turn may overshoot. A Discord account nobody linked has windows of its own at the settings'
//! share of the limits.
//!
//! plugged.host: once a window refuses turns, a person may buy one more day's worth of usage with
//! tokens ([`buy_extra`]): it is kept on each active window as `bonus_tokens`, on top of its
//! limit, until the window resets.

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

/// A window with the usage bought for it on top of its limit (only while it is active: a closed
/// window starts again without it).
pub fn with_bonus(mut window: Window, bonus: i64) -> Window {
    if window.active && bonus > 0 {
        window.limit += bonus;
        window.percent = if window.limit > 0 {
            (window.used * 100 / window.limit).min(100)
        } else {
            100
        };
    }

    window
}

/// One day's worth of usage: the weekly limit (at the subject's share) spread over its days.
pub fn day_worth(settings: &AppSettingsNeo, subject: &Subject) -> i64 {
    limit(settings, LONG, subject) / settings.long_days.max(1) as i64
}

/// A window's bonus after one more day's worth is bought: the day on top of what is used now, so
/// a turn that overshot the limit does not eat into it.
pub fn bonus_after(limit: i64, bonus: i64, used: i64, day: i64) -> i64 {
    bonus + (used - (limit + bonus)).max(0) + day.max(0)
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
        "SELECT kind, started, resets, tokens_used, bonus_tokens FROM neo_usage_windows WHERE {SUBJECT}"
    )))
    .bind(subject.user_uuid)
    .bind(&subject.discord_id)
    .fetch_all(database.write())
    .await?;

    Ok(limits_of(
        settings,
        subject,
        &rows,
        chrono::Utc::now().naive_utc(),
    ))
}

/// The windows from their rows (`kind`, `started`, `resets`, `tokens_used`, `bonus_tokens`).
fn limits_of(
    settings: &AppSettingsNeo,
    subject: &Subject,
    rows: &[sqlx::postgres::PgRow],
    now: chrono::NaiveDateTime,
) -> Limits {
    let mut limits = Limits::new();
    for kind in [SHORT, LONG] {
        let row = rows.iter().find(|row| row.get::<String, _>("kind") == kind);
        let window = describe(
            settings,
            kind,
            subject,
            row.map(|row| {
                (
                    row.get::<chrono::NaiveDateTime, _>("started"),
                    row.get::<chrono::NaiveDateTime, _>("resets"),
                    row.get::<i64, _>("tokens_used"),
                )
            }),
            now,
        );
        let bonus = row.map_or(0, |row| row.get::<i64, _>("bonus_tokens"));
        limits.insert(kind.into(), with_bonus(window, bonus));
    }

    limits
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
                    "UPDATE neo_usage_windows SET started = $2, resets = $3, tokens_used = $4,
                        bonus_tokens = 0
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

/// The token ledger's type for usage bought in the chat.
pub const TYPE_NEO_EXTRA: &str = "neo_extra";

/// Why one more day's worth could not be bought.
#[derive(Debug)]
pub enum ExtraError {
    /// Turned off in the settings.
    Disabled,
    /// Not sold right now: tokens off, noprofit.space runs neo, or the limits leave no day.
    Unavailable,
    /// No window refuses turns, so there is nothing to raise.
    NotLimited,
    /// The price moved since the person saw it: the price now.
    PriceChanged(i64),
    /// The balance is short.
    Short,
    Other(anyhow::Error),
}

impl ExtraError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "this is turned off".into(),
            Self::Unavailable => "this is not available".into(),
            Self::NotLimited => "you have not reached your limit".into(),
            Self::PriceChanged(price) => format!("the price changed, it is {price} tokens now"),
            Self::Short => "your balance is too low".into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled => 403,
            Self::Other(_) => 500,
            _ => 409,
        }
    }
}

impl From<sqlx::Error> for ExtraError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<anyhow::Error> for ExtraError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

/// What a purchase did: the tokens it cost, the usage it added per active window, and the windows
/// as they are now.
#[derive(Debug, Clone)]
pub struct ExtraBought {
    pub cost: i64,
    pub day: i64,
    pub limits: Limits,
}

/// Buys one more day's worth of usage for a panel user whose windows refuse turns: the tokens and
/// every active window's bonus change in one transaction, with the user row and the windows
/// locked. `seen` is the price the chat showed.
pub async fn buy_extra(
    database: &crate::database::Database,
    settings: &AppSettingsNeo,
    subject: &Subject,
    cost: i64,
    seen: Option<i64>,
) -> Result<ExtraBought, ExtraError> {
    let Some(user_uuid) = subject.user_uuid else {
        return Err(ExtraError::Unavailable);
    };
    if seen.is_some_and(|seen| seen != cost) {
        return Err(ExtraError::PriceChanged(cost));
    }
    let day = day_worth(settings, subject);
    if day <= 0 || cost <= 0 {
        return Err(ExtraError::Unavailable);
    }

    let mut transaction = database.write().begin().await?;
    sqlx::query("SELECT 1 FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(user_uuid)
        .execute(&mut *transaction)
        .await?;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT uuid, kind, started, resets, tokens_used, bonus_tokens FROM neo_usage_windows
        WHERE {SUBJECT} FOR UPDATE"
    )))
    .bind(subject.user_uuid)
    .bind(&subject.discord_id)
    .fetch_all(&mut *transaction)
    .await?;

    let now = chrono::Utc::now().naive_utc();
    let Some(blocking) = blocking_window(&limits_of(settings, subject, &rows, now)) else {
        return Err(ExtraError::NotLimited);
    };

    if crate::economy::change(
        &mut transaction,
        crate::economy::Currency::Tokens,
        user_uuid,
        -cost,
        TYPE_NEO_EXTRA,
        "neo: one more day",
        serde_json::json!({ "window": blocking, "usage": day }),
    )
    .await?
    .is_none()
    {
        return Err(ExtraError::Short);
    }

    for row in rows
        .iter()
        .filter(|row| row.get::<chrono::NaiveDateTime, _>("resets") > now)
    {
        let kind: String = row.get("kind");
        let bonus = bonus_after(
            limit(settings, &kind, subject),
            row.get("bonus_tokens"),
            row.get("tokens_used"),
            day,
        );
        sqlx::query("UPDATE neo_usage_windows SET bonus_tokens = $2 WHERE uuid = $1")
            .bind(row.get::<uuid::Uuid, _>("uuid"))
            .bind(bonus)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;

    Ok(ExtraBought {
        cost,
        day,
        limits: snapshot(database, settings, subject).await?,
    })
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
    fn a_bought_day_is_headroom_from_where_the_window_stands() {
        let settings = AppSettingsNeo::default();
        let free = Subject::user(uuid::Uuid::nil(), 100);
        // 15 000 000 a week over 7 days
        assert_eq!(day_worth(&settings, &free), 2_142_857);

        // a turn overshot the limit by 20: the day comes on top of what is used
        assert_eq!(bonus_after(100, 0, 120, 30), 50);
        // at the limit, or bought again
        assert_eq!(bonus_after(100, 0, 100, 30), 30);
        assert_eq!(bonus_after(100, 30, 130, 30), 60);
        // a window under its limit keeps its room and gets the day too
        assert_eq!(bonus_after(100, 0, 40, 30), 30);

        let window = describe(
            &settings,
            SHORT,
            &free,
            Some((at(10), at(15), 3_000_000)),
            at(12),
        );
        let raised = with_bonus(window.clone(), 1_000_000);
        assert_eq!(raised.limit, 4_000_000);
        assert_eq!(raised.percent, 75);
        assert!(blocking_window(&Limits::from([(SHORT.to_string(), raised)])).is_none());

        // a closed window starts again without the bonus
        let closed = describe(&settings, SHORT, &free, Some((at(10), at(15), 9)), at(16));
        assert_eq!(with_bonus(closed, 1_000_000).limit, 3_000_000);
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
