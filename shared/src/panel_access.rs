//! plugged.host: the panel itself costs tokens (Admin › Settings › Economy, docs/features/economy.md).
//!
//! One payment buys a month of access; the date lives in `users.panel_access_until`. The first
//! membership (and the second) and administrators never pay, and a blocked account may still use
//! the account endpoints that let it earn tokens, buy access, change its settings and log out.
//!
//! The gate runs in the client API's auth middleware, before a route sees the request, so a
//! blocked account cannot reach servers through the panel or an API key.

use crate::{
    State,
    economy::{Currency, EconomyError},
    models::{ByUuid, user::User},
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use utoipa::ToSchema;

/// The response header that tells the frontend to show the paywall instead of a plain error.
pub const HEADER: &str = "X-Panel-Access";

/// What a blocked account may still call: everything on the account page (profile, settings,
/// tokens and earning, balance, memberships, panel access, logout) and the permissions it needs
/// to render them. Servers and everything else wait for a payment.
pub fn allowed(matched_path: &str) -> bool {
    const PREFIXES: &[&str] = &["/api/client/account", "/api/client/permissions"];

    PREFIXES.iter().any(|prefix| {
        matched_path == *prefix
            || matched_path
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Whether this account has to pay before it can use the panel. `None` in the settings means the
/// feature is off; a price of 0 makes it free and therefore off too.
pub fn required(
    user: &User,
    economy: &crate::settings::economy::AppSettingsEconomy,
    now: chrono::NaiveDateTime,
) -> bool {
    if !economy.panel_access_enabled || economy.panel_access_tokens <= 0 || !economy.tokens_enabled
    {
        return false;
    }
    if user.admin || user.has_plus || user.has_plusplus {
        return false;
    }

    user.panel_access_until.map_or(true, |until| until <= now)
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiPanelAccess {
    pub enabled: bool,
    /// Whether this account has to pay before it can use the panel.
    pub required: bool,
    /// Whether a membership (or being an administrator) makes the panel free for it.
    pub exempt: bool,
    /// Tokens one payment costs.
    pub tokens: i64,
    /// Days one payment buys.
    pub days: i64,
    /// When the paid access runs out; `None` when it never paid.
    pub until: Option<DateTime<Utc>>,
}

/// The account's panel access as it is now.
pub async fn status(state: &State, user: &User) -> Result<ApiPanelAccess, anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    let now = Utc::now().naive_utc();

    Ok(ApiPanelAccess {
        enabled: economy.panel_access_enabled
            && economy.panel_access_tokens > 0
            && economy.tokens_enabled,
        required: required(user, &economy, now),
        exempt: user.admin || user.has_plus || user.has_plusplus,
        tokens: economy.panel_access_tokens.max(0) as i64,
        days: economy.panel_access_days.max(1) as i64,
        until: user
            .panel_access_until
            .filter(|until| *until > now)
            .map(|until| until.and_utc()),
    })
}

/// Buys (or extends) a month of access with tokens. Time bought early adds to what is left, so
/// buying before the month ends loses nothing.
pub async fn buy(state: &State, user: &User) -> Result<ApiPanelAccess, EconomyError> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.panel_access_enabled {
        return Err(EconomyError::Disabled);
    }

    let cost = economy.panel_access_tokens.max(0) as i64;
    if cost > 0 && !economy.tokens_enabled {
        return Err(EconomyError::Unavailable);
    }
    if user.admin || user.has_plus || user.has_plusplus {
        return Err(EconomyError::Owned);
    }

    let days = economy.panel_access_days.max(1) as i64;
    let now = Utc::now().naive_utc();

    let mut transaction = state.database.write().begin().await?;
    let current: Option<Option<chrono::NaiveDateTime>> =
        sqlx::query_scalar("SELECT panel_access_until FROM users WHERE uuid = $1 FOR UPDATE")
            .bind(user.uuid)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(current) = current else {
        return Err(EconomyError::Other(anyhow::anyhow!("account not found")));
    };

    let from = current.filter(|until| *until > now).unwrap_or(now);
    let until = from + Duration::days(days);

    if cost > 0
        && crate::economy::change(
            &mut transaction,
            Currency::Tokens,
            user.uuid,
            -cost,
            crate::panel_access::TYPE_PANEL_ACCESS,
            "Panel access",
            serde_json::json!({ "days": days }),
        )
        .await?
        .is_none()
    {
        return Err(EconomyError::Short);
    }

    sqlx::query("UPDATE users SET panel_access_until = $2 WHERE uuid = $1")
        .bind(user.uuid)
        .bind(until)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    User::invalidate_cached(&state.database, user.uuid).await;

    Ok(ApiPanelAccess {
        enabled: true,
        required: false,
        exempt: user.admin || user.has_plus || user.has_plusplus,
        tokens: cost,
        days,
        until: Some(until.and_utc()),
    })
}

pub const TYPE_PANEL_ACCESS: &str = "panel_access";
