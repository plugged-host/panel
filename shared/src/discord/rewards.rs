//! plugged.host: the account's Discord rewards page (docs/features/discord.md).

use super::{Context, LEDGER_TYPES, invites};
use crate::{allowance::Resources, economy::ApiTransaction};
use serde::Serialize;
use utoipa::ToSchema;

/// Ledger rows the page shows.
const HISTORY: i64 = 20;

#[derive(sqlx::FromRow)]
struct MemberRow {
    username: Option<String>,
    avatar: Option<String>,
    in_guild: bool,
    premium_since: Option<chrono::NaiveDateTime>,
    boosts: i16,
    checked: Option<chrono::NaiveDateTime>,
}

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The panel links, checks and pays.
    Panel,
    /// noprofit.space does (the panel shows the Discord account it knows of).
    Noprofit,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiDiscordAccount {
    pub id: String,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiDiscordRules {
    pub credits_per_invite: i64,
    pub min_account_age_days: i32,
    pub invite_maturity_hours: i32,
    pub invite_clawback: bool,
    pub boost_one_credits: i64,
    /// For both boosts together.
    pub boost_two_credits: i64,
    /// What being in the server adds to the allowance (only where it has a limit).
    pub tier: Resources,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiDiscordBoosts {
    pub level: i16,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    /// The levels paid so far.
    pub paid_levels: Vec<i16>,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiDiscordRewards {
    pub source: Source,
    /// The panel pays invite rewards, with credits on (standalone, or into noprofit.space's
    /// balance); boosts are paid here only standalone (`boosts`).
    pub pays: bool,
    pub account: Option<ApiDiscordAccount>,
    /// `None` when nobody checked yet (or nobody can).
    pub in_guild: Option<bool>,
    pub checked: Option<chrono::DateTime<chrono::Utc>>,
    /// Where to link a Discord account: noprofit.space's settings (without it: the OAuth links
    /// page, which the frontend knows).
    pub link_url: Option<String>,
    /// noprofit.space's page of Discord servers and boosts.
    pub noprofit_url: Option<String>,
    /// Empty: no invite links or buttons anywhere.
    pub invite_url: Option<String>,
    pub rules: ApiDiscordRules,
    pub invites: invites::Summary,
    /// Standalone only (noprofit.space pays boosts).
    pub boosts: Option<ApiDiscordBoosts>,
    /// Credits the panel paid for Discord, taken-back and owed ones counted off (with
    /// noprofit.space: what it applied of the changes sent, and the ones not sent yet).
    pub earned: i64,
    pub history: Vec<ApiTransaction>,
}

/// What a player's Discord rewards page shows.
pub async fn page(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<ApiDiscordRewards, anyhow::Error> {
    let context = Context::load(state).await?;
    let settings = &context.settings;

    let mut account = None;
    if context.connected {
        if let Some(discord) = crate::noprofit::Link::by_user_uuid(&state.database, user_uuid)
            .await?
            .and_then(|link| link.discord)
            && let Some(id) = discord.get("id").and_then(|id| id.as_str())
            && super::is_snowflake(id)
        {
            account = Some(ApiDiscordAccount {
                id: id.to_string(),
                username: discord
                    .get("username")
                    .and_then(|name| name.as_str())
                    .map(str::to_string),
                avatar_url: super::avatar_url(id, discord.get("avatar").and_then(|a| a.as_str())),
            });
        }
    } else if let Some(id) = super::linked_discord_id(state, &context, user_uuid).await? {
        account = Some(ApiDiscordAccount {
            id,
            username: None,
            avatar_url: None,
        });
    }

    let member = match &account {
        Some(account) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT username, avatar, in_guild, premium_since, boosts, checked
                FROM discord_members WHERE discord_id = $1",
            )
            .bind(&account.id)
            .fetch_optional(state.database.read())
            .await?
        }
        None => None,
    };
    if let (Some(account), Some(member)) = (&mut account, &member) {
        if account.username.is_none() {
            account.username = member.username.clone();
        }
        if account.avatar_url.is_none() {
            account.avatar_url = super::avatar_url(&account.id, member.avatar.as_deref());
        }
    }
    let checked = member
        .as_ref()
        .and_then(|member| member.checked)
        .map(|checked| checked.and_utc());
    let in_guild = member
        .as_ref()
        .filter(|member| member.checked.is_some())
        .map(|member| member.in_guild);

    let invites = match &account {
        Some(account) => invites::summary(&state.database, &account.id).await?,
        None => invites::Summary::default(),
    };

    let boosts = match (&account, &member) {
        (Some(account), Some(member)) if context.standalone() => {
            let paid_levels: Vec<i16> = sqlx::query_scalar(
                "SELECT level FROM discord_boost_payouts
                WHERE discord_id = $1 OR user_uuid = $2 ORDER BY level",
            )
            .bind(&account.id)
            .bind(user_uuid)
            .fetch_all(state.database.read())
            .await?;
            let mut paid_levels = paid_levels;
            paid_levels.dedup();

            Some(ApiDiscordBoosts {
                level: member.boosts,
                since: member.premium_since.map(|since| since.and_utc()),
                paid_levels,
            })
        }
        (Some(_), None) if context.standalone() => Some(ApiDiscordBoosts {
            level: 0,
            since: None,
            paid_levels: Vec::new(),
        }),
        _ => None,
    };

    let types: Vec<String> = LEDGER_TYPES.iter().map(|kind| kind.to_string()).collect();
    let earned: i64 = sqlx::query_scalar(if context.connected {
        "SELECT COALESCE(SUM(COALESCE(applied, amount)), 0)::BIGINT
        FROM user_noprofit_credit_changes
        WHERE user_uuid = $1 AND type = ANY($2)"
    } else {
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM user_credit_transactions
        WHERE user_uuid = $1 AND type = ANY($2)"
    })
    .bind(user_uuid)
    .bind(&types)
    .fetch_one(state.database.read())
    .await?;
    let history = sqlx::query_as::<_, (i64, i64, String, String, chrono::NaiveDateTime)>(
        "SELECT amount, balance_after, type, description, created FROM user_credit_transactions
        WHERE user_uuid = $1 AND type = ANY($2) ORDER BY created DESC LIMIT $3",
    )
    .bind(user_uuid)
    .bind(&types)
    .bind(HISTORY)
    .fetch_all(state.database.read())
    .await?
    .into_iter()
    .map(
        |(amount, balance_after, kind, description, created)| ApiTransaction {
            amount,
            balance_after,
            kind: kind.into(),
            description,
            created: Some(created.and_utc()),
        },
    )
    .collect();

    Ok(ApiDiscordRewards {
        source: if context.connected {
            Source::Noprofit
        } else {
            Source::Panel
        },
        pays: context.pays_invites(),
        account,
        in_guild,
        checked,
        link_url: context
            .accounts_url
            .as_ref()
            .map(|url| format!("{url}/settings")),
        noprofit_url: context
            .accounts_url
            .as_ref()
            .map(|url| format!("{url}/discord")),
        invite_url: Some(settings.invite_url.to_string()).filter(|url| !url.is_empty()),
        rules: ApiDiscordRules {
            credits_per_invite: settings.credits_per_invite.max(0) as i64,
            min_account_age_days: settings.min_account_age_days.max(0),
            invite_maturity_hours: settings.invite_maturity_hours.max(0),
            invite_clawback: settings.invite_clawback,
            boost_one_credits: settings.boost_credits(1),
            boost_two_credits: settings.boost_credits(1) + settings.boost_credits(2),
            tier: if context.standalone() {
                settings.tier()
            } else {
                Resources::default()
            },
        },
        invites,
        boosts,
        earned,
        history,
    })
}

/// The page's "check for new rewards": the account's membership and boosts now, and its matured
/// invites; with noprofit.space a fresh snapshot (and the membership, for the subdomain check).
pub async fn check(state: &crate::State, user_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let context = Context::load(state).await?;
    if !context.settings.active() {
        return Ok(());
    }

    // an unreachable noprofit.space leaves the snapshot as it was
    if context.connected
        && let Err(err) = crate::noprofit::refresh_user(state, user_uuid).await
    {
        tracing::warn!(user = %user_uuid, "failed to refresh the noprofit.space snapshot: {err:#}");
    }
    super::members::check_user(state, &context, user_uuid).await?;

    if context.pays_invites()
        && let Some(discord_id) = super::linked_discord_id(state, &context, user_uuid).await?
    {
        invites::settle(state, &context, Some(&discord_id)).await?;
    }

    Ok(())
}
