//! plugged.host: profile badges (docs/features/promos.md#profile-badges).
//!
//! A few preset badges an account buys with tokens, for good (`users.badges`), and picks one of to
//! show next to its name (`users.badge`): in the sidebar's account row, the server's subusers and
//! its chat.

use crate::{
    economy::{Currency, EconomyError},
    models::{ByUuid, user::User},
    settings::economy::AppSettingsEconomy,
};
use compact_str::{CompactString, ToCompactString};
use serde::{Deserialize, Serialize};
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

/// The Mantine colours a badge may use. A badge is a small light badge, so the colour must be
/// readable on the dark panel.
pub const COLORS: &[&str] = &[
    "blue", "yellow", "violet", "orange", "teal", "green", "red", "pink", "grape", "indigo",
    "cyan", "lime", "gray",
];

/// One badge's appearance and optional Discord role. `label` empty: the translated default.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BadgeConfig {
    pub id: CompactString,
    pub label: Option<CompactString>,
    /// A Mantine colour name from [`COLORS`].
    pub color: CompactString,
    /// The Discord role granted while this badge is shown; empty: no role.
    pub discord_role: Option<CompactString>,
}

fn default_color(id: &str) -> &'static str {
    match id {
        "supporter" => "blue",
        "early_bird" => "yellow",
        "night_owl" => "violet",
        "og" => "orange",
        "builder" => "teal",
        "explorer" => "green",
        _ => "gray",
    }
}

fn default_badge(id: &str) -> BadgeConfig {
    BadgeConfig {
        id: id.into(),
        label: None,
        color: default_color(id).into(),
        discord_role: None,
    }
}

/// The six preset badges with their current colours and no Discord roles.
pub fn default_badges() -> Vec<BadgeConfig> {
    BADGES.iter().map(|id| default_badge(id)).collect()
}

/// Keeps the six known badges, in the board's order, with valid colours and role ids. Missing
/// badges fall back to their defaults; unknown ids are dropped.
pub fn normalize_badges(input: Vec<BadgeConfig>) -> Vec<BadgeConfig> {
    BADGES
        .iter()
        .map(|id| {
            let Some(mut badge) = input.iter().find(|badge| badge.id == *id).cloned() else {
                return default_badge(id);
            };
            badge.id = (*id).into();
            badge.label = badge
                .label
                .map(|label| label.trim().to_compact_string())
                .filter(|label| !label.is_empty());
            badge.color = if COLORS.contains(&badge.color.as_str()) {
                badge.color
            } else {
                default_color(id).into()
            };
            badge.discord_role = badge
                .discord_role
                .map(|role| role.trim().to_compact_string())
                .filter(|role| crate::discord::is_snowflake(role));
            badge
        })
        .collect()
}

/// The badge's settings, by id.
pub fn config<'a>(economy: &'a AppSettingsEconomy, id: &str) -> Option<&'a BadgeConfig> {
    economy.badges.iter().find(|badge| badge.id == id)
}

/// Gives the linked Discord account the new badge's role and takes the old one away. Best effort:
/// a missing bot token, an unlinked account or a role the bot cannot manage never fails the
/// purchase or the switch.
pub async fn sync_discord_role(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    old: Option<&str>,
    new: Option<&str>,
) {
    if old == new {
        return;
    }
    let settings = match state.settings.get().await {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!(user = %user_uuid, "failed to read the badge roles: {err:#}");
            return;
        }
    };
    let context = match crate::discord::Context::load(state).await {
        Ok(context) => context,
        Err(err) => {
            tracing::warn!(user = %user_uuid, "failed to load the Discord context: {err:#}");
            return;
        }
    };
    let Some(rest) = context.rest(state) else {
        return;
    };
    let Some(discord_id) = crate::discord::linked_discord_id(state, &context, user_uuid)
        .await
        .ok()
        .flatten()
    else {
        return;
    };
    let guild_id = context.settings.guild_id.as_str();
    if guild_id.is_empty() {
        return;
    }

    for (badge, add) in [(old, false), (new, true)] {
        let Some(badge) = badge else {
            continue;
        };
        let Some(role) = config(&settings.economy, badge)
            .and_then(|badge| badge.discord_role.as_deref())
            .filter(|role| crate::discord::is_snowflake(role))
        else {
            continue;
        };
        if let Err(err) = rest.set_role(guild_id, &discord_id, role, add).await {
            tracing::warn!(
                user = %user_uuid,
                badge,
                "failed to sync the badge's Discord role: {err:#}"
            );
        }
    }
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
    let owned: Option<(Vec<String>, Option<String>)> =
        sqlx::query_as("SELECT badges, badge FROM users WHERE uuid = $1 FOR UPDATE")
            .bind(user.uuid)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some((owned, shown)) = owned else {
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

    // the first badge is shown at once; give the linked Discord account its role
    if shown.is_none() {
        let state = state.clone();
        let user_uuid = user.uuid;
        let badge = badge.to_string();
        tokio::spawn(async move {
            sync_discord_role(&state, user_uuid, None, Some(&badge)).await;
        });
    }

    Ok(view(state, user).await?)
}

/// Shows one of the account's badges next to its name, or none.
pub async fn show(
    state: &crate::State,
    user: &User,
    badge: Option<&str>,
) -> Result<ApiBadges, EconomyError> {
    let mut previous = None;
    if let Some(badge) = badge {
        let (owned, shown, _) = read(state, user.uuid).await?;
        previous = shown;
        if !known(badge) || !owned.iter().any(|owned| owned == badge) {
            return Err(EconomyError::BadRequest("you do not have this badge"));
        }
    } else {
        previous = read(state, user.uuid).await?.1;
    }

    sqlx::query("UPDATE users SET badge = $2 WHERE uuid = $1")
        .bind(user.uuid)
        .bind(badge)
        .execute(state.database.write())
        .await?;

    User::invalidate_cached(&state.database, user.uuid).await;

    let background_state = state.clone();
    let user_uuid = user.uuid;
    let new = badge.map(str::to_string);
    tokio::spawn(async move {
        sync_discord_role(
            &background_state,
            user_uuid,
            previous.as_deref(),
            new.as_deref(),
        )
        .await;
    });

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
