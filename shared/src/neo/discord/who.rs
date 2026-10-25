//! plugged.host: who a Discord account is to neo, and its turns (docs/features/neo-discord.md).
//!
//! A Discord account linked to an account (through the Discord OAuth provider, or on
//! noprofit.space) uses that account's limits, shared with neo in the panel; one nobody linked has
//! windows of its own at a share of them (`neo::discord_unlinked_percent`). While noprofit.space
//! runs neo, it decides both and meters the turns; otherwise the panel does, in `neo_usage_*`.

use super::{
    super::{
        Ask, Limits, TurnAnswer, TurnError, gateway, local,
        usage::{self, Subject},
    },
    Setup,
};
use crate::settings::neo::NeoPlan;
use serde_json::Value;

/// The service the panel records neo on Discord's turns under.
pub const SERVICE: &str = "discord";
/// How long noprofit.space's answer about a Discord account is kept (a burst of messages asks once).
const USAGE_TTL: u64 = 20;

#[derive(Debug, Clone)]
pub struct Who {
    pub discord_id: String,
    /// Linked to an account: its limits are the account's.
    pub linked: bool,
    /// The panel user the Discord account belongs to.
    pub user_uuid: Option<uuid::Uuid>,
    /// noprofit.space's account, while noprofit.space runs neo.
    pub account_id: Option<i64>,
    /// The account's name on noprofit.space.
    pub account_name: Option<String>,
    pub plan: NeoPlan,
    pub usage_percent: i64,
    pub limits: Option<Limits>,
    /// The window that refuses new turns.
    pub blocked: Option<String>,
    /// The share of the limits an unlinked Discord account gets, in % (noprofit.space's own while
    /// it runs neo). Not read yet: the prompt (`neo::prompt::DiscordChat`) states the panel
    /// setting's share until it takes this one.
    #[allow(dead_code)]
    pub unlinked_percent: i64,
}

impl Who {
    /// When the window that refuses turns opens again (RFC 3339).
    pub fn resets_at(&self, window: &str) -> Option<&str> {
        self.limits.as_ref()?.get(window)?.resets_at.as_deref()
    }
}

/// Whose windows a turn the panel meters uses: the panel account the Discord account belongs to,
/// or the Discord account on its own.
pub fn subject(discord_id: &str, user_uuid: Option<uuid::Uuid>, usage_percent: i64) -> Subject {
    match user_uuid {
        Some(user_uuid) => Subject::user(user_uuid, usage_percent),
        None => Subject::unlinked_discord(discord_id),
    }
}

/// noprofit.space's answer about a Discord account. `user_uuid` is the panel user of the account it
/// names (or, when it names none, the one the panel knows the Discord account by).
pub fn from_gateway(
    discord_id: &str,
    data: &Value,
    user_uuid: Option<uuid::Uuid>,
    plan: NeoPlan,
    usage_percent: i64,
) -> Who {
    let linked = data["linked"].as_bool().unwrap_or(false);

    Who {
        discord_id: discord_id.to_string(),
        linked,
        user_uuid: user_uuid.filter(|_| linked),
        account_id: data["account_id"].as_i64().filter(|id| *id > 0),
        account_name: data["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        plan: if linked { plan } else { NeoPlan::Free },
        usage_percent: if linked { usage_percent } else { 100 },
        limits: serde_json::from_value(data["limits"].clone()).ok(),
        blocked: data["blocked"]
            .as_str()
            .filter(|window| !window.is_empty())
            .map(str::to_string),
        unlinked_percent: data["unlinked_percent"]
            .as_i64()
            .unwrap_or(50)
            .clamp(0, 100),
    }
}

fn unavailable(reason: impl std::fmt::Display) -> TurnError {
    TurnError::Unavailable {
        reason: reason.to_string(),
    }
}

/// The membership of the panel user, as neo sees it.
async fn plan_of(state: &crate::State, user_uuid: uuid::Uuid) -> Result<NeoPlan, TurnError> {
    let settings = state.settings.get().await.map_err(unavailable)?;

    Ok(super::super::plan(state, &settings, user_uuid).await)
}

fn usage_key(discord_id: &str) -> String {
    format!("neo::discord::usage::{discord_id}")
}

/// Who the Discord account is and where its windows stand.
pub async fn resolve(
    state: &crate::State,
    setup: &Setup,
    discord_id: &str,
) -> Result<Who, TurnError> {
    let known = crate::discord::user_for_discord_id(state, &setup.discord, discord_id)
        .await
        .map_err(unavailable)?;
    let (plan, usage_percent) = match known {
        Some(user_uuid) => {
            let plan = plan_of(state, user_uuid).await?;
            (plan, setup.neo.usage_percent(plan))
        }
        None => (NeoPlan::Free, 100),
    };

    let Some(config) = &setup.gateway else {
        let subject = subject(discord_id, known, usage_percent);
        let limits = usage::snapshot(&state.database, &setup.neo, &subject)
            .await
            .map_err(unavailable)?;

        return Ok(Who {
            discord_id: discord_id.to_string(),
            linked: known.is_some(),
            user_uuid: known,
            account_id: None,
            account_name: None,
            plan,
            usage_percent,
            blocked: usage::blocking_window(&limits),
            limits: Some(limits),
            unlinked_percent: usage::unlinked_discord_percent(&setup.neo),
        });
    };

    let data = match state.cache.get::<Value>(&usage_key(discord_id)).await {
        Ok(Some(data)) => data,
        _ => {
            let data =
                gateway::discord_usage(state, config, discord_id, plan, usage_percent).await?;
            let _ = state
                .cache
                .set(&usage_key(discord_id), USAGE_TTL, &data)
                .await;
            data
        }
    };

    // the panel user of the account noprofit.space names (the panel's copy of the links may be a
    // sync behind), else the one the panel knows the Discord account by
    let mut user_uuid = known;
    if let Some(account_id) = data["account_id"].as_i64().filter(|id| *id > 0)
        && let Ok(Some(link)) =
            crate::noprofit::Link::by_account_id(&state.database, account_id).await
        && link.deleted.is_none()
    {
        user_uuid = Some(link.user_uuid);
    }

    // its turns are metered at that user's membership
    let (plan, usage_percent) = match user_uuid {
        Some(user_uuid) if known != Some(user_uuid) => {
            let plan = plan_of(state, user_uuid).await?;
            (plan, setup.neo.usage_percent(plan))
        }
        _ => (plan, usage_percent),
    };

    Ok(from_gateway(
        discord_id,
        &data,
        user_uuid,
        plan,
        usage_percent,
    ))
}

/// One model turn for the Discord account, metered against it.
pub async fn turn(
    state: &crate::State,
    setup: &Setup,
    who: &Who,
    ask: Ask<'_>,
) -> Result<TurnAnswer, TurnError> {
    match &setup.gateway {
        Some(config) => {
            let answer = gateway::discord_turn(
                state,
                config,
                &who.discord_id,
                who.plan,
                who.usage_percent,
                ask,
            )
            .await;
            if answer.is_ok() {
                let _ = state.cache.invalidate(&usage_key(&who.discord_id)).await;
            }

            answer
        }
        None => {
            let subject = subject(&who.discord_id, who.user_uuid, who.usage_percent);
            let slot = local::slot_for(&setup.neo, None, who.plan)?;

            local::run(state, &setup.neo, &subject, SERVICE, ask, slot, who.plan).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DISCORD: &str = "123456789012345678";

    #[test]
    fn linked_accounts_use_their_own_windows() {
        let user = uuid::Uuid::new_v4();

        let linked = subject(DISCORD, Some(user), 125);
        assert_eq!(linked.user_uuid, Some(user));
        assert_eq!(linked.usage_percent, 125);
        assert!(!linked.is_unlinked_discord());

        let stranger = subject(DISCORD, None, 125);
        assert!(stranger.is_unlinked_discord());
        assert_eq!(stranger.discord_id.as_deref(), Some(DISCORD));
        assert_eq!(stranger.lock_key(), format!("discord::{DISCORD}"));
    }

    #[test]
    fn reads_what_noprofit_says_about_a_discord_account() {
        let user = uuid::Uuid::new_v4();
        let linked = from_gateway(
            DISCORD,
            &json!({
                "account_id": 7,
                "linked": true,
                "name": "Ann",
                "unlinked_percent": 50,
                "limits": {"short": {"used": 10, "limit": 10, "percent": 100, "active": true, "resets_at": "2026-10-24T15:00:00Z", "window_seconds": 18000}},
                "blocked": "short",
            }),
            Some(user),
            NeoPlan::Plus,
            125,
        );
        assert!(linked.linked);
        assert_eq!(linked.user_uuid, Some(user));
        assert_eq!(linked.account_id, Some(7));
        assert_eq!(linked.account_name.as_deref(), Some("Ann"));
        assert_eq!(linked.plan, NeoPlan::Plus);
        assert_eq!(linked.blocked.as_deref(), Some("short"));
        assert_eq!(linked.resets_at("short"), Some("2026-10-24T15:00:00Z"));
        assert_eq!(linked.resets_at("long"), None);

        // an unlinked account is free, at its own share, with no panel user even if one was guessed
        let stranger = from_gateway(
            DISCORD,
            &json!({"account_id": null, "linked": false, "unlinked_percent": 40, "blocked": null}),
            Some(user),
            NeoPlan::Plusplus,
            156,
        );
        assert!(!stranger.linked);
        assert_eq!(stranger.user_uuid, None);
        assert_eq!(stranger.plan, NeoPlan::Free);
        assert_eq!(stranger.usage_percent, 100);
        assert_eq!(stranger.unlinked_percent, 40);
        assert_eq!(stranger.blocked, None);
        assert!(stranger.limits.is_none());
    }
}
