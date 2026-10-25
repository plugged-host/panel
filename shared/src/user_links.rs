//! plugged.host: the noprofit.space and Discord accounts linked to users, for Admin › Users (its
//! Discord column and search, and the identity lock; docs/features/admin-users.md).

use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Clone, Debug)]
#[schema(title = "AdminUserDiscord")]
pub struct ApiUserDiscord {
    pub id: compact_str::CompactString,
    /// Its name at the last sync or member check; `None` before there was one.
    pub username: Option<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize, Clone, Debug, Default)]
#[schema(title = "AdminUserLinks")]
pub struct ApiUserLinks {
    /// Linked to a noprofit.space account while it is connected: the username and email are
    /// noprofit.space's, so the panel does not change them.
    pub noprofit: bool,
    pub discord: Option<ApiUserDiscord>,
}

/// The links of these users, with one query per source (not one per user): with noprofit.space
/// connected its account links and the Discord account it reports; standalone, the Discord OAuth
/// link (Admin › Settings › Discord) and the name the member check saw.
pub async fn load(
    state: &crate::State,
    users: &[uuid::Uuid],
) -> Result<HashMap<uuid::Uuid, ApiUserLinks>, crate::database::DatabaseError> {
    let mut links: HashMap<uuid::Uuid, ApiUserLinks> = users
        .iter()
        .map(|uuid| (*uuid, ApiUserLinks::default()))
        .collect();
    if users.is_empty() {
        return Ok(links);
    }

    let (connected, provider) = {
        let settings = state.settings.get().await?;
        (
            settings.noprofit.connected(),
            settings.discord.oauth_provider_uuid,
        )
    };

    let rows = if connected {
        sqlx::query(
            r#"
            SELECT user_noprofit_accounts.user_uuid,
                user_noprofit_accounts.discord->>'id' AS discord_id,
                COALESCE(user_noprofit_accounts.discord->>'username', discord_members.username)
                    AS discord_username
            FROM user_noprofit_accounts
            LEFT JOIN discord_members
                ON discord_members.discord_id = user_noprofit_accounts.discord->>'id'
            WHERE user_noprofit_accounts.user_uuid = ANY($1)
            "#,
        )
        .bind(users)
        .fetch_all(state.database.read())
        .await?
    } else if let Some(provider) = provider {
        sqlx::query(
            r#"
            SELECT user_oauth_links.user_uuid,
                user_oauth_links.identifier AS discord_id,
                discord_members.username AS discord_username
            FROM user_oauth_links
            LEFT JOIN discord_members ON discord_members.discord_id = user_oauth_links.identifier
            WHERE user_oauth_links.user_uuid = ANY($1)
                AND user_oauth_links.oauth_provider_uuid = $2
            "#,
        )
        .bind(users)
        .bind(provider)
        .fetch_all(state.database.read())
        .await?
    } else {
        Vec::new()
    };

    for row in rows {
        let uuid: uuid::Uuid = row.try_get("user_uuid")?;
        let Some(link) = links.get_mut(&uuid) else {
            continue;
        };

        link.noprofit = connected;
        link.discord = discord(row.try_get("discord_id")?, row.try_get("discord_username")?);
    }

    Ok(links)
}

/// Fills [`AdminApiUser::links`](crate::models::user::AdminApiUser::links) of these users.
pub async fn attach(
    state: &crate::State,
    users: &mut [crate::models::user::AdminApiUser],
) -> Result<(), crate::database::DatabaseError> {
    let uuids = users.iter().map(|user| user.uuid).collect::<Vec<_>>();
    let mut links = load(state, &uuids).await?;

    for user in users {
        user.links = links.remove(&user.uuid);
    }

    Ok(())
}

/// The user's username and email belong to noprofit.space: it is connected and the user is
/// linked to one of its accounts. Admins then cannot change them (the Pelican panel's identity
/// lock); without noprofit.space, nothing is locked.
pub async fn identity_locked(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<bool, crate::database::DatabaseError> {
    if !state.settings.get().await?.noprofit.connected() {
        return Ok(false);
    }

    Ok(
        crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
            .await?
            .is_some(),
    )
}

/// The part of Admin › Users' search that finds a user by the id or name of their Discord
/// account (`$param`), from either source; OR-ed into the search of `User::all_with_pagination`.
pub fn discord_search_sql(param: u8) -> String {
    format!(
        r#"(${param}::text IS NOT NULL AND (
            EXISTS (
                SELECT 1 FROM user_noprofit_accounts
                WHERE user_noprofit_accounts.user_uuid = users.uuid
                    AND (user_noprofit_accounts.discord->>'id' = ${param}
                        OR user_noprofit_accounts.discord->>'username' ILIKE '%' || ${param} || '%')
            )
            OR EXISTS (
                SELECT 1 FROM discord_members
                WHERE discord_members.user_uuid = users.uuid
                    AND (discord_members.discord_id = ${param}
                        OR discord_members.username ILIKE '%' || ${param} || '%')
            )
        ))"#
    )
}

fn discord(id: Option<String>, username: Option<String>) -> Option<ApiUserDiscord> {
    let id = id.filter(|id| crate::discord::is_snowflake(id))?;

    Some(ApiUserDiscord {
        id: id.into(),
        username: username
            .map(|username| username.trim().to_string())
            .filter(|username| !username.is_empty())
            .map(Into::into),
    })
}

#[cfg(test)]
mod tests {
    use super::{discord, discord_search_sql};

    #[test]
    fn a_discord_link_needs_a_snowflake_id() {
        let linked = discord(Some("80351110224678912".into()), Some(" someone ".into())).unwrap();
        assert_eq!(linked.id, "80351110224678912");
        assert_eq!(linked.username.as_deref(), Some("someone"));

        let unnamed = discord(Some("80351110224678912".into()), Some(" ".into())).unwrap();
        assert_eq!(unnamed.username, None);

        assert!(discord(Some("not-an-id".into()), Some("someone".into())).is_none());
        assert!(discord(None, Some("someone".into())).is_none());
    }

    #[test]
    fn the_discord_search_uses_the_given_parameter() {
        let sql = discord_search_sql(1);
        assert!(sql.starts_with("($1::text IS NOT NULL"));
        assert!(!sql.contains("$2"));
    }
}
