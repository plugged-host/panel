//! plugged.host: referrals (docs/features/referrals.md).
//!
//! Every account has a code and a link, `https://<panel>/r/<code>`. Opening it keeps the code in a
//! cookie for 30 days and goes to sign-in. When a new panel user is made (the first noprofit.space
//! sign-in, registration, an OAuth sign-up) with the cookie, [`record`] notes who brought it:
//! never the account itself, never an account that existed before, and not when the sign-up comes
//! from the address the referrer was last seen at. Once the friend earned **Friend Earns** tokens
//! (10 by default; `users.tokens_earned`), [`pay`] gives the referrer **Tokens per Friend** (5)
//! once, at most **Paid per Month** (10) friends in any 30 days, with a ledger row and a
//! notification. A friend over that waits for the window to free up.

use crate::economy::{Currency, change};
use rand::distr::SampleString;
use serde::Serialize;
use utoipa::ToSchema;

/// The cookie the link leaves for the sign-up.
pub const COOKIE: &str = "referral";
/// How long the cookie is kept.
pub const COOKIE_DAYS: i64 = 30;
/// The ledger type of a referrer's tokens.
pub const TYPE_REFERRAL: &str = "referral";
const CODE_LENGTH: usize = 8;
/// The friends the Tokens page lists, newest first.
const FRIENDS_SHOWN: i64 = 50;
/// Friends paid per run of [`pay`] at most.
const PAY_BATCH: i64 = 200;
/// The window the monthly cap counts in.
const CAP_DAYS: i32 = 30;

/// Whether referrals are offered: tokens and referrals both on.
pub fn offered(economy: &crate::settings::economy::AppSettingsEconomy) -> bool {
    economy.tokens_enabled && economy.referrals_enabled
}

/// A code as the panel makes them: 6 to 16 lowercase letters and digits.
pub fn valid_code(code: &str) -> bool {
    (6..=16).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The referral link of a code.
pub fn link(app_url: &str, code: &str) -> String {
    format!("{}/r/{code}", app_url.trim_end_matches('/'))
}

/// The account's code, made the first time it is asked for.
pub async fn code(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<String, anyhow::Error> {
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT referral_code FROM users WHERE uuid = $1")
            .bind(user_uuid)
            .fetch_optional(database.write())
            .await?;
    if let Some(Some(code)) = stored {
        return Ok(code);
    }

    // a taken code (unique index) is drawn again
    for _ in 0..5 {
        let code = rand::distr::Alphanumeric
            .sample_string(&mut rand::rng(), CODE_LENGTH)
            .to_ascii_lowercase();

        let set: Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
            "UPDATE users SET referral_code = COALESCE(referral_code, $2)
            WHERE uuid = $1
            RETURNING referral_code",
        )
        .bind(user_uuid)
        .bind(&code)
        .fetch_optional(database.write())
        .await;

        match set {
            Ok(Some(code)) => return Ok(code),
            Ok(None) => anyhow::bail!("the account is gone"),
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => continue,
            Err(err) => return Err(err.into()),
        }
    }

    anyhow::bail!("no free referral code after 5 tries")
}

/// Whether an account has the code (the link's route sets its cookie only then).
pub async fn code_exists(
    database: &crate::database::Database,
    code: &str,
) -> Result<bool, sqlx::Error> {
    if !valid_code(code) {
        return Ok(false);
    }

    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE referral_code = $1)")
        .bind(code)
        .fetch_one(database.read())
        .await
}

/// Records who brought a newly made account, from its sign-up's referral cookie, and removes the
/// cookie. Never fails the sign-up: a problem is logged.
pub async fn record(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    cookies: &tower_cookies::Cookies,
    ip: std::net::IpAddr,
) {
    let Some(code) = cookies
        .get(COOKIE)
        .map(|cookie| cookie.value().trim().to_ascii_lowercase())
    else {
        return;
    };
    cookies.remove(tower_cookies::Cookie::build(COOKIE).path("/").build());

    if let Err(err) = record_code(state, user_uuid, &code, ip).await {
        tracing::warn!(user = %user_uuid, "failed to record a referral: {err:#}");
    }
}

async fn record_code(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    code: &str,
    ip: std::net::IpAddr,
) -> Result<(), anyhow::Error> {
    let on = offered(&state.settings.get().await?.economy);
    if !on || !valid_code(code) {
        return Ok(());
    }

    let referrer: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT uuid FROM users WHERE referral_code = $1")
            .bind(code)
            .fetch_optional(state.database.write())
            .await?;
    let Some(referrer) = referrer.filter(|referrer| *referrer != user_uuid) else {
        return Ok(());
    };

    // the referrer signing up a second account from where they are
    let last_seen: Option<sqlx::types::ipnetwork::IpNetwork> = sqlx::query_scalar(
        "SELECT ip FROM user_sessions WHERE user_uuid = $1 ORDER BY last_used DESC LIMIT 1",
    )
    .bind(referrer)
    .fetch_optional(state.database.write())
    .await?;
    if last_seen.is_some_and(|seen| seen.ip() == ip) {
        tracing::info!(user = %user_uuid, referrer = %referrer, "a referral from the referrer's own address, not counted");
        return Ok(());
    }

    // a new account only: made in the last hour, and not referred before
    sqlx::query(
        "UPDATE users SET referred_by = $2
        WHERE uuid = $1 AND referred_by IS NULL AND created >= NOW() - INTERVAL '1 hour'",
    )
    .bind(user_uuid)
    .bind(referrer)
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// A friend the account brought, as the Tokens page lists them.
#[derive(ToSchema, Serialize)]
pub struct ApiReferralFriend {
    pub username: String,
    pub tokens_earned: i64,
    /// When the referrer was paid for them; `None`: not yet.
    pub paid: Option<chrono::DateTime<chrono::Utc>>,
}

/// The Tokens page's referral section.
#[derive(ToSchema, Serialize)]
pub struct ApiReferrals {
    pub code: String,
    pub url: String,
    /// Tokens per friend.
    pub reward: i64,
    /// Tokens a friend earns before it pays.
    pub threshold: i64,
    /// Friends paid in any 30 days at most (0: no limit).
    pub monthly_cap: i64,
    /// Friends paid in the last 30 days.
    pub paid_this_month: i64,
    /// All the friends brought.
    pub total: i64,
    /// The newest 50.
    pub friends: Vec<ApiReferralFriend>,
}

/// The section; `None` while referrals are off.
pub async fn page(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<Option<ApiReferrals>, anyhow::Error> {
    let (economy, app_url) = {
        let settings = state.settings.get().await?;
        (settings.economy.clone(), settings.app.url.to_string())
    };
    if !offered(&economy) {
        return Ok(None);
    }

    let code = code(&state.database, user_uuid).await?;
    let rows: Vec<(String, i64, Option<chrono::NaiveDateTime>)> = sqlx::query_as(
        "SELECT username, tokens_earned, referral_paid FROM users
        WHERE referred_by = $1
        ORDER BY created DESC
        LIMIT $2",
    )
    .bind(user_uuid)
    .bind(FRIENDS_SHOWN)
    .fetch_all(state.database.read())
    .await?;
    let (total, paid_this_month): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*),
            COUNT(*) FILTER (WHERE referral_paid >= NOW() - make_interval(days => $2))
        FROM users WHERE referred_by = $1",
    )
    .bind(user_uuid)
    .bind(CAP_DAYS)
    .fetch_one(state.database.read())
    .await?;

    Ok(Some(ApiReferrals {
        url: link(&app_url, &code),
        code,
        reward: economy.referral_reward.max(1) as i64,
        threshold: economy.referral_threshold.max(1) as i64,
        monthly_cap: economy.referral_monthly_cap.max(0) as i64,
        paid_this_month,
        total,
        friends: rows
            .into_iter()
            .map(|(username, tokens_earned, paid)| ApiReferralFriend {
                username,
                tokens_earned,
                paid: paid.map(|paid| paid.and_utc()),
            })
            .collect(),
    }))
}

/// Pays the referrers of friends who reached the threshold (the `pay_referrals` task, every
/// minute): each friend once, at most the monthly cap per referrer in any 30 days.
pub async fn pay(state: &crate::State) -> Result<(), anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    if !offered(&economy) {
        return Ok(());
    }
    let reward = economy.referral_reward.max(1) as i64;
    let threshold = economy.referral_threshold.max(1) as i64;
    let cap = economy.referral_monthly_cap.max(0) as i64;

    // referrers at their cap are left out, so they never fill a run
    let due: Vec<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
        "SELECT friend.uuid, friend.referred_by, friend.username FROM users friend
        WHERE friend.referred_by IS NOT NULL AND friend.referral_paid IS NULL
            AND friend.tokens_earned >= $1
            AND ($2 = 0 OR (
                SELECT COUNT(*) FROM users paid
                WHERE paid.referred_by = friend.referred_by
                    AND paid.referral_paid >= NOW() - make_interval(days => $3)
            ) < $2)
        ORDER BY friend.created
        LIMIT $4",
    )
    .bind(threshold)
    .bind(cap)
    .bind(CAP_DAYS)
    .bind(PAY_BATCH)
    .fetch_all(state.database.write())
    .await?;

    for (friend, referrer, username) in due {
        match pay_one(state, friend, referrer, &username, reward, threshold, cap).await {
            Ok(true) => {
                crate::notifications::send(
                    state,
                    referrer,
                    None,
                    crate::notifications::REFERRAL_PAID,
                    serde_json::json!({
                        "username": username,
                        "amount": reward,
                        "threshold": threshold,
                    }),
                )
                .await;
            }
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(friend = %friend, referrer = %referrer, "failed to pay a referral: {err:#}");
            }
        }
    }

    Ok(())
}

/// Pays one friend's referrer, with the referrer's row locked so its 30-day count holds. False
/// when it was paid meanwhile or the referrer reached the cap.
async fn pay_one(
    state: &crate::State,
    friend: uuid::Uuid,
    referrer: uuid::Uuid,
    username: &str,
    reward: i64,
    threshold: i64,
    cap: i64,
) -> Result<bool, anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    sqlx::query("SELECT 1 FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(referrer)
        .execute(&mut *transaction)
        .await?;

    if cap > 0 {
        let paid: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users
            WHERE referred_by = $1 AND referral_paid >= NOW() - make_interval(days => $2)",
        )
        .bind(referrer)
        .bind(CAP_DAYS)
        .fetch_one(&mut *transaction)
        .await?;
        if paid >= cap {
            return Ok(false);
        }
    }

    let marked = sqlx::query(
        "UPDATE users SET referral_paid = NOW()
        WHERE uuid = $1 AND referred_by = $2 AND referral_paid IS NULL AND tokens_earned >= $3",
    )
    .bind(friend)
    .bind(referrer)
    .bind(threshold)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if marked == 0 {
        return Ok(false);
    }

    change(
        &mut transaction,
        Currency::Tokens,
        referrer,
        reward,
        TYPE_REFERRAL,
        username,
        serde_json::json!({ "friend": friend }),
    )
    .await?;
    transaction.commit().await?;

    Ok(true)
}

/// Who brought an account, for Admin › Users › a user › Economy.
#[derive(ToSchema, Serialize)]
pub struct ApiReferrer {
    pub uuid: uuid::Uuid,
    pub username: String,
}

/// Admin › Users › a user › Economy: who brought the account and how many it brought.
#[derive(ToSchema, Serialize)]
pub struct ApiAdminReferrals {
    pub referred_by: Option<ApiReferrer>,
    pub referred: i64,
    /// Of those, the ones its referrer was paid for.
    pub paid: i64,
}

pub async fn admin_view(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
) -> Result<ApiAdminReferrals, sqlx::Error> {
    let referred_by: Option<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT referrer.uuid, referrer.username FROM users
        JOIN users referrer ON referrer.uuid = users.referred_by
        WHERE users.uuid = $1",
    )
    .bind(user_uuid)
    .fetch_optional(database.read())
    .await?;
    let (referred, paid): (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COUNT(referral_paid) FROM users WHERE referred_by = $1")
            .bind(user_uuid)
            .fetch_one(database.read())
            .await?;

    Ok(ApiAdminReferrals {
        referred_by: referred_by.map(|(uuid, username)| ApiReferrer { uuid, username }),
        referred,
        paid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_short_lowercase_words() {
        assert!(valid_code("ab12cd34"));
        assert!(!valid_code("AB12CD34"));
        assert!(!valid_code("ab12"));
        assert!(!valid_code("ab12cd34ab12cd34x"));
        assert!(!valid_code("ab12-cd34"));
        assert!(!valid_code(""));
    }

    #[test]
    fn the_link_names_the_code() {
        assert_eq!(
            link("https://panel.example.com/", "ab12cd34"),
            "https://panel.example.com/r/ab12cd34"
        );
    }
}
