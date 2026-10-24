//! plugged.host: who is in the Discord server, their boosts and the Discord tier
//! (docs/features/discord.md).
//!
//! Without noprofit.space the `sync_discord_members` task asks Discord about every linked
//! account in turn (the least recently checked first) and [`record`]s the answer: membership,
//! boosting, the boost levels to pay. Only Discord saying "no such member" counts as having left;
//! an error changes nothing. Leaving (or unlinking) records an allowance cut of the Discord tier.

use super::{Context, REASON_DISCORD, TYPE_BOOST, rest};
use crate::{
    allowance::Resources,
    economy::{self, Currency},
};

/// Linked accounts checked per run of the task.
const PER_RUN: i64 = 250;

/// The boost level of a member: 0 while not boosting, 2 with the two-boost role or when two were
/// recorded (`/setboost`), else 1. Discord does not say how often a member boosts.
pub fn boost_level(boosting: bool, has_two_role: bool, recorded: i16) -> i16 {
    if !boosting {
        0
    } else if has_two_role || recorded >= 2 {
        2
    } else {
        1
    }
}

/// What [`record`] wrote.
#[derive(Clone, Copy, Debug, Default)]
pub struct Recorded {
    pub in_guild: bool,
    pub level: i16,
    /// Credits paid for new boost levels.
    pub paid: i64,
}

/// Writes what Discord said about a member (`None`: not in the server) and, for a linked account,
/// pays new boost levels and cuts the allowance of one that lost the Discord tier.
pub async fn record(
    state: &crate::State,
    context: &Context,
    discord_id: &str,
    user_uuid: Option<uuid::Uuid>,
    member: Option<&rest::Member>,
) -> Result<Recorded, anyhow::Error> {
    let had_tier =
        match user_uuid {
            Some(user_uuid) => sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM discord_members WHERE user_uuid = $1 AND in_guild)",
            )
            .bind(user_uuid)
            .fetch_one(state.database.write())
            .await?,
            None => false,
        };

    let mut transaction = state.database.write().begin().await?;
    let previous: Option<(Option<uuid::Uuid>, bool, i16)> = sqlx::query_as(
        "SELECT user_uuid, in_guild, boosts FROM discord_members WHERE discord_id = $1 FOR UPDATE",
    )
    .bind(discord_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let recorded = previous.map_or(0, |(_, _, boosts)| boosts);
    // the Discord account was another panel account's, which loses the tier with it
    let previous_owner = previous.and_then(|(owner, in_guild, _)| {
        owner.filter(|owner| in_guild && user_uuid.is_some_and(|user_uuid| user_uuid != *owner))
    });

    let two_role = context.settings.boost_two_role_id.as_str();
    let has_two_role = !two_role.is_empty()
        && member.is_some_and(|member| member.roles.iter().any(|role| role == two_role));
    let boosting = member.is_some_and(|member| member.premium_since.is_some());
    let level = boost_level(boosting, has_two_role, recorded);
    let user = member.and_then(|member| member.user.as_ref());

    if let Some(user_uuid) = user_uuid {
        // one Discord account per panel account: an earlier one is not theirs any more
        sqlx::query(
            "UPDATE discord_members SET user_uuid = NULL WHERE user_uuid = $1 AND discord_id <> $2",
        )
        .bind(user_uuid)
        .bind(discord_id)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "INSERT INTO discord_members
            (discord_id, user_uuid, username, avatar, in_guild, premium_since, boosts, checked)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT (discord_id) DO UPDATE SET
            user_uuid = COALESCE(EXCLUDED.user_uuid, discord_members.user_uuid),
            username = COALESCE(EXCLUDED.username, discord_members.username),
            avatar = COALESCE(EXCLUDED.avatar, discord_members.avatar),
            in_guild = EXCLUDED.in_guild,
            premium_since = EXCLUDED.premium_since,
            boosts = EXCLUDED.boosts,
            checked = NOW()",
    )
    .bind(discord_id)
    .bind(user_uuid)
    .bind(
        user.and_then(|user| user.display_name())
            .map(|name| truncate(name, 64)),
    )
    .bind(
        user.and_then(|user| user.avatar.as_deref())
            .map(|avatar| truncate(avatar, 64)),
    )
    .bind(member.is_some())
    .bind(
        member
            .and_then(|member| member.premium_since)
            .map(|since| since.naive_utc()),
    )
    .bind(level)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let mut paid = 0;
    if let Some(user_uuid) = user_uuid
        && context.pays()
        && level > 0
    {
        paid = pay_boosts(state, context, user_uuid, discord_id, level).await?;
    }

    // the two-boost role belongs to boosters only
    if has_two_role
        && !boosting
        && let Some(rest) = context.rest(state)
        && let Err(err) = rest
            .set_role(&context.settings.guild_id, discord_id, two_role, false)
            .await
    {
        tracing::warn!(discord = %discord_id, "failed to take the two-boost role: {err:#}");
    }

    if had_tier
        && member.is_none()
        && let Some(user_uuid) = user_uuid
        && context.standalone()
    {
        crate::allowance_cuts::record(
            &state.database,
            user_uuid,
            context.settings.tier(),
            REASON_DISCORD,
        )
        .await?;
    }
    if let Some(previous_owner) = previous_owner
        && context.standalone()
    {
        crate::allowance_cuts::record(
            &state.database,
            previous_owner,
            context.settings.tier(),
            REASON_DISCORD,
        )
        .await?;
    }

    Ok(Recorded {
        in_guild: member.is_some(),
        level,
        paid,
    })
}

/// Pays the boost levels up to `level` that this Discord account and this panel account were not
/// paid before, each once, ever.
pub async fn pay_boosts(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
    discord_id: &str,
    level: i16,
) -> Result<i64, anyhow::Error> {
    let mut paid = 0;

    for level in 1..=level.min(2) {
        let credits = context.settings.boost_credits(level);
        let mut transaction = state.database.write().begin().await?;
        let claimed: Option<uuid::Uuid> = sqlx::query_scalar(
            "INSERT INTO discord_boost_payouts (discord_id, user_uuid, level, credits)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            RETURNING uuid",
        )
        .bind(discord_id)
        .bind(user_uuid)
        .bind(level)
        .bind(credits)
        .fetch_optional(&mut *transaction)
        .await?;
        if claimed.is_none() {
            continue;
        }

        if credits > 0 {
            economy::change(
                &mut transaction,
                Currency::Credits,
                user_uuid,
                credits,
                TYPE_BOOST,
                &format!("Boost level {level}"),
                serde_json::json!({ "discord_id": discord_id, "level": level }),
            )
            .await?;
        }
        transaction.commit().await?;
        paid += credits;
    }

    Ok(paid)
}

/// A member left (the gateway said so): not in the server any more. Unknown Discord accounts are
/// not written.
pub async fn left(
    state: &crate::State,
    context: &Context,
    discord_id: &str,
) -> Result<(), anyhow::Error> {
    let row: Option<(Option<uuid::Uuid>, bool)> = sqlx::query_as(
        "UPDATE discord_members SET in_guild = FALSE, premium_since = NULL, boosts = 0, checked = NOW()
        FROM (SELECT m.discord_id, m.in_guild AS was FROM discord_members m
            WHERE m.discord_id = $1 FOR UPDATE) prev
        WHERE discord_members.discord_id = prev.discord_id
        RETURNING discord_members.user_uuid, prev.was",
    )
    .bind(discord_id)
    .fetch_optional(state.database.write())
    .await?;

    if let Some((Some(user_uuid), true)) = row
        && context.standalone()
    {
        crate::allowance_cuts::record(
            &state.database,
            user_uuid,
            context.settings.tier(),
            REASON_DISCORD,
        )
        .await?;
    }

    Ok(())
}

/// Checks one account's Discord membership now (the page's check, the gateway's events go
/// through [`record`] directly). `None`: Discord gave no answer.
pub async fn check_user(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
) -> Result<Option<Recorded>, anyhow::Error> {
    let Some(rest) = context.rest(state) else {
        return Ok(None);
    };
    let Some(discord_id) = super::linked_discord_id(state, context, user_uuid).await? else {
        return Ok(None);
    };

    match rest.member(&context.settings.guild_id, &discord_id).await {
        rest::Lookup::Found(member) => Ok(Some(
            record(state, context, &discord_id, Some(user_uuid), Some(&member)).await?,
        )),
        rest::Lookup::Absent => Ok(Some(
            record(state, context, &discord_id, Some(user_uuid), None).await?,
        )),
        rest::Lookup::Unknown => Ok(None),
    }
}

/// The `sync_discord_members` task: forgets Discord accounts that were unlinked, then checks the
/// linked ones, the least recently checked first.
pub async fn sync(state: &crate::State) -> Result<(), anyhow::Error> {
    let context = Context::load(state).await?;
    if !context.standalone() {
        return Ok(());
    }
    let Some(rest) = context.rest(state) else {
        return Ok(());
    };
    let provider = context.settings.oauth_provider_uuid;

    // unlinked (or linked to someone else): the tier is gone with the link
    let unlinked: Vec<(String, uuid::Uuid, bool)> = sqlx::query_as(
        "UPDATE discord_members SET user_uuid = NULL
        FROM (
            SELECT m.discord_id, m.user_uuid AS was_user, m.in_guild AS was_in
            FROM discord_members m
            WHERE m.user_uuid IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM user_oauth_links l
                WHERE l.oauth_provider_uuid = $1
                    AND l.identifier = m.discord_id
                    AND l.user_uuid = m.user_uuid
            )
            FOR UPDATE
        ) prev
        WHERE discord_members.discord_id = prev.discord_id
        RETURNING prev.discord_id, prev.was_user, prev.was_in",
    )
    .bind(provider)
    .fetch_all(state.database.write())
    .await?;
    for (_, user_uuid, was_in) in unlinked {
        if was_in {
            crate::allowance_cuts::record(
                &state.database,
                user_uuid,
                context.settings.tier(),
                REASON_DISCORD,
            )
            .await?;
        }
    }

    let linked: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT user_oauth_links.user_uuid, user_oauth_links.identifier FROM user_oauth_links
        LEFT JOIN discord_members ON discord_members.discord_id = user_oauth_links.identifier
        WHERE user_oauth_links.oauth_provider_uuid = $1
        ORDER BY discord_members.checked ASC NULLS FIRST
        LIMIT $2",
    )
    .bind(provider)
    .bind(PER_RUN)
    .fetch_all(state.database.read())
    .await?;

    for (user_uuid, discord_id) in linked {
        let result = match rest.member(&context.settings.guild_id, &discord_id).await {
            rest::Lookup::Found(member) => {
                record(state, &context, &discord_id, Some(user_uuid), Some(&member)).await
            }
            rest::Lookup::Absent => {
                record(state, &context, &discord_id, Some(user_uuid), None).await
            }
            rest::Lookup::Unknown => continue,
        };
        if let Err(err) = result {
            tracing::warn!(user = %user_uuid, "failed to record a Discord member: {err:#}");
        }
    }

    Ok(())
}

/// What `/setboost` came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoostsOutcome {
    /// The member's boost level now: the count set while Discord says they boost, else 0.
    Level(i16),
    /// Discord says they are not in the server.
    NotInServer,
    /// Discord gave no answer; the count is kept for the next check.
    Unknown,
}

/// What [`set_boosts`] did.
#[derive(Clone, Copy, Debug)]
pub struct BoostsSet {
    pub user_uuid: Option<uuid::Uuid>,
    pub outcome: BoostsOutcome,
    /// Credits paid for new boost levels.
    pub paid: i64,
}

/// Sets a member's boost count by hand (`/setboost`): Discord says whether someone boosts, not how
/// often. The two-boost role follows the count while they boost, and a linked account is paid the
/// levels it reaches.
pub async fn set_boosts(
    state: &crate::State,
    context: &Context,
    discord_id: &str,
    count: i16,
) -> Result<BoostsSet, anyhow::Error> {
    let count = count.clamp(0, 2);
    let Some(rest) = context.rest(state) else {
        anyhow::bail!("the bot token is not set");
    };
    let guild_id = context.settings.guild_id.as_str();
    let user_uuid = super::user_for_discord_id(state, context, discord_id).await?;

    sqlx::query(
        "INSERT INTO discord_members (discord_id, boosts) VALUES ($1, $2)
        ON CONFLICT (discord_id) DO UPDATE SET boosts = EXCLUDED.boosts",
    )
    .bind(discord_id)
    .bind(count)
    .execute(state.database.write())
    .await?;

    let mut set = BoostsSet {
        user_uuid,
        outcome: BoostsOutcome::Unknown,
        paid: 0,
    };
    match rest.member(guild_id, discord_id).await {
        rest::Lookup::Found(mut member) => {
            let role = context.settings.boost_two_role_id.as_str();
            if !role.is_empty() {
                // the two-boost role belongs to boosters only
                let two = count >= 2 && member.premium_since.is_some();
                if two != member.roles.iter().any(|existing| existing == role)
                    && let Err(err) = rest.set_role(guild_id, discord_id, role, two).await
                {
                    tracing::warn!(discord = %discord_id, "failed to change the two-boost role: {err:#}");
                }
                // what was asked counts, also while Discord's answer does not show it yet
                member.roles.retain(|existing| existing != role);
                if two {
                    member.roles.push(role.to_string());
                }
            }

            let recorded = record(state, context, discord_id, user_uuid, Some(&member)).await?;
            set.outcome = BoostsOutcome::Level(recorded.level);
            set.paid = recorded.paid;
        }
        rest::Lookup::Absent => {
            record(state, context, discord_id, user_uuid, None).await?;
            set.outcome = BoostsOutcome::NotInServer;
        }
        rest::Lookup::Unknown => {}
    }

    Ok(set)
}

/// How much the Discord tier went down, per resource (0 adds nothing, unlike an allowance's 0).
pub fn tier_decrease(before: Resources, after: Resources) -> Resources {
    Resources::new(
        (before.cpu - after.cpu).max(0),
        (before.memory - after.memory).max(0),
        (before.disk - after.disk).max(0),
    )
}

/// The Discord tier went down (the settings changed, or the panel stopped linking itself): every
/// member who owns servers gives the difference back (shared/src/allowance_cuts.rs).
pub async fn record_tier_cut(
    database: &crate::database::Database,
    provider: Option<uuid::Uuid>,
    before: Resources,
    after: Resources,
) -> Result<(), sqlx::Error> {
    let decrease = tier_decrease(before, after);
    let Some(provider) = provider else {
        return Ok(());
    };
    if decrease == Resources::default() {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO user_allowance_cuts (user_uuid, reason, cpu, memory, disk)
        SELECT DISTINCT l.user_uuid, $2, $3, $4, $5 FROM user_oauth_links l
        JOIN discord_members m ON m.discord_id = l.identifier
        WHERE l.oauth_provider_uuid = $1 AND m.in_guild
            AND EXISTS (
                SELECT 1 FROM servers WHERE servers.owner_uuid = l.user_uuid
                    AND NOT EXISTS (SELECT 1 FROM devboxes WHERE devboxes.server_uuid = servers.uuid)
            )",
    )
    .bind(provider)
    .bind(REASON_DISCORD)
    .bind(decrease.cpu)
    .bind(decrease.memory)
    .bind(decrease.disk)
    .execute(database.write())
    .await?;

    Ok(())
}

fn truncate(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_levels() {
        assert_eq!(boost_level(false, true, 2), 0, "not boosting is level 0");
        assert_eq!(boost_level(true, false, 0), 1);
        assert_eq!(boost_level(true, true, 0), 2);
        assert_eq!(
            boost_level(true, false, 2),
            2,
            "a recorded second boost stays"
        );
        assert_eq!(boost_level(true, false, 1), 1);
    }

    #[test]
    fn only_a_smaller_tier_is_cut() {
        assert_eq!(
            tier_decrease(Resources::new(100, 2048, 0), Resources::new(50, 4096, 0)),
            Resources::new(50, 0, 0)
        );
        assert_eq!(
            tier_decrease(Resources::new(100, 2048, 1024), Resources::default()),
            Resources::new(100, 2048, 1024),
            "turned off: all of it"
        );
    }
}
