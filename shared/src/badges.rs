//! plugged.host: profile badges (docs/features/promos.md#profile-badges).
//!
//! A few preset badges an account buys with tokens, for good (`users.badges`), and picks one of to
//! show next to its name (`users.badge`): in the sidebar's account row, the server's subusers and
//! its chat.

use crate::{
    economy::{Currency, EconomyError},
    models::{ByUuid, user::User},
};
use serde::Serialize;
use utoipa::ToSchema;

/// The ledger type of a badge.
pub const TYPE_BADGE: &str = "badge";

/// The badges on sale, in the order the account page lists them.
pub const BADGES: &[&str] = &[
    "supporter",
    "early_bird",
    "night_owl",
    "og",
    "builder",
    "explorer",
];

pub fn known(badge: &str) -> bool {
    BADGES.contains(&badge)
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiBadges {
    /// Whether badges are sold (owned ones show either way).
    pub enabled: bool,
    /// Tokens a badge costs.
    pub tokens: i64,
    /// Every badge on sale.
    pub available: Vec<String>,
    /// The ones this account bought.
    pub owned: Vec<String>,
    /// The one shown next to its name.
    pub shown: Option<String>,
    /// The account's tokens.
    pub balance: i64,
}

async fn read(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<(Vec<String>, Option<String>, i64), sqlx::Error> {
    let row: Option<(Vec<String>, Option<String>, i64)> =
        sqlx::query_as("SELECT badges, badge, tokens FROM users WHERE uuid = $1")
            .bind(user_uuid)
            .fetch_optional(state.database.write())
            .await?;

    Ok(row.unwrap_or_default())
}

/// The account's badges and what they cost.
pub async fn view(state: &crate::State, user: &User) -> Result<ApiBadges, anyhow::Error> {
    let (enabled, tokens) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.badges_enabled && settings.economy.tokens_enabled,
            settings.economy.badge_tokens.max(1) as i64,
        )
    };
    let (owned, shown, balance) = read(state, user.uuid).await?;

    Ok(ApiBadges {
        enabled,
        tokens,
        available: BADGES.iter().map(|badge| badge.to_string()).collect(),
        owned: owned.into_iter().filter(|badge| known(badge)).collect(),
        shown: shown.filter(|badge| known(badge)),
        balance,
    })
}

/// Buys `badge` for good: the user row locked, the tokens, the ledger row and the badge written
/// together. The first badge bought is shown at once. `price` is the price the user saw.
pub async fn buy(
    state: &crate::State,
    user: &User,
    badge: &str,
    price: Option<i64>,
) -> Result<ApiBadges, EconomyError> {
    let cost = {
        let settings = state.settings.get().await?;
        if !settings.economy.badges_enabled {
            return Err(EconomyError::Disabled);
        }
        if !settings.economy.tokens_enabled {
            return Err(EconomyError::Unavailable);
        }
        settings.economy.badge_tokens.max(1) as i64
    };
    if !known(badge) {
        return Err(EconomyError::BadRequest("unknown badge"));
    }
    if price.is_some_and(|price| price != cost) {
        return Err(EconomyError::PriceMoved);
    }

    let mut transaction = state.database.write().begin().await?;
    let owned: Option<Vec<String>> =
        sqlx::query_scalar("SELECT badges FROM users WHERE uuid = $1 FOR UPDATE")
            .bind(user.uuid)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(owned) = owned else {
        return Err(EconomyError::Other(anyhow::anyhow!("account not found")));
    };
    if owned.iter().any(|owned| owned == badge) {
        return Err(EconomyError::Owned);
    }

    if crate::economy::change(
        &mut transaction,
        Currency::Tokens,
        user.uuid,
        -cost,
        TYPE_BADGE,
        "Profile badge",
        serde_json::json!({ "badge": badge }),
    )
    .await?
    .is_none()
    {
        return Err(EconomyError::Short);
    }

    sqlx::query(
        "UPDATE users SET badges = array_append(badges, $2), badge = COALESCE(badge, $2)
        WHERE uuid = $1",
    )
    .bind(user.uuid)
    .bind(badge)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    User::invalidate_cached(&state.database, user.uuid).await;

    Ok(view(state, user).await?)
}

/// Shows one of the account's badges next to its name, or none.
pub async fn show(
    state: &crate::State,
    user: &User,
    badge: Option<&str>,
) -> Result<ApiBadges, EconomyError> {
    if let Some(badge) = badge {
        let (owned, _, _) = read(state, user.uuid).await?;
        if !known(badge) || !owned.iter().any(|owned| owned == badge) {
            return Err(EconomyError::BadRequest("you do not have this badge"));
        }
    }

    sqlx::query("UPDATE users SET badge = $2 WHERE uuid = $1")
        .bind(user.uuid)
        .bind(badge)
        .execute(state.database.write())
        .await?;

    User::invalidate_cached(&state.database, user.uuid).await;

    Ok(view(state, user).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_ids_are_short_and_plain() {
        for badge in BADGES {
            assert!(known(badge));
            assert!(badge.len() <= 32);
            assert!(badge.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
        }
        assert!(!known("admin"));
    }
}
