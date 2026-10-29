//! plugged.host: the daily streak on the Tokens page (docs/features/economy.md#daily-streak).
//!
//! An account claims once a UTC day, with a button. A claim the day after the last one moves the
//! streak a day on; a missed day starts it again at day 1. Each day pays what Admin › Settings ›
//! Economy › Tokens lists for it (0.25, 0.25, 0.5, 0.5, 0.75, 0.75 and 1 by default; the last one
//! repeats once the list ends), through the account's saved part like a link's fractional reward
//! ([`crate::earn::split_reward`]), so 0.25 adds a quarter token toward the next whole one.

use crate::{
    economy::{Currency, EconomyError, change},
    models::user::User,
};
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;

/// What each day pays by default, in tokens.
pub const DEFAULT_REWARDS: &str = "0.25,0.25,0.5,0.5,0.75,0.75,1";
/// The ledger type of a streak's tokens.
pub const TYPE_STREAK: &str = "streak";
/// The most days the list may name; the last one repeats after it.
const MAX_REWARDS: usize = 14;

/// The settings' list in hundredths of a token (`"0.25,1"` -> `[25, 100]`), or `None` when it is
/// empty, longer than 14, or has an amount outside 0.01 to 100.
pub fn parse_rewards(text: &str) -> Option<Vec<i64>> {
    let rewards = text
        .split(',')
        .map(|value| {
            let tokens: f64 = value.trim().parse().ok()?;

            (tokens.is_finite() && (0.01..=100.0).contains(&tokens))
                .then(|| (tokens * 100.0).round() as i64)
        })
        .collect::<Option<Vec<i64>>>()?;

    (!rewards.is_empty() && rewards.len() <= MAX_REWARDS).then_some(rewards)
}

/// The list as the settings keep it (`[25, 100]` -> `"0.25,1"`).
pub fn rewards_text(rewards: &[i64]) -> String {
    rewards
        .iter()
        .map(|reward| crate::earn::tokens_text(*reward).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// What day `day` of a streak (1: the first) pays, in hundredths: past the list, its last one.
pub fn reward_on(rewards: &[i64], day: i64) -> i64 {
    let index = (day.max(1) - 1) as usize;

    rewards.get(index).or(rewards.last()).copied().unwrap_or(0)
}

/// The day a claim on `today` is: the next one after a claim yesterday, else day 1 (a missed
/// day, or never claimed). `None` while today is claimed already.
pub fn claim_day(
    last_day: i64,
    last_claimed: Option<chrono::NaiveDate>,
    today: chrono::NaiveDate,
) -> Option<i64> {
    match last_claimed {
        Some(claimed) if claimed >= today => None,
        Some(claimed) if claimed.succ_opt() == Some(today) => Some(last_day.max(0) + 1),
        _ => Some(1),
    }
}

/// The streak as the Tokens page shows it.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq)]
pub struct ApiStreak {
    /// The day a claim now is (while `claimable`), else the day claimed today.
    pub day: i64,
    /// What that day pays, in tokens (up to 2 decimals).
    pub reward: f64,
    /// What the day after it pays, if the streak goes on.
    pub next_reward: f64,
    /// Today's tokens wait to be claimed.
    pub claimable: bool,
}

/// The page's view of a streak at `today`.
pub fn status_of(
    rewards: &[i64],
    last_day: i64,
    last_claimed: Option<chrono::NaiveDate>,
    today: chrono::NaiveDate,
) -> ApiStreak {
    let (day, claimable) = match claim_day(last_day, last_claimed, today) {
        Some(day) => (day, true),
        None => (last_day.max(1), false),
    };

    ApiStreak {
        day,
        reward: reward_on(rewards, day) as f64 / 100.0,
        next_reward: reward_on(rewards, day + 1) as f64 / 100.0,
        claimable,
    }
}

/// Whether the streak is offered: tokens and the streak both on.
pub fn offered(economy: &crate::settings::economy::AppSettingsEconomy) -> bool {
    economy.tokens_enabled && economy.streak_enabled
}

/// The settings' list, or the default one should the stored text not read.
fn rewards(economy: &crate::settings::economy::AppSettingsEconomy) -> Vec<i64> {
    parse_rewards(&economy.streak_rewards)
        .or_else(|| parse_rewards(DEFAULT_REWARDS))
        .unwrap_or_default()
}

fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

/// The account's streak: the day last claimed and when.
async fn stored(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<(i64, Option<chrono::NaiveDate>), sqlx::Error> {
    let row = sqlx::query("SELECT streak_day, streak_claimed FROM users WHERE uuid = $1")
        .bind(user_uuid)
        .fetch_optional(database.write())
        .await?;

    match row {
        Some(row) => Ok((
            row.try_get::<i32, _>("streak_day")? as i64,
            row.try_get("streak_claimed")?,
        )),
        None => Ok((0, None)),
    }
}

/// The Tokens page's streak; `None` while it is off.
pub async fn status(
    state: &crate::State,
    economy: &crate::settings::economy::AppSettingsEconomy,
    user_uuid: uuid::Uuid,
) -> Result<Option<ApiStreak>, sqlx::Error> {
    if !offered(economy) {
        return Ok(None);
    }

    let (last_day, last_claimed) = stored(&state.database, user_uuid).await?;

    Ok(Some(status_of(
        &rewards(economy),
        last_day,
        last_claimed,
        today(),
    )))
}

/// Whether today's tokens wait to be claimed (the sidebar's dot). False while the streak is off.
pub async fn claimable(state: &crate::State, user_uuid: uuid::Uuid) -> Result<bool, anyhow::Error> {
    let on = offered(&state.settings.get().await?.economy);
    if !on {
        return Ok(false);
    }

    let (last_day, last_claimed) = stored(&state.database, user_uuid).await?;

    Ok(claim_day(last_day, last_claimed, today()).is_some())
}

/// What a claim paid.
#[derive(ToSchema, Serialize)]
pub struct ApiStreakClaim {
    /// The day of the streak it was.
    pub day: i64,
    /// What the day paid, in tokens; a part of a token is saved toward the next one.
    pub reward: f64,
    /// Whole tokens added now.
    pub amount: i64,
    pub streak: ApiStreak,
}

/// Claims today's tokens: the account row locked, so two clicks at once pay once.
pub async fn claim(state: &crate::State, user: &User) -> Result<ApiStreakClaim, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if !offered(&economy) {
        return Err(EconomyError::Disabled);
    }
    let rewards = rewards(&economy);
    let today = today();

    let mut transaction = state.database.write().begin().await?;
    let row = sqlx::query(
        "SELECT streak_day, streak_claimed, token_fraction FROM users WHERE uuid = $1 FOR UPDATE",
    )
    .bind(user.uuid)
    .fetch_one(&mut *transaction)
    .await?;
    let last_day = row.try_get::<i32, _>("streak_day")? as i64;
    let last_claimed: Option<chrono::NaiveDate> = row.try_get("streak_claimed")?;
    let saved = row.try_get::<i32, _>("token_fraction")? as i64;

    let Some(day) = claim_day(last_day, last_claimed, today) else {
        return Err(EconomyError::Conflict(
            "today's tokens are claimed already, come back tomorrow".into(),
        ));
    };
    let day = day.min(i32::MAX as i64);
    let reward = reward_on(&rewards, day);
    let (whole, rest) = crate::earn::split_reward(saved, reward);

    sqlx::query(
        "UPDATE users SET streak_day = $2, streak_claimed = $3, token_fraction = $4
        WHERE uuid = $1",
    )
    .bind(user.uuid)
    .bind(day as i32)
    .bind(today)
    .bind(rest as i32)
    .execute(&mut *transaction)
    .await?;

    if whole > 0 {
        change(
            &mut transaction,
            Currency::Tokens,
            user.uuid,
            whole,
            TYPE_STREAK,
            &format!("Day {day}"),
            serde_json::json!({ "day": day, "reward": reward }),
        )
        .await?;
    }
    transaction.commit().await?;

    Ok(ApiStreakClaim {
        day,
        reward: reward as f64 / 100.0,
        amount: whole,
        streak: status_of(&rewards, day, Some(today), today),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(day: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    #[test]
    fn the_rewards_read_in_hundredths() {
        assert_eq!(
            parse_rewards(DEFAULT_REWARDS),
            Some(vec![25, 25, 50, 50, 75, 75, 100])
        );
        assert_eq!(parse_rewards(" 1 , 2.5 "), Some(vec![100, 250]));
        assert_eq!(parse_rewards(""), None);
        assert_eq!(parse_rewards("1,,2"), None);
        assert_eq!(parse_rewards("0.001"), None);
        assert_eq!(parse_rewards("101"), None);
        assert_eq!(parse_rewards("-1"), None);
        assert_eq!(parse_rewards("NaN"), None);
        assert_eq!(parse_rewards(&["1"; 15].join(",")), None);
        assert_eq!(rewards_text(&[25, 50, 100]), "0.25,0.5,1");
    }

    #[test]
    fn each_day_pays_its_reward_and_the_last_repeats() {
        let rewards = parse_rewards(DEFAULT_REWARDS).unwrap();
        let paid: Vec<i64> = (1..=9).map(|day| reward_on(&rewards, day)).collect();

        assert_eq!(paid, [25, 25, 50, 50, 75, 75, 100, 100, 100]);
        assert_eq!(reward_on(&rewards, 0), 25);
        assert_eq!(reward_on(&[], 3), 0);
    }

    #[test]
    fn a_missed_day_starts_again_at_day_one() {
        // never claimed
        assert_eq!(claim_day(0, None, date(10)), Some(1));
        // yesterday: the streak goes on
        assert_eq!(claim_day(3, Some(date(9)), date(10)), Some(4));
        // claimed today already
        assert_eq!(claim_day(3, Some(date(10)), date(10)), None);
        // a day missed
        assert_eq!(claim_day(3, Some(date(8)), date(10)), Some(1));
        // across a month's end
        assert_eq!(
            claim_day(
                6,
                Some(chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()),
                date(1)
            ),
            Some(7)
        );
    }

    #[test]
    fn the_page_shows_today_and_tomorrow() {
        let rewards = parse_rewards(DEFAULT_REWARDS).unwrap();

        let open = status_of(&rewards, 2, Some(date(9)), date(10));
        assert_eq!((open.day, open.claimable), (3, true));
        assert_eq!((open.reward, open.next_reward), (0.5, 0.5));

        let claimed = status_of(&rewards, 3, Some(date(10)), date(10));
        assert_eq!((claimed.day, claimed.claimable), (3, false));
        assert_eq!((claimed.reward, claimed.next_reward), (0.5, 0.5));

        let week = status_of(&rewards, 7, Some(date(10)), date(10));
        assert_eq!((week.reward, week.next_reward), (1.0, 1.0));
    }
}
