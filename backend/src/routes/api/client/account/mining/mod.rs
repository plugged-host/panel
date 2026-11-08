//! plugged.host: the desktop app's AFK mining page (docs/features/app-mining.md).
//!
//! The app keeps a local session while it is open and claims once a minute. The panel pays whole
//! tokens for the time since the last mining row, but never more than two minutes per claim, so
//! closing the app stops the clock. The daily cap and the rate are env settings; the ledger type is
//! `mining`, so the Tokens page's history shows it like every other earning.

use super::State;
use axum::http::StatusCode;
use chrono::Utc;
use serde::Serialize;
use shared::{
    GetState,
    economy::{Currency, change},
    models::user::GetUser,
    response::{ApiResponse, ApiResponseResult},
};
use sqlx::Row;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

const KIND: &str = "mining";
/// A claim covers at most this much time: the app claims once a minute, and a closed app must not
/// pay for the hours it was away.
const MAX_CLAIM_SECONDS: i64 = 120;

fn per_hour() -> i64 {
    std::env::var("PLUGGED_MINING_TOKENS_PER_HOUR")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(6)
        .clamp(0, 120)
}

fn daily_cap() -> i64 {
    std::env::var("PLUGGED_MINING_DAILY_CAP")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(24)
        .clamp(0, 1000)
}

#[derive(ToSchema, Serialize)]
pub struct MiningStatus {
    tokens: i64,
    tokens_earned: i64,
    rate_per_hour: i64,
    daily_cap: i64,
    daily_earned: i64,
    last_claim: Option<chrono::DateTime<Utc>>,
    next_claim_in: i64,
    can_claim: bool,
}

async fn status(state: &shared::State, user: &shared::models::user::User) -> anyhow::Result<MiningStatus> {
    let row = sqlx::query(
        "SELECT tokens, tokens_earned,
            (SELECT MAX(created) FROM user_token_transactions
                WHERE user_uuid = $1 AND type = $2) AS last_claim,
            (SELECT COALESCE(SUM(amount), 0)::BIGINT FROM user_token_transactions
                WHERE user_uuid = $1 AND type = $2 AND created >= CURRENT_DATE) AS daily_earned
        FROM users WHERE uuid = $1",
    )
    .bind(user.uuid)
    .bind(KIND)
    .fetch_one(state.database.write())
    .await?;

    let tokens: i64 = row.try_get("tokens")?;
    let tokens_earned: i64 = row.try_get("tokens_earned")?;
    let last: Option<chrono::NaiveDateTime> = row.try_get("last_claim")?;
    let daily_earned: i64 = row.try_get("daily_earned")?;
    let rate = per_hour();
    let cap = daily_cap();
    let elapsed = last
        .map(|last| (Utc::now().naive_utc() - last).num_seconds().max(0))
        .unwrap_or(0);
    let interval = if rate > 0 { (3600 / rate).max(1) } else { 0 };

    Ok(MiningStatus {
        tokens,
        tokens_earned,
        rate_per_hour: rate,
        daily_cap: cap,
        daily_earned,
        last_claim: last.map(|last| last.and_utc()),
        next_claim_in: if rate > 0 { (interval - elapsed).max(0) } else { 0 },
        can_claim: rate > 0 && cap > 0 && daily_earned < cap,
    })
}

mod get {
    use super::*;

    #[utoipa::path(get, path = "/", responses((status = OK, body = inline(MiningStatus))))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        if !state
            .settings
            .get_as(|settings| settings.economy.tokens_enabled)
            .await?
        {
            return ApiResponse::error("tokens are disabled")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        ApiResponse::new_serialized(super::status(&state, &user).await?).ok()
    }
}

mod start {
    use super::*;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[utoipa::path(post, path = "/", responses((status = OK, body = inline(MiningStatus))))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        if !state
            .settings
            .get_as(|settings| settings.economy.tokens_enabled)
            .await?
        {
            return ApiResponse::error("tokens are disabled")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let mut transaction = state.database.write().begin().await?;
        // A zero-token row marks the moment the session started; claims measure from it.
        change(
            &mut transaction,
            Currency::Tokens,
            user.uuid,
            0,
            KIND,
            "AFK mining started",
            serde_json::json!({ "started": true }),
        )
        .await?;
        transaction.commit().await?;

        ApiResponse::new_serialized(super::status(&state, &user).await?).ok()
    }

    pub fn router(state: &super::State) -> OpenApiRouter<super::State> {
        OpenApiRouter::new()
            .routes(routes!(route))
            .with_state(state.clone())
    }
}

mod claim {
    use super::*;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(ToSchema, Serialize)]
    struct Response {
        earned: i64,
        #[schema(inline)]
        mining: MiningStatus,
    }

    #[utoipa::path(post, path = "/", responses((status = OK, body = inline(Response))))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        if !state
            .settings
            .get_as(|settings| settings.economy.tokens_enabled)
            .await?
        {
            return ApiResponse::error("tokens are disabled")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let rate = per_hour();
        let cap = daily_cap();
        if rate <= 0 || cap <= 0 {
            return ApiResponse::new_serialized(Response {
                earned: 0,
                mining: super::status(&state, &user).await?,
            })
            .ok();
        }

        let row = sqlx::query(
            "SELECT
                (SELECT MAX(created) FROM user_token_transactions
                    WHERE user_uuid = $1 AND type = $2) AS last_claim,
                (SELECT COALESCE(SUM(amount), 0)::BIGINT FROM user_token_transactions
                    WHERE user_uuid = $1 AND type = $2 AND created >= CURRENT_DATE) AS daily_earned",
        )
        .bind(user.uuid)
        .bind(KIND)
        .fetch_one(state.database.write())
        .await?;
        let last: Option<chrono::NaiveDateTime> = row.try_get("last_claim")?;
        let daily_earned: i64 = row.try_get("daily_earned")?;
        let elapsed = last
            .map(|last| (Utc::now().naive_utc() - last).num_seconds().max(0))
            .unwrap_or(0)
            .min(MAX_CLAIM_SECONDS);
        let mut earned = elapsed * rate / 3600;
        earned = earned.min((cap - daily_earned).max(0));

        if earned > 0 {
            let mut transaction = state.database.write().begin().await?;
            if change(
                &mut transaction,
                Currency::Tokens,
                user.uuid,
                earned,
                KIND,
                "AFK mining",
                serde_json::json!({ "seconds": elapsed }),
            )
            .await?
            .is_none()
            {
                transaction.rollback().await?;
                earned = 0;
            } else {
                transaction.commit().await?;
            }
        }

        ApiResponse::new_serialized(Response {
            earned,
            mining: super::status(&state, &user).await?,
        })
        .ok()
    }

    pub fn router(state: &super::State) -> OpenApiRouter<super::State> {
        OpenApiRouter::new()
            .routes(routes!(route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/start", start::router(state))
        .nest("/claim", claim::router(state))
        .with_state(state.clone())
}
