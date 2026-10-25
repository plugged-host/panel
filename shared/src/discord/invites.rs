//! plugged.host: invite rewards (docs/features/discord.md).
//!
//! The invite and boost bot sees a member join and works out whose invite they used
//! ([`attribute`]); [`record_join`] writes it to `discord_invites`. A join pays when it is old
//! enough (the anti-alt minimum, from the account's id), has been held for the maturity hours and
//! the member is still there: [`settle`] checks that with Discord right before paying. A member
//! who leaves ([`record_leave`]) stops counting; one whose invite was paid is taken back when
//! clawback is on, from the balance and, for what it cannot cover, from the inviter's next invite
//! rewards (`discord_members.invite_debt`). [`reconcile`] looks for departures the bot missed;
//! only Discord saying "no such member" counts.
//!
//! With noprofit.space connected (its account sync on) the rewards, clawbacks and owed credits go
//! to the account's balance there: queued with the change ([`crate::noprofit::queue_credit_change`])
//! and sent right after, like Pelican's credit outbox. noprofit.space pays the boosts then.

use super::{Context, TYPE_INVITE, TYPE_INVITE_CLAWBACK, TYPE_INVITE_DEBT, rest};
use crate::economy::{self, Currency};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use utoipa::ToSchema;

/// Joins checked by a run of [`reconcile`], and paid by one of [`settle`].
const RECONCILE_PER_RUN: i64 = 200;
const SETTLE_PER_RUN: i64 = 100;

/// The invite a join used: the first code whose uses went up since `known` (code → uses), with
/// its inviter. A code not in `known` counts from 0.
pub fn attribute<'a>(
    known: &HashMap<String, i64>,
    current: &'a [rest::Invite],
) -> Option<&'a rest::Invite> {
    current
        .iter()
        .find(|invite| invite.uses > known.get(&invite.code).copied().unwrap_or(0))
}

/// Whether a join pays at all: an account at least `min_days` old that is not the inviter.
pub fn eligible(
    inviter: &str,
    joined: &str,
    account_created: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
    min_days: i32,
) -> bool {
    inviter != joined && now - account_created >= chrono::Duration::days(min_days.max(0) as i64)
}

/// Where the credits of an invite go.
enum Ledger {
    /// The panel's balance (`users.credits`).
    Panel,
    /// The account's balance on noprofit.space.
    Noprofit { account_id: i64 },
}

impl Ledger {
    /// The inviter's ledger in `transaction`, with the balance a clawback may take from (with
    /// noprofit.space: what it last said plus the changes not sent yet). `None`: the inviter's
    /// noprofit.space account is gone.
    async fn of(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        context: &Context,
        user_uuid: uuid::Uuid,
    ) -> Result<Option<(Self, i64)>, sqlx::Error> {
        if context.connected {
            return Ok(crate::noprofit::pending_balance(transaction, user_uuid)
                .await?
                .map(|(account_id, balance)| (Self::Noprofit { account_id }, balance)));
        }

        let balance: i64 =
            sqlx::query_scalar("SELECT credits FROM users WHERE uuid = $1 FOR UPDATE")
                .bind(user_uuid)
                .fetch_optional(&mut **transaction)
                .await?
                .unwrap_or(0);

        Ok(Some((Self::Panel, balance)))
    }

    async fn change(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_uuid: uuid::Uuid,
        amount: i64,
        kind: &str,
        description: &str,
        data: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        match self {
            Self::Panel => {
                economy::change(
                    transaction,
                    Currency::Credits,
                    user_uuid,
                    amount,
                    kind,
                    description,
                    data,
                )
                .await?;
            }
            // nothing to send
            Self::Noprofit { .. } if amount == 0 => {}
            Self::Noprofit { account_id } => {
                crate::noprofit::queue_credit_change(
                    transaction,
                    user_uuid,
                    *account_id,
                    amount,
                    kind,
                    description,
                    data,
                )
                .await?;
            }
        }

        Ok(())
    }
}

/// Splits a clawback into what the balance gives back and what is owed.
pub fn clawback(balance: i64, credits: i64) -> (i64, i64) {
    let taken = balance.clamp(0, credits.max(0));

    (taken, credits.max(0) - taken)
}

/// Records a join through `inviter`'s invite. Without an inviter (a vanity link, or the bot did
/// not see the code) a member who joined before keeps their earlier inviter. `false`: nobody to
/// attribute it to.
pub async fn record_join(
    state: &crate::State,
    context: &Context,
    inviter: Option<&str>,
    joined: &str,
    code: Option<&str>,
) -> Result<bool, anyhow::Error> {
    let now = chrono::Utc::now().naive_utc();
    let account_created = super::created_at(joined)
        .map(|created| created.naive_utc())
        .unwrap_or(now);
    let matures =
        now + chrono::Duration::hours(context.settings.invite_maturity_hours.max(0) as i64);

    let mut transaction = state.database.write().begin().await?;
    #[allow(clippy::type_complexity)]
    let existing: Option<(
        String,
        Option<String>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
    )> = sqlx::query_as(
        "SELECT inviter_discord_id, invite_code, rewarded, revoked FROM discord_invites
        WHERE joined_discord_id = $1 FOR UPDATE",
    )
    .bind(joined)
    .fetch_optional(&mut *transaction)
    .await?;

    let inviter = match &existing {
        Some((earlier, earlier_code, rewarded, revoked)) => {
            // a paid invite that was not taken back stays its inviter's
            let kept = rewarded.is_some() && revoked.is_none();
            match inviter {
                Some(inviter) if !kept => (inviter.to_string(), code.map(str::to_string)),
                _ => (earlier.clone(), earlier_code.clone()),
            }
        }
        None => match inviter {
            Some(inviter) => (inviter.to_string(), code.map(str::to_string)),
            None => return Ok(false),
        },
    };
    let (inviter, code) = inviter;
    let eligible = eligible(
        &inviter,
        joined,
        account_created,
        now,
        context.settings.min_account_age_days,
    );

    if existing.is_some() {
        sqlx::query(
            "UPDATE discord_invites SET
                inviter_discord_id = $2, invite_code = $3, joined = $4, eligible = $5, matures = $6,
                last_seen = $4,
                rejoin_count = rejoin_count + CASE WHEN departed IS NOT NULL THEN 1 ELSE 0 END,
                departed = NULL,
                rewarded = CASE WHEN revoked IS NOT NULL THEN NULL ELSE rewarded END,
                credits = CASE WHEN revoked IS NOT NULL THEN 0 ELSE credits END,
                revoked_credits = CASE WHEN revoked IS NOT NULL THEN 0 ELSE revoked_credits END,
                revoked = NULL
            WHERE joined_discord_id = $1",
        )
        .bind(joined)
        .bind(&inviter)
        .bind(&code)
        .bind(now)
        .bind(eligible)
        .bind(matures)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO discord_invites
                (inviter_discord_id, joined_discord_id, invite_code, account_created, joined,
                eligible, matures, last_seen)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $5)",
        )
        .bind(&inviter)
        .bind(joined)
        .bind(&code)
        .bind(account_created)
        .bind(now)
        .bind(eligible)
        .bind(matures)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;

    // no hold: paid right away
    if context.settings.invite_maturity_hours <= 0 {
        settle(state, context, Some(&inviter)).await?;
    }

    Ok(true)
}

/// What [`record_leave`] did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Leave {
    /// The member had joined through someone's invite.
    pub tracked: bool,
    /// Credits taken back from the inviter's balance.
    pub taken: i64,
    /// Credits the balance could not cover, now owed.
    pub owed: i64,
}

/// A member who joined through an invite left: it stops counting, and a paid one is taken back
/// (once, however often the departure is seen) while clawback is on.
pub async fn record_leave(
    state: &crate::State,
    context: &Context,
    joined: &str,
) -> Result<Leave, anyhow::Error> {
    let mut transaction = state.database.write().begin().await?;
    #[allow(clippy::type_complexity)]
    let row: Option<(
        uuid::Uuid,
        String,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        i64,
    )> = sqlx::query_as(
        "SELECT uuid, inviter_discord_id, rewarded, revoked, departed, credits FROM discord_invites
        WHERE joined_discord_id = $1 FOR UPDATE",
    )
    .bind(joined)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((uuid, inviter, rewarded, revoked, departed, credits)) = row else {
        return Ok(Leave::default());
    };

    let mut leave = Leave {
        tracked: true,
        ..Default::default()
    };
    if departed.is_some() {
        return Ok(leave);
    }

    sqlx::query(
        "UPDATE discord_invites SET departed = NOW(), leave_count = leave_count + 1 WHERE uuid = $1",
    )
    .bind(uuid)
    .execute(&mut *transaction)
    .await?;

    let mut queued = false;
    if rewarded.is_some()
        && revoked.is_none()
        && context.settings.invite_clawback
        && context.pays_invites()
    {
        let user_uuid = super::user_for_discord_id(state, context, &inviter).await?;
        let mut taken = 0;
        if let Some(user_uuid) = user_uuid
            && let Some((ledger, balance)) =
                Ledger::of(&mut transaction, context, user_uuid).await?
        {
            let (from_balance, owed) = clawback(balance, credits);
            taken = from_balance;

            ledger
                .change(
                    &mut transaction,
                    user_uuid,
                    -taken,
                    TYPE_INVITE_CLAWBACK,
                    &if owed > 0 {
                        format!("An invited member left, {owed} credits owed")
                    } else {
                        "An invited member left".to_string()
                    },
                    serde_json::json!({ "joined_discord_id": joined, "credits": credits, "owed": owed }),
                )
                .await?;
            queued = matches!(ledger, Ledger::Noprofit { .. });
        }
        let owed = credits.max(0) - taken;
        if owed > 0 {
            sqlx::query(
                "INSERT INTO discord_members (discord_id, invite_debt) VALUES ($1, $2)
                ON CONFLICT (discord_id) DO UPDATE
                SET invite_debt = discord_members.invite_debt + EXCLUDED.invite_debt",
            )
            .bind(&inviter)
            .bind(owed)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            "UPDATE discord_invites SET revoked = NOW(), revoked_credits = $2 WHERE uuid = $1",
        )
        .bind(uuid)
        .bind(taken)
        .execute(&mut *transaction)
        .await?;

        leave.taken = taken;
        leave.owed = owed;
    }
    transaction.commit().await?;

    if queued {
        send_credit_changes(state).await;
    }

    Ok(leave)
}

/// Sends the credit changes just queued for noprofit.space; what fails goes with the next
/// account sync.
async fn send_credit_changes(state: &crate::State) {
    if let Err(err) = crate::noprofit::push_credit_changes(state).await {
        tracing::warn!("failed to send invite credits to noprofit.space: {err:#}");
    }
}

/// Pays the held invites that matured (of one inviter, or of everyone), after making sure with
/// Discord that their members are still there. Owed credits come off first. Returns what was
/// paid.
pub async fn settle(
    state: &crate::State,
    context: &Context,
    inviter: Option<&str>,
) -> Result<i64, anyhow::Error> {
    if !context.pays_invites() {
        return Ok(0);
    }
    let Some(rest) = context.rest(state) else {
        return Ok(0);
    };
    let per_invite = context.settings.credits_per_invite.max(0) as i64;

    // only inviters who linked a panel account (with noprofit.space: whose account there has
    // their Discord account): the others' joins wait, and must not fill a run
    let rows: Vec<(uuid::Uuid, String, String)> = sqlx::query_as(
        "SELECT i.uuid, i.inviter_discord_id, i.joined_discord_id FROM discord_invites i
        WHERE i.rewarded IS NULL AND i.eligible AND i.departed IS NULL AND i.matures <= NOW()
            AND ($1::varchar IS NULL OR i.inviter_discord_id = $1)
            AND CASE WHEN $4::boolean THEN EXISTS (
                SELECT 1 FROM user_noprofit_accounts n
                WHERE n.discord->>'id' = i.inviter_discord_id AND n.deleted IS NULL
            ) ELSE EXISTS (
                SELECT 1 FROM user_oauth_links l
                WHERE l.oauth_provider_uuid = $3 AND l.identifier = i.inviter_discord_id
            ) END
        ORDER BY i.matures
        LIMIT $2",
    )
    .bind(inviter)
    .bind(SETTLE_PER_RUN)
    .bind(context.settings.oauth_provider_uuid)
    .bind(context.connected)
    .fetch_all(state.database.read())
    .await?;

    let mut by_inviter: BTreeMap<String, Vec<(uuid::Uuid, String)>> = BTreeMap::new();
    for (uuid, inviter, joined) in rows {
        by_inviter.entry(inviter).or_default().push((uuid, joined));
    }

    let mut total = 0;
    let mut queued = false;
    for (inviter, joins) in by_inviter {
        // waits for its inviter to link a panel account
        let Some(user_uuid) = super::user_for_discord_id(state, context, &inviter).await? else {
            continue;
        };

        let mut present = Vec::with_capacity(joins.len());
        for (uuid, joined) in joins {
            match rest.member(&context.settings.guild_id, &joined).await {
                rest::Lookup::Found(_) => present.push(uuid),
                rest::Lookup::Absent => {
                    record_leave(state, context, &joined).await?;
                    super::members::left(state, context, &joined).await?;
                }
                rest::Lookup::Unknown => {}
            }
        }
        if present.is_empty() {
            continue;
        }

        let mut transaction = state.database.write().begin().await?;
        let claimed: Vec<uuid::Uuid> = sqlx::query_scalar(
            "UPDATE discord_invites SET rewarded = NOW(), credits = $2, last_seen = NOW()
            WHERE uuid = ANY($1) AND rewarded IS NULL AND departed IS NULL
            RETURNING uuid",
        )
        .bind(&present)
        .bind(per_invite)
        .fetch_all(&mut *transaction)
        .await?;
        let count = claimed.len() as i64;
        let gross = count * per_invite;
        if gross <= 0 {
            transaction.commit().await?;
            continue;
        }
        // its noprofit.space account went away meanwhile: they wait (the claim is rolled back)
        let Some((ledger, _)) = Ledger::of(&mut transaction, context, user_uuid).await? else {
            continue;
        };

        ledger
            .change(
                &mut transaction,
                user_uuid,
                gross,
                TYPE_INVITE,
                &if count == 1 {
                    "1 invited member".to_string()
                } else {
                    format!("{count} invited members")
                },
                serde_json::json!({ "discord_id": inviter, "invites": count, "per_invite": per_invite }),
            )
            .await?;

        let debt: i64 = sqlx::query_scalar(
            "SELECT invite_debt FROM discord_members WHERE discord_id = $1 FOR UPDATE",
        )
        .bind(&inviter)
        .fetch_optional(&mut *transaction)
        .await?
        .unwrap_or(0);
        let applied = debt.clamp(0, gross);
        if applied > 0 {
            sqlx::query(
                "UPDATE discord_members SET invite_debt = invite_debt - $2 WHERE discord_id = $1",
            )
            .bind(&inviter)
            .bind(applied)
            .execute(&mut *transaction)
            .await?;
            ledger
                .change(
                    &mut transaction,
                    user_uuid,
                    -applied,
                    TYPE_INVITE_DEBT,
                    "Owed invite credits",
                    serde_json::json!({ "discord_id": inviter, "owed": debt - applied }),
                )
                .await?;
        }
        transaction.commit().await?;

        queued |= matches!(ledger, Ledger::Noprofit { .. });
        total += gross - applied;
    }

    if queued {
        send_credit_changes(state).await;
    }

    Ok(total)
}

/// The `reconcile_discord_invites` task: asks Discord about the members still counted (the least
/// recently seen first) and records the ones it says are gone.
pub async fn reconcile(state: &crate::State) -> Result<(), anyhow::Error> {
    let context = Context::load(state).await?;
    let Some(rest) = context.rest(state) else {
        return Ok(());
    };

    let joined: Vec<String> = sqlx::query_scalar(
        "SELECT joined_discord_id FROM discord_invites WHERE departed IS NULL
        ORDER BY last_seen ASC NULLS FIRST LIMIT $1",
    )
    .bind(RECONCILE_PER_RUN)
    .fetch_all(state.database.read())
    .await?;

    for joined in joined {
        match rest.member(&context.settings.guild_id, &joined).await {
            rest::Lookup::Found(_) => {
                sqlx::query(
                    "UPDATE discord_invites SET last_seen = NOW() WHERE joined_discord_id = $1",
                )
                .bind(&joined)
                .execute(state.database.write())
                .await?;
            }
            rest::Lookup::Absent => {
                record_leave(state, &context, &joined).await?;
                super::members::left(state, &context, &joined).await?;
            }
            rest::Lookup::Unknown => {}
        }
    }

    Ok(())
}

/// The `settle_discord_invites` task.
pub async fn settle_all(state: &crate::State) -> Result<(), anyhow::Error> {
    let context = Context::load(state).await?;
    settle(state, &context, None).await?;

    Ok(())
}

/// One Discord account's invites.
#[derive(ToSchema, Serialize, Clone, Debug, Default)]
pub struct Summary {
    /// Members still in the server whose invite was paid.
    pub paid: i64,
    /// Members still in the server whose reward is held (or waits for its inviter to link).
    pub waiting: i64,
    /// Members who left.
    pub left: i64,
    /// Members whose account was too new when they joined (or who used their own invite).
    pub too_new: i64,
    /// Credits the paid invites brought (taken-back ones not counted).
    pub credits: i64,
    /// When the next held reward is due.
    pub next_payout: Option<chrono::DateTime<chrono::Utc>>,
    /// Credits owed back, taken from the next invite rewards.
    pub owed: i64,
}

pub async fn summary(
    database: &crate::database::Database,
    discord_id: &str,
) -> Result<Summary, sqlx::Error> {
    let row: (i64, i64, i64, i64, i64, Option<chrono::NaiveDateTime>) = sqlx::query_as(
        "SELECT
            COUNT(*) FILTER (WHERE departed IS NULL AND rewarded IS NOT NULL AND revoked IS NULL),
            COUNT(*) FILTER (WHERE departed IS NULL AND rewarded IS NULL AND eligible),
            COUNT(*) FILTER (WHERE departed IS NOT NULL),
            COUNT(*) FILTER (WHERE departed IS NULL AND rewarded IS NULL AND NOT eligible),
            COALESCE(SUM(credits) FILTER (WHERE rewarded IS NOT NULL AND revoked IS NULL), 0)::BIGINT,
            MIN(matures) FILTER (WHERE departed IS NULL AND rewarded IS NULL AND eligible)
        FROM discord_invites WHERE inviter_discord_id = $1",
    )
    .bind(discord_id)
    .fetch_one(database.read())
    .await?;
    let owed: i64 =
        sqlx::query_scalar("SELECT invite_debt FROM discord_members WHERE discord_id = $1")
            .bind(discord_id)
            .fetch_optional(database.read())
            .await?
            .unwrap_or(0);

    Ok(Summary {
        paid: row.0,
        waiting: row.1,
        left: row.2,
        too_new: row.3,
        credits: row.4,
        next_payout: row.5.map(|matures| matures.and_utc()),
        owed,
    })
}

/// A line of the `/topinvites` leaderboard.
#[derive(Clone, Debug)]
pub struct Inviter {
    pub discord_id: String,
    /// Eligible members still in the server.
    pub counted: i64,
    pub paid: i64,
    pub waiting: i64,
    pub left: i64,
    pub credits: i64,
}

pub async fn leaderboard(
    database: &crate::database::Database,
    limit: i64,
) -> Result<Vec<Inviter>, sqlx::Error> {
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT inviter_discord_id,
            COUNT(*) FILTER (WHERE departed IS NULL AND eligible) AS counted,
            COUNT(*) FILTER (WHERE departed IS NULL AND rewarded IS NOT NULL AND revoked IS NULL) AS paid,
            COUNT(*) FILTER (WHERE departed IS NULL AND rewarded IS NULL AND eligible),
            COUNT(*) FILTER (WHERE departed IS NOT NULL),
            COALESCE(SUM(credits) FILTER (WHERE rewarded IS NOT NULL AND revoked IS NULL), 0)::BIGINT
        FROM discord_invites
        GROUP BY inviter_discord_id
        HAVING COUNT(*) FILTER (WHERE departed IS NULL AND eligible) > 0
        ORDER BY counted DESC, paid DESC, inviter_discord_id
        LIMIT $1",
    )
    .bind(limit.clamp(1, 25))
    .fetch_all(database.read())
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(discord_id, counted, paid, waiting, left, credits)| Inviter {
                discord_id,
                counted,
                paid,
                waiting,
                left,
                credits,
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite(code: &str, uses: i64, inviter: &str) -> rest::Invite {
        rest::Invite {
            code: code.into(),
            uses,
            inviter: Some(rest::User {
                id: inviter.into(),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn a_join_is_the_invite_whose_uses_went_up() {
        let known = HashMap::from([("aaa".to_string(), 3), ("bbb".to_string(), 5)]);
        let current = [invite("aaa", 3, "1"), invite("bbb", 6, "2")];
        assert_eq!(
            attribute(&known, &current)
                .and_then(|invite| invite.inviter.as_ref())
                .map(|user| user.id.as_str()),
            Some("2")
        );

        // a code made after the cache counts from 0
        let current = [
            invite("aaa", 3, "1"),
            invite("bbb", 5, "2"),
            invite("ccc", 1, "3"),
        ];
        assert_eq!(
            attribute(&known, &current).map(|invite| invite.code.as_str()),
            Some("ccc")
        );

        // nothing went up: a vanity link, or a use the bot did not see
        let current = [invite("aaa", 3, "1"), invite("bbb", 5, "2")];
        assert!(attribute(&known, &current).is_none());
    }

    #[test]
    fn young_accounts_and_own_invites_pay_nothing() {
        let now = chrono::DateTime::from_timestamp(1_800_000_000, 0)
            .unwrap()
            .naive_utc();
        let old = now - chrono::Duration::days(40);
        let young = now - chrono::Duration::days(3);

        assert!(eligible("1", "2", old, now, 30));
        assert!(!eligible("1", "2", young, now, 30));
        assert!(!eligible("2", "2", old, now, 30));
        assert!(eligible("1", "2", young, now, 0));
    }

    #[test]
    fn a_clawback_takes_what_the_balance_has() {
        assert_eq!(clawback(500, 50), (50, 0));
        assert_eq!(clawback(20, 50), (20, 30));
        assert_eq!(clawback(0, 50), (0, 50));
        assert_eq!(clawback(-5, 50), (0, 50));
    }
}
