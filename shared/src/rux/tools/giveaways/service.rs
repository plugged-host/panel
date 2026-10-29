//! plugged.host: giveaways on Discord, of credits, tokens or days of a membership.
//!
//! Entering always needs a Discord account linked to an account ([`payouts::linked`]). A
//! giveaway can also keep out the winners of the last N giveaways; which giveaways those were is
//! fixed when it is created, so the rule the admin read back is the rule the draw applies.
//!
//! When it ends, exactly one run claims it (`active` → `drawing`), draws the winners among the
//! entries that are still linked and still in the server, and pays them: through noprofit.space
//! under the key `rux:calagopus:<id>` (`…:r<n>` for a reroll), which pays nobody twice when a
//! payout is retried and never meets another panel's keys there; or, without noprofit.space,
//! into the panel's own credits, each winner once. A draw that cannot tell who is linked goes
//! back to `active` and is tried again a minute later.
//!
//! A giveaway belongs to the server it was posted on: only that server's members can enter and
//! win it. Its prize is credits, tokens or days of a membership ([`prize_kind`]); its title names
//! the occasion or its own prize and can never promise anything else ([`prize_title`]). Tokens
//! and memberships are always the panel's own, so the panel pays them in every mode, to the panel
//! account the winner's Discord is linked to (a winner without one waits, like an unlinked one).

use super::super::{
    ToolError,
    credits::payouts::{self, Payer},
    moderation::cases::thousands,
};
use crate::rux::{
    config::Config,
    context::Context,
    discord::Rest,
    models::{GIVEAWAY_COLUMNS, Giveaway, giveaway_status as status, prize_kind},
    noprofit::Unavailable,
    resolver, say, text, time,
};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use rand::seq::SliceRandom;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::LazyLock};

/// The giveaway post's colour while it runs (the theme's accent).
pub const ACCENT: u32 = 0x1D4ED8;
/// "Keep out the winners of the last N giveaways": at most this many.
pub const MAX_EXCLUDE_RECENT: i64 = 10;
/// A payout still waiting (someone not linked yet, noprofit.space down) is tried for this long.
const RETRY_DAYS: i64 = 90;

/// Prizes rux cannot give, as a title might promise them ("1 month of Plus", "500 tokens",
/// "8 GB RAM", "Nitro", "VIP role").
static OTHER_PRIZES: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)\b(?:plus(?:plus)?|premium|vip|nitro|(?:cloud\s*)?tokens?|ram|cpus?|v?cores?|disk|ssd|\d+\s*(?:gb|mb|tb)|gb|tb|roles?|ranks?|subscriptions?)\b",
    )
    .unwrap()
});

/// A draw's key on noprofit.space (`rux:` and 1 to 60 of `[A-Za-z0-9:_-]`); `calagopus` keeps
/// it apart from the keys another panel sends there.
pub fn payout_key(giveaway_id: i64, draw: i16) -> String {
    if draw == 0 {
        format!("rux:calagopus:{giveaway_id}")
    } else {
        format!("rux:calagopus:{giveaway_id}:r{draw}")
    }
}

/// A credit giveaway's title, refused when it promises another prize.
pub fn credits_only_title(title: &str) -> Result<String, ToolError> {
    prize_title(prize_kind::CREDITS, title)
}

/// Whether a word the title matched names the giveaway's own prize ("tokens" on a token
/// giveaway, "Plus" on days of the first membership).
fn names_own_prize(kind: &str, word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    match kind {
        prize_kind::TOKENS => word == "token" || word == "tokens",
        prize_kind::PLUS_DAYS => word == "plus",
        prize_kind::PLUSPLUS_DAYS => word == "premium" || word == "plusplus",
        _ => false,
    }
}

/// A giveaway's title, refused when it promises a prize other than its own.
pub fn prize_title(kind: &str, title: &str) -> Result<String, ToolError> {
    let title = title.trim();

    if let Some(found) = OTHER_PRIZES
        .find_iter(title)
        .find(|found| !names_own_prize(kind, found.as_str()))
    {
        return Err(ToolError::Invalid(format!(
            "A giveaway's prize is what its prize_kind says ({kind}) and nothing else, so the title cannot promise \"{}\". Use a title that names the occasion (or none).",
            found.as_str()
        )));
    }

    Ok(title.to_string())
}

/// The prize for one winner, "500 credits", "15 tokens", "30 days of Plus" (`each`: "… for each
/// winner").
pub fn prize_label(config: &Config, kind: &str, amount: i64, each: bool) -> String {
    let locale = config.locale();
    match kind {
        prize_kind::TOKENS => say::say_count(
            locale,
            if each {
                "giveaway.prize_tokens_each"
            } else {
                "giveaway.prize_tokens_one"
            },
            amount,
            &[],
        ),
        prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS => say::say_count(
            locale,
            if each {
                "giveaway.prize_days_each"
            } else {
                "giveaway.prize_days_one"
            },
            amount,
            &[(
                "membership",
                config.memberships.of(kind == prize_kind::PLUSPLUS_DAYS),
            )],
        ),
        _ => config.say(
            if each {
                "giveaway.prize_each"
            } else {
                "giveaway.prize_one"
            },
            &[("credits", &thousands(amount, locale))],
        ),
    }
}

/// The title a giveaway gets when it is given none: "500 credits", "15 tokens".
pub fn auto_title(config: &Config, kind: &str, amount: i64) -> String {
    prize_label(config, kind, amount, false)
}

fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

fn postponed_key(giveaway_id: i64) -> String {
    format!("rux::giveaways::postponed::{giveaway_id}")
}

impl Giveaway {
    /// Running, and its time is not up.
    pub fn is_open(&self, now: NaiveDateTime) -> bool {
        self.status == status::ACTIVE && self.ends > now
    }

    /// The Discord ids kept out (the winners of the last giveaways, fixed at creation).
    pub fn excluded_ids(&self) -> Vec<String> {
        self.excluded
            .as_ref()
            .and_then(|excluded| excluded["discord_ids"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|id| match id {
                Value::String(id) => Some(id.clone()),
                Value::Number(id) => Some(id.to_string()),
                _ => None,
            })
            .collect()
    }

    pub fn excludes(&self, discord_id: &str) -> bool {
        self.excluded_ids().iter().any(|id| id == discord_id)
    }

    pub fn jump_url(&self) -> Option<String> {
        self.message_id.as_ref().map(|message| {
            format!(
                "https://discord.com/channels/{}/{}/{message}",
                self.guild_id, self.channel_id
            )
        })
    }

    pub async fn by_id(
        database: &crate::database::Database,
        id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(database.read())
        .await
    }

    pub async fn entry_count(
        &self,
        database: &crate::database::Database,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM rux_giveaway_entries WHERE giveaway_id = $1")
            .bind(self.id)
            .fetch_one(database.read())
            .await
    }

    /// The winners: Discord id, draw and payout, first draw first.
    pub async fn winners(
        &self,
        database: &crate::database::Database,
    ) -> Result<Vec<(String, i16, Option<String>)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT discord_id, COALESCE(draw, 0), payout FROM rux_giveaway_entries
            WHERE giveaway_id = $1 AND won IS NOT NULL ORDER BY draw, id",
        )
        .bind(self.id)
        .fetch_all(database.read())
        .await
    }
}

/// What a new giveaway is.
pub struct NewGiveaway {
    /// A [`prize_kind`]; `credits` holds its amount per winner.
    pub kind: String,
    pub credits: i64,
    pub ends_at: String,
    pub winners: i64,
    pub channel: Option<String>,
    pub exclude_recent: i64,
    pub title: Option<String>,
}

fn validate(
    config: &Config,
    kind: &str,
    credits: i64,
    winners: i64,
    ends: DateTime<chrono_tz::Tz>,
    now: DateTime<chrono_tz::Tz>,
) -> Result<(), ToolError> {
    let settings = &config.settings;
    let max_winners = i64::from(settings.giveaway_max_winners.max(1));
    let max_days = i64::from(settings.giveaway_max_days.max(1));
    let zone = config.tz();

    // plugged.host: each prize kind has its own limit
    let (max_amount, what) = match kind {
        prize_kind::TOKENS => (settings.giveaway_max_tokens.max(1), "Tokens"),
        prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS => (
            i64::from(settings.giveaway_max_membership_days.max(1)),
            "Membership days",
        ),
        _ => (settings.giveaway_max_credits.max(1), "Credits"),
    };
    if !(1..=max_amount).contains(&credits) {
        return Err(ToolError::Invalid(format!(
            "{what} per winner must be between 1 and {max_amount} (the limit set in rux's settings)."
        )));
    }
    if !(1..=max_winners).contains(&winners) {
        return Err(ToolError::Invalid(format!(
            "Winners must be between 1 and {max_winners}."
        )));
    }
    if ends < now + TimeDelta::minutes(1) {
        return Err(ToolError::Invalid(format!(
            "The end time {} is not in the future (it is now {}). Ask the admin when it should end.",
            time::human(ends.with_timezone(&Utc), zone),
            time::human(now.with_timezone(&Utc), zone)
        )));
    }
    if ends > now + TimeDelta::days(max_days) {
        return Err(ToolError::Invalid(format!(
            "A giveaway can run for at most {max_days} days."
        )));
    }

    Ok(())
}

/// Starts a giveaway: written, then posted with its Enter button (written back out when Discord
/// refuses the post). Refused while nothing could pay its winners.
pub async fn create(context: &Context, input: NewGiveaway) -> Result<Giveaway, ToolError> {
    let config = &context.config;
    let now = config.now();
    let ends = time::parse(&input.ends_at, now).map_err(ToolError::Invalid)?;
    let kind = input.kind.trim().to_ascii_lowercase();
    if !prize_kind::ALL.contains(&kind.as_str()) {
        return Err(ToolError::Invalid(format!(
            "prize_kind must be one of {}.",
            prize_kind::ALL.join(", ")
        )));
    }
    validate(config, &kind, input.credits, input.winners, ends, now)?;
    let title = prize_title(&kind, input.title.as_deref().unwrap_or_default())?;
    if !(0..=MAX_EXCLUDE_RECENT).contains(&input.exclude_recent) {
        return Err(ToolError::Invalid(format!(
            "exclude_recent_winners must be between 0 and {MAX_EXCLUDE_RECENT}."
        )));
    }

    let Some(channel_input) = input.channel.clone().or_else(|| {
        context
            .guild()
            .and_then(|guild| guild.giveaway_channel_id.clone())
    }) else {
        return Err(ToolError::invalid(
            "No channel given and no default giveaway channel is set. Ask the admin which channel to post it in.",
        ));
    };
    let channel =
        resolver::postable_channel(&context.rest, &context.guild_id, &channel_input).await?;
    let channel_id = channel["id"].as_str().unwrap_or_default().to_string();

    // credits are paid where they live; tokens and memberships always by the panel
    let payer = if prize_kind::panel_only(&kind) {
        panel_prize_available(&context.state, &kind).await?;
        Payer::Panel
    } else {
        payouts::payer(&context.state).await?
    };

    // fixed now: "the last 2 giveaways" means the last 2 at the moment the admin asked
    let mut excluded = Value::Null;
    if input.exclude_recent > 0 {
        let recent = match payouts::recent_giveaways(&context.state, &payer, input.exclude_recent)
            .await
        {
            Ok(recent) => recent,
            Err(ToolError::Unavailable(Unavailable(why))) => {
                return Err(ToolError::Invalid(format!(
                    "Cannot look up the last giveaways' winners right now ({why}), so the giveaway was not created. Tell the admin to try again in a minute."
                )));
            }
            Err(err) => return Err(err),
        };
        let mut ids: Vec<&str> = Vec::new();
        for id in recent.iter().flat_map(|giveaway| giveaway.winners.iter()) {
            if !ids.contains(&id.as_str()) {
                ids.push(id);
            }
        }
        excluded = json!({
            "giveaways": recent.iter().map(|giveaway| json!({
                "id": giveaway.id,
                "prize": giveaway.prize,
                "at": giveaway.at,
                "winners": giveaway.winners.len(),
            })).collect::<Vec<_>>(),
            "discord_ids": ids,
        });
    }

    let prize = if title.is_empty() {
        auto_title(config, &kind, input.credits)
    } else {
        title
    };
    let giveaway = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "INSERT INTO rux_giveaways
            (guild_id, channel_id, prize, credits, prize_kind, winners_count, ends, exclude_recent, excluded, status,
             created_by_discord_id, created_by_account_id, created_by_user_uuid)
        VALUES ($1, $2, $3, $4, $12, $5, $6, $7, $8, 'active', $9, $10, $11)
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(&context.guild_id)
    .bind(&channel_id)
    .bind(text::cut(&prize, 120, ""))
    .bind(input.credits)
    .bind(input.winners as i16)
    .bind(ends.with_timezone(&Utc).naive_utc())
    .bind(input.exclude_recent as i16)
    .bind((!excluded.is_null()).then_some(&excluded))
    .bind(&context.actor_discord_id)
    .bind(context.actor_account_id)
    .bind(context.actor_user_uuid)
    .bind(&kind)
    .fetch_one(context.state.database.write())
    .await?;

    let payload = render(&context.state, config, &giveaway).await?;
    let message = match context.rest.create_message(&channel_id, &payload).await {
        Ok(message) => message,
        Err(err) => {
            sqlx::query("DELETE FROM rux_giveaways WHERE id = $1")
                .bind(giveaway.id)
                .execute(context.state.database.write())
                .await?;
            return Err(err.into());
        }
    };
    let message_id = message["id"].as_str().unwrap_or_default();

    Ok(sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "UPDATE rux_giveaways SET message_id = $2, updated = NOW() WHERE id = $1
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(giveaway.id)
    .bind(message_id)
    .fetch_one(context.state.database.write())
    .await?)
}

/// What an edit changes (`None`: as it is).
pub struct Changes {
    pub credits: Option<i64>,
    pub ends_at: Option<String>,
    pub winners: Option<i64>,
    pub title: Option<String>,
}

/// Changes a running giveaway; its post follows.
pub async fn update(
    context: &Context,
    giveaway: &Giveaway,
    changes: Changes,
) -> Result<Giveaway, ToolError> {
    let config = &context.config;
    if !giveaway.is_open(now()) {
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} is {}; only a running giveaway can be changed.",
            giveaway.id, giveaway.status
        )));
    }

    let local_now = config.now();
    let credits = changes.credits.unwrap_or(giveaway.credits);
    let winners = changes.winners.unwrap_or(i64::from(giveaway.winners_count));
    let ends = match &changes.ends_at {
        Some(ends_at) => time::parse(ends_at, local_now).map_err(ToolError::Invalid)?,
        None => giveaway.ends.and_utc().with_timezone(&config.tz()),
    };
    let kind = giveaway.prize_kind.as_str();
    validate(config, kind, credits, winners, ends, local_now)?;

    let title = prize_title(kind, changes.title.as_deref().unwrap_or_default())?;
    let prize = if !title.is_empty() {
        title
    } else if giveaway.prize == auto_title(config, kind, giveaway.credits) {
        auto_title(config, kind, credits)
    } else {
        giveaway.prize.clone()
    };

    let Some(updated) = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "UPDATE rux_giveaways SET credits = $2, winners_count = $3, ends = $4, prize = $5, updated = NOW()
        WHERE id = $1 AND status = 'active'
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(giveaway.id)
    .bind(credits)
    .bind(winners as i16)
    .bind(ends.with_timezone(&Utc).naive_utc())
    .bind(text::cut(&prize, 120, ""))
    .fetch_optional(context.state.database.write())
    .await?
    else {
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} ended meanwhile; only a running giveaway can be changed.",
            giveaway.id
        )));
    };
    refresh_message(&context.state, config, &context.rest, &updated).await;

    Ok(updated)
}

/// Cancels a running giveaway: nobody wins, nothing is paid.
pub async fn cancel(context: &Context, giveaway: &Giveaway) -> Result<Giveaway, ToolError> {
    let Some(cancelled) = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "UPDATE rux_giveaways SET status = 'cancelled', ended = $2, updated = NOW()
        WHERE id = $1 AND status = 'active'
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(giveaway.id)
    .bind(now())
    .fetch_optional(context.state.database.write())
    .await?
    else {
        let current = Giveaway::by_id(&context.state.database, giveaway.id).await?;
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} is {}; only a running giveaway can be cancelled.",
            giveaway.id,
            current.map_or_else(|| "gone".to_string(), |current| current.status)
        )));
    };
    refresh_message(&context.state, &context.config, &context.rest, &cancelled).await;

    Ok(cancelled)
}

/// Ends a running giveaway now: the draw happens right away. `None` when it could not be drawn
/// yet (it is then drawn by itself within a minute or two).
pub async fn end_now(
    context: &Context,
    giveaway: &Giveaway,
) -> Result<Option<Giveaway>, ToolError> {
    let ended = sqlx::query(
        "UPDATE rux_giveaways SET ends = $2, updated = NOW() WHERE id = $1 AND status = 'active'",
    )
    .bind(giveaway.id)
    .bind(now())
    .execute(context.state.database.write())
    .await?
    .rows_affected();
    if ended == 0 {
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} is {}; only a running giveaway can be ended.",
            giveaway.id, giveaway.status
        )));
    }
    let _ = context
        .state
        .cache
        .invalidate(&postponed_key(giveaway.id))
        .await;

    Ok(draw(&context.state, &context.config, &context.rest, giveaway.id).await?)
}

/// The giveaways whose time is up, drawn (one that could not be drawn a moment ago waits a
/// minute).
pub async fn due(state: &crate::State, config: &Config, rest: &Rest) -> Result<(), anyhow::Error> {
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM rux_giveaways WHERE status = 'active' AND ends <= $1 ORDER BY ends LIMIT 10",
    )
    .bind(now())
    .fetch_all(state.database.read())
    .await?;

    for id in ids {
        if state
            .cache
            .exists(&postponed_key(id))
            .await
            .unwrap_or(false)
        {
            continue;
        }
        if let Err(err) = draw(state, config, rest, id).await {
            tracing::warn!(giveaway = id, "rux: giveaway draw failed: {err:#}");
        }
    }

    Ok(())
}

/// Draws a giveaway whose time is up. Only the run that moves it from `active` to `drawing`
/// draws; any other gets `None`. When it cannot be told who is linked, nothing is drawn and it
/// goes back to `active` for a minute later.
pub async fn draw(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    giveaway_id: i64,
) -> Result<Option<Giveaway>, anyhow::Error> {
    let Some(giveaway) = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "UPDATE rux_giveaways SET status = 'drawing', updated = NOW()
        WHERE id = $1 AND status = 'active' AND ends <= $2
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(giveaway_id)
    .bind(now())
    .fetch_optional(state.database.write())
    .await?
    else {
        return Ok(None);
    };

    let winners = match pick(state, rest, &giveaway, i64::from(giveaway.winners_count)).await {
        Ok(winners) => winners,
        Err(err) => {
            tracing::warn!(
                giveaway = giveaway.id,
                "rux: giveaway draw postponed, it cannot be told who is linked: {err}"
            );
            sqlx::query("UPDATE rux_giveaways SET status = 'active', updated = NOW() WHERE id = $1 AND status = 'drawing'")
                .bind(giveaway.id)
                .execute(state.database.write())
                .await?;
            let _ = state
                .cache
                .set(&postponed_key(giveaway.id), 60, &true)
                .await;
            return Ok(None);
        }
    };

    let drawn = now();
    let mut transaction = state.database.write().begin().await?;
    sqlx::query(
        "UPDATE rux_giveaway_entries SET won = $3, draw = 0, updated = NOW()
        WHERE giveaway_id = $1 AND discord_id = ANY($2) AND won IS NULL",
    )
    .bind(giveaway.id)
    .bind(&winners)
    .bind(drawn)
    .execute(&mut *transaction)
    .await?;
    let giveaway = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "UPDATE rux_giveaways SET status = 'ended', ended = $2, payout_status = $3, updated = NOW()
        WHERE id = $1
        RETURNING {GIVEAWAY_COLUMNS}"
    )))
    .bind(giveaway.id)
    .bind(drawn)
    .bind(if winners.is_empty() {
        status::PAYOUT_NONE
    } else {
        status::PAYOUT_PENDING
    })
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    tracing::info!(giveaway = giveaway.id, winners = ?winners, "rux: giveaway drawn");

    let paid = winners.is_empty() || pay_draw(state, &giveaway, 0).await;
    refresh_message(state, config, rest, &giveaway).await;
    announce(state, config, rest, &giveaway, &winners, paid, false).await;

    Ok(Some(giveaway))
}

/// Draws more winners for an ended giveaway, among the entries that have not won yet; they are
/// paid like the first ones. Answers the new winners.
pub async fn reroll(
    context: &Context,
    giveaway: &Giveaway,
    count: i64,
) -> Result<Vec<String>, ToolError> {
    let state = &context.state;
    if giveaway.status != status::ENDED {
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} is {}; only an ended giveaway can be rerolled.",
            giveaway.id, giveaway.status
        )));
    }

    let count = count.clamp(
        1,
        i64::from(context.config.settings.giveaway_max_winners.max(1)),
    );
    let picked =
        pick(state, &context.rest, giveaway, count)
            .await
            .map_err(|Unavailable(why)| {
                ToolError::Invalid(format!("Cannot reroll right now: {why}"))
            })?;
    if picked.is_empty() {
        return Err(ToolError::Invalid(format!(
            "Nobody else in giveaway #{} can win: every remaining entry is unlinked, excluded or gone from the server.",
            giveaway.id
        )));
    }

    let mut transaction = state.database.write().begin().await?;
    let Some(draw): Option<i16> = sqlx::query_scalar(
        "UPDATE rux_giveaways SET rerolls = rerolls + 1, payout_status = 'pending', updated = NOW()
        WHERE id = $1 AND status = 'ended'
        RETURNING rerolls",
    )
    .bind(giveaway.id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Err(ToolError::Invalid(format!(
            "Giveaway #{} is no longer ended.",
            giveaway.id
        )));
    };
    let winners: Vec<String> = sqlx::query_scalar(
        "UPDATE rux_giveaway_entries SET won = $3, draw = $4, updated = NOW()
        WHERE giveaway_id = $1 AND discord_id = ANY($2) AND won IS NULL
        RETURNING discord_id",
    )
    .bind(giveaway.id)
    .bind(&picked)
    .bind(now())
    .bind(draw)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;

    // in the order they were drawn
    let winners: Vec<String> = picked
        .into_iter()
        .filter(|id| winners.contains(id))
        .collect();
    let giveaway = Giveaway::by_id(&state.database, giveaway.id)
        .await?
        .ok_or_else(|| ToolError::Failed(anyhow::anyhow!("the giveaway is gone")))?;
    let paid = winners.is_empty() || pay_draw(state, &giveaway, draw).await;
    refresh_message(state, &context.config, &context.rest, &giveaway).await;
    announce(
        state,
        &context.config,
        &context.rest,
        &giveaway,
        &winners,
        paid,
        true,
    )
    .await;

    Ok(winners)
}

/// Up to `count` winners in random order among the entries that have not won, are not kept out,
/// are linked and are still in the server.
async fn pick(
    state: &crate::State,
    rest: &Rest,
    giveaway: &Giveaway,
    count: i64,
) -> Result<Vec<String>, Unavailable> {
    let excluded = giveaway.excluded_ids();
    let candidates: Vec<String> = sqlx::query_scalar(
        "SELECT discord_id FROM rux_giveaway_entries WHERE giveaway_id = $1 AND won IS NULL ORDER BY id",
    )
    .bind(giveaway.id)
    .fetch_all(state.database.read())
    .await
    .map_err(|err| Unavailable(format!("the entries could not be read: {err}")))?
    .into_iter()
    .filter(|id| !excluded.contains(id))
    .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let linked = payouts::linked(state, &candidates, 8).await?;
    let mut eligible: Vec<String> = candidates
        .into_iter()
        .filter(|id| linked.contains(id))
        .collect();
    // the OS-seeded CSPRNG, before anything is awaited
    eligible.shuffle(&mut rand::rng());

    let mut winners = Vec::new();
    for discord_id in eligible {
        if winners.len() as i64 >= count {
            break;
        }
        match rest.member(&giveaway.guild_id, &discord_id).await {
            Ok(None) => continue,
            Ok(Some(_)) => {}
            // Discord could not say; the entrant is not punished for it
            Err(err) => tracing::debug!("rux: giveaway winner {discord_id}: {err}"),
        }
        winners.push(discord_id);
    }

    Ok(winners)
}

async fn payout_failed(state: &crate::State, giveaway: &Giveaway, draw: i16, why: &str) {
    tracing::warn!(
        giveaway = giveaway.id,
        draw,
        "rux: giveaway payout failed, it is tried again: {why}"
    );
    if let Err(err) = sqlx::query(
        "UPDATE rux_giveaways SET payout_attempts = payout_attempts + 1, payout_error = $2, updated = NOW()
        WHERE id = $1",
    )
    .bind(giveaway.id)
    .bind(text::cut(why, 250, ""))
    .execute(state.database.write())
    .await
    {
        tracing::warn!(giveaway = giveaway.id, "rux: could not note a failed payout: {err}");
    }
}

/// Pays one draw's winners. `true` once the payer took them (paid, or waiting until they link);
/// `false` leaves the payout pending for [`retry_payouts`].
pub async fn pay_draw(state: &crate::State, giveaway: &Giveaway, draw: i16) -> bool {
    match pay(state, giveaway, draw).await {
        Ok(()) => true,
        Err(why) => {
            payout_failed(state, giveaway, draw, &why).await;
            false
        }
    }
}

async fn pay(state: &crate::State, giveaway: &Giveaway, draw: i16) -> Result<(), String> {
    let entries: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, discord_id FROM rux_giveaway_entries
        WHERE giveaway_id = $1 AND draw = $2 AND won IS NOT NULL ORDER BY id",
    )
    .bind(giveaway.id)
    .bind(draw)
    .fetch_all(state.database.read())
    .await
    .map_err(|err| err.to_string())?;
    if entries.is_empty() {
        return Ok(());
    }

    // tokens and memberships are always the panel's own
    let payer = if prize_kind::panel_only(&giveaway.prize_kind) {
        Payer::Panel
    } else {
        payouts::payer(state).await.map_err(|err| err.to_string())?
    };
    let panel = matches!(payer, Payer::Panel);
    let reference = match payer {
        Payer::Noprofit(client) => {
            let answer = client
                .post(
                    "/giveaways",
                    &json!({
                        "key": payout_key(giveaway.id, draw),
                        "prize": giveaway.prize,
                        "amount": giveaway.credits,
                        "discord_ids": entries.iter().map(|(_, id)| id).collect::<Vec<_>>(),
                        "created_by": giveaway.created_by_account_id,
                    }),
                    20,
                )
                .await
                .map_err(|Unavailable(why)| why)?;
            let statuses: HashMap<String, String> = answer["recipients"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|recipient| {
                    Some((
                        recipient["discord_id"].as_str()?.to_string(),
                        recipient["status"].as_str()?.to_string(),
                    ))
                })
                .collect();

            for (id, discord_id) in &entries {
                let payout = match statuses.get(discord_id).map(String::as_str) {
                    Some(status::PAYOUT_PAID) => status::PAYOUT_PAID,
                    _ => status::PAYOUT_PENDING,
                };
                sqlx::query(
                    "UPDATE rux_giveaway_entries SET payout = $2, updated = NOW() WHERE id = $1",
                )
                .bind(id)
                .bind(payout)
                .execute(state.database.write())
                .await
                .map_err(|err| err.to_string())?;
            }

            (draw == 0)
                .then(|| answer["giveaway_id"].as_i64())
                .flatten()
                .map(|id| id.to_string())
        }
        Payer::Panel => {
            for (id, discord_id) in &entries {
                pay_on_panel(state, giveaway, draw, *id, discord_id).await?;
            }
            None
        }
    };

    // the panel waits for winners who are not linked (any more): they stay pending
    sqlx::query(
        "UPDATE rux_giveaways SET
            payout_status = CASE WHEN EXISTS (
                SELECT 1 FROM rux_giveaway_entries
                WHERE giveaway_id = $1 AND won IS NOT NULL
                    AND (payout IS NULL OR ($3 AND payout = 'pending'))
            ) THEN 'pending' ELSE 'paid' END,
            payout_attempts = payout_attempts + 1,
            payout_error = NULL,
            payout_reference = COALESCE($2, payout_reference),
            updated = NOW()
        WHERE id = $1",
    )
    .bind(giveaway.id)
    .bind(reference)
    .bind(panel)
    .execute(state.database.write())
    .await
    .map_err(|err| err.to_string())?;

    Ok(())
}

/// Pays one winner on the panel (its own credits, tokens or a membership's days), once: the entry
/// is claimed in the same transaction. A winner who is not linked (any more) stays pending.
async fn pay_on_panel(
    state: &crate::State,
    giveaway: &Giveaway,
    draw: i16,
    entry_id: i64,
    discord_id: &str,
) -> Result<(), String> {
    let Some(user_uuid) = payouts::panel_user(state, discord_id)
        .await
        .map_err(|err| err.to_string())?
    else {
        sqlx::query(
            "UPDATE rux_giveaway_entries SET payout = 'pending', updated = NOW() WHERE id = $1 AND payout IS NULL",
        )
        .bind(entry_id)
        .execute(state.database.write())
        .await
        .map_err(|err| err.to_string())?;
        return Ok(());
    };

    let result: Result<bool, sqlx::Error> = async {
        let mut transaction = state.database.write().begin().await?;
        let claimed = sqlx::query(
            "UPDATE rux_giveaway_entries SET payout = 'paid', updated = NOW()
            WHERE id = $1 AND (payout IS NULL OR payout = 'pending')",
        )
        .bind(entry_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if claimed == 0 {
            return Ok(false);
        }

        let data = json!({
            "giveaway": giveaway.id,
            "draw": draw,
            "key": payout_key(giveaway.id, draw),
            "discord_id": discord_id,
            "prize_kind": giveaway.prize_kind,
        });
        let description = format!("Giveaway on Discord: {}", giveaway.prize);

        // plugged.host: tokens and membership days are paid like the credits, once per entry
        match giveaway.prize_kind.as_str() {
            prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS => {
                crate::economy::give_membership_days(
                    &mut transaction,
                    user_uuid,
                    giveaway.prize_kind == prize_kind::PLUSPLUS_DAYS,
                    giveaway.credits,
                )
                .await?;
                transaction.commit().await?;
                return Ok::<bool, sqlx::Error>(true);
            }
            kind => {
                let currency = if kind == prize_kind::TOKENS {
                    crate::economy::Currency::Tokens
                } else {
                    crate::economy::Currency::Credits
                };
                let paid = crate::economy::change(
                    &mut transaction,
                    currency,
                    user_uuid,
                    giveaway.credits,
                    "rux_giveaway",
                    &description,
                    data,
                )
                .await?;
                if paid.is_some() {
                    transaction.commit().await?;
                }
            }
        }

        Ok::<bool, sqlx::Error>(false)
    }
    .await;

    match result {
        Ok(membership) => {
            if membership {
                crate::economy::membership_changed(state, user_uuid).await;
            }
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Whether the panel can pay a prize of this kind now: tokens need tokens on, days the
/// memberships on.
async fn panel_prize_available(state: &crate::State, kind: &str) -> Result<(), ToolError> {
    let (tokens, memberships) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.tokens_enabled,
            settings.economy.membership_enabled,
        )
    };

    match kind {
        prize_kind::TOKENS if !tokens => Err(ToolError::invalid(
            "Tokens are turned off on this panel, so no tokens can be given. Tell an admin.",
        )),
        prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS if !memberships => {
            Err(ToolError::invalid(
                "Memberships are turned off on this panel, so no membership days can be given. Tell an admin.",
            ))
        }
        _ => Ok(()),
    }
}

/// Pays the draws that are still waiting: noprofit.space was down at the draw, or (the panel
/// paying itself) a winner had not linked yet. Each giveaway is tried less often the more often
/// it failed, for 90 days.
pub async fn retry_payouts(state: &crate::State) -> Result<(), anyhow::Error> {
    let giveaways = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways
        WHERE status = 'ended' AND payout_status = 'pending' AND ended > $1
            AND updated <= NOW() - make_interval(mins => LEAST(payout_attempts, 60))
        ORDER BY id LIMIT 20"
    )))
    .bind(now() - TimeDelta::days(RETRY_DAYS))
    .fetch_all(state.database.read())
    .await?;

    for giveaway in giveaways {
        let draws: Vec<i16> = sqlx::query_scalar(
            "SELECT DISTINCT draw FROM rux_giveaway_entries
            WHERE giveaway_id = $1 AND won IS NOT NULL AND draw IS NOT NULL
                AND (payout IS NULL OR payout = 'pending')
            ORDER BY draw",
        )
        .bind(giveaway.id)
        .fetch_all(state.database.read())
        .await?;

        for draw in draws {
            pay_draw(state, &giveaway, draw).await;
        }
    }

    Ok(())
}

/// A draw that died halfway (the process was stopped) holds its giveaway in `drawing` with no
/// winners written: it goes back, so the next run draws it.
pub async fn recover_stalled(state: &crate::State) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE rux_giveaways SET status = 'active', updated = NOW()
        WHERE status = 'drawing' AND updated < NOW() - INTERVAL '10 minutes'",
    )
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// Keeps the entry count on running giveaways' posts current.
pub async fn refresh_counts(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
) -> Result<(), sqlx::Error> {
    let giveaways = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways g
        WHERE status = 'active' AND message_id IS NOT NULL
            AND rendered_entries <> (SELECT COUNT(*) FROM rux_giveaway_entries e WHERE e.giveaway_id = g.id)
        ORDER BY id LIMIT 25"
    )))
    .fetch_all(state.database.read())
    .await?;

    for giveaway in giveaways {
        refresh_message(state, config, rest, &giveaway).await;
    }

    Ok(())
}

/// Puts a giveaway's post up to date; a failure is only logged.
pub async fn refresh_message(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    giveaway: &Giveaway,
) {
    let Some(message_id) = &giveaway.message_id else {
        return;
    };

    let result = match render(state, config, giveaway).await {
        Ok(payload) => rest
            .edit_message(&giveaway.channel_id, message_id, &payload)
            .await
            .map(|_| ())
            .map_err(|err| err.to_string()),
        Err(err) => Err(err.to_string()),
    };
    if let Err(err) = result {
        tracing::warn!(
            giveaway = giveaway.id,
            "rux: could not update a giveaway post: {err}"
        );
    }
}

/// The giveaway's post as it is now (and the entry count it shows, remembered).
pub async fn render(
    state: &crate::State,
    config: &Config,
    giveaway: &Giveaway,
) -> Result<Value, sqlx::Error> {
    let entries = giveaway.entry_count(&state.database).await?;
    let winners: Vec<String> = giveaway
        .winners(&state.database)
        .await?
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    if i64::from(giveaway.rendered_entries) != entries {
        sqlx::query("UPDATE rux_giveaways SET rendered_entries = $2 WHERE id = $1")
            .bind(giveaway.id)
            .bind(entries.clamp(0, i64::from(i32::MAX)) as i32)
            .execute(state.database.write())
            .await?;
    }

    Ok(render_payload(config, giveaway, entries, &winners))
}

/// The post: what it is, when it ends, who can enter, how many did, and the Enter button
/// (disabled once it is over).
pub fn render_payload(
    config: &Config,
    giveaway: &Giveaway,
    entries: i64,
    winners: &[String],
) -> Value {
    let locale = config.locale();
    let prize_line = prize_label(
        config,
        &giveaway.prize_kind,
        giveaway.credits,
        giveaway.winners_count > 1,
    );

    let mut rules = vec![format!("- {}", config.say("giveaway.rule_link", &[]))];
    if giveaway.exclude_recent > 0 {
        rules.push(format!(
            "- {}",
            say::say_count(
                locale,
                "giveaway.rule_exclude",
                i64::from(giveaway.exclude_recent),
                &[]
            )
        ));
    }

    let ends = giveaway.ends.and_utc();
    let custom_id = format!("{}enter:{}", super::BUTTON_PREFIX, giveaway.id);
    let (body, winners_value, button, color) = match giveaway.status.as_str() {
        status::ACTIVE | status::DRAWING => (
            config.say(
                "giveaway.open_body",
                &[
                    ("relative", &time::discord(ends, 'R')),
                    ("absolute", &time::discord(ends, 'f')),
                ],
            ),
            giveaway.winners_count.to_string(),
            json!({
                "style": 1,
                "label": config.say("giveaway.enter", &[]),
                "custom_id": custom_id,
                "disabled": giveaway.status != status::ACTIVE,
            }),
            Some(ACCENT),
        ),
        status::CANCELLED => (
            config.say("giveaway.cancelled_body", &[]),
            giveaway.winners_count.to_string(),
            json!({
                "style": 2,
                "label": config.say("giveaway.cancelled", &[]),
                "custom_id": custom_id,
                "disabled": true,
            }),
            None,
        ),
        _ => (
            config.say(
                "giveaway.ended_body",
                &[(
                    "relative",
                    &time::discord(giveaway.ended.unwrap_or(giveaway.ends).and_utc(), 'R'),
                )],
            ),
            if winners.is_empty() {
                config.say("giveaway.no_winner", &[])
            } else {
                winners
                    .iter()
                    .map(|id| format!("<@{id}>"))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            json!({
                "style": 2,
                "label": config.say("giveaway.ended", &[]),
                "custom_id": custom_id,
                "disabled": true,
            }),
            None,
        ),
    };

    let description = if giveaway.prize
        != auto_title(config, &giveaway.prize_kind, giveaway.credits)
        || giveaway.winners_count > 1
    {
        format!("{prize_line}\n\n{body}")
    } else {
        body
    };
    let mut embed = json!({
        "title": text::cut(&giveaway.prize, 256, ""),
        "description": description,
        "fields": [
            { "name": config.say("giveaway.winners", &[]), "value": text::cut(&winners_value, 1024, "…"), "inline": true },
            { "name": config.say("giveaway.entries", &[]), "value": thousands(entries, locale), "inline": true },
            { "name": config.say("giveaway.rules", &[]), "value": rules.join("\n"), "inline": false },
        ],
        "footer": { "text": config.say("giveaway.footer", &[("id", &giveaway.id.to_string())]) },
        "timestamp": ends.to_rfc3339(),
    });
    if let Some(color) = color {
        embed["color"] = json!(color);
    }
    let mut button = button;
    button["type"] = json!(2);

    json!({
        "content": "",
        "embeds": [embed],
        "components": [{ "type": 1, "components": [button] }],
        "allowed_mentions": { "parse": [] },
    })
}

/// Tells the channel who won (as a reply to the post), pinging only the winners.
async fn announce(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    giveaway: &Giveaway,
    winners: &[String],
    paid: bool,
    reroll: bool,
) {
    let content = if winners.is_empty() {
        config.say("giveaway.announce_none", &[("prize", &giveaway.prize)])
    } else {
        let mentions = winners
            .iter()
            .map(|id| format!("<@{id}>"))
            .collect::<Vec<_>>()
            .join(", ");
        let all_paid = paid
            && sqlx::query_scalar::<_, bool>(
                "SELECT NOT EXISTS (
                    SELECT 1 FROM rux_giveaway_entries
                    WHERE giveaway_id = $1 AND discord_id = ANY($2) AND payout IS DISTINCT FROM 'paid'
                )",
            )
            .bind(giveaway.id)
            .bind(winners)
            .fetch_one(state.database.read())
            .await
            .unwrap_or(false);

        format!(
            "{} {}",
            config.say(
                if reroll {
                    "giveaway.announce_reroll"
                } else {
                    "giveaway.announce"
                },
                &[("winners", &mentions), ("prize", &giveaway.prize)],
            ),
            config.say(
                match (giveaway.prize_kind.as_str(), all_paid) {
                    (prize_kind::TOKENS, true) => "giveaway.announce_paid_tokens",
                    (prize_kind::TOKENS, false) => "giveaway.announce_pending_tokens",
                    (prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS, true) => {
                        "giveaway.announce_paid_days"
                    }
                    (prize_kind::PLUS_DAYS | prize_kind::PLUSPLUS_DAYS, false) => {
                        "giveaway.announce_pending_days"
                    }
                    (_, true) => "giveaway.announce_paid",
                    (_, false) => "giveaway.announce_pending",
                },
                &[
                    ("app", &config.app_name),
                    (
                        "membership",
                        config
                            .memberships
                            .of(giveaway.prize_kind == prize_kind::PLUSPLUS_DAYS),
                    ),
                ],
            )
        )
    };

    let mut payload = json!({
        "content": text::cut(&content, 2000, ""),
        "allowed_mentions": { "parse": [], "users": winners.iter().take(100).collect::<Vec<_>>() },
    });
    if let Some(message_id) = &giveaway.message_id {
        payload["message_reference"] =
            json!({ "message_id": message_id, "fail_if_not_exists": false });
    }

    if let Err(err) = rest.create_message(&giveaway.channel_id, &payload).await {
        tracing::warn!(
            giveaway = giveaway.id,
            "rux: could not announce the winners: {err}"
        );
    }
}

/// A giveaway for the model.
pub async fn summary(
    state: &crate::State,
    config: &Config,
    giveaway: &Giveaway,
) -> Result<Value, sqlx::Error> {
    let zone = config.tz();
    let entries = giveaway.entry_count(&state.database).await?;
    let winners: Vec<String> = giveaway
        .winners(&state.database)
        .await?
        .into_iter()
        .map(|(id, draw, payout)| {
            format!(
                "<@{id}>{} {}",
                if draw > 0 {
                    format!(" (reroll {draw})")
                } else {
                    String::new()
                },
                payout.unwrap_or_else(|| "not paid yet".into())
            )
        })
        .collect();
    let excludes = (giveaway.exclude_recent > 0).then(|| {
        giveaway
            .excluded
            .as_ref()
            .and_then(|excluded| excluded["giveaways"].as_array())
            .into_iter()
            .flatten()
            .map(|recent| {
                let at = recent["at"]
                    .as_str()
                    .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                    .map(|at| format!(" ({})", time::human(at.with_timezone(&Utc), zone)))
                    .unwrap_or_default();
                format!(
                    "#{} \"{}\"{at}",
                    recent["id"],
                    recent["prize"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
    });

    Ok(super::super::moderation::common::without_nulls(json!({
        "id": giveaway.id,
        "status": giveaway.status,
        "prize": giveaway.prize,
        "prize_kind": giveaway.prize_kind,
        "per_winner": prize_label(config, &giveaway.prize_kind, giveaway.credits, false),
        "winners_wanted": giveaway.winners_count,
        "channel": format!("<#{}>", giveaway.channel_id),
        "ends_at": time::human(giveaway.ends.and_utc(), zone),
        "ended_at": giveaway.ended.map(|ended| time::human(ended.and_utc(), zone)),
        "entries": entries,
        "requires_linked_account": true,
        "excludes_winners_of": excludes,
        "excluded_people": (giveaway.exclude_recent > 0).then(|| giveaway.excluded_ids().len()),
        "winners": (!winners.is_empty()).then_some(winners),
        "payout": (giveaway.status == status::ENDED).then(|| match &giveaway.payout_error {
            Some(error) if giveaway.payout_status != status::PAYOUT_PAID => format!("{} ({error})", giveaway.payout_status),
            _ => giveaway.payout_status.clone(),
        }),
        "message": giveaway.jump_url(),
    })))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::settings::rux::AppSettingsRux;

    pub fn config(locale: &str) -> Config {
        let settings = AppSettingsRux {
            locale: locale.into(),
            guild_id: "100000000000000001".into(),
            ..Default::default()
        };

        Config {
            guilds: crate::rux::config::guilds(&settings, "", "Example"),
            settings,
            enabled: true,
            app_name: "Example".into(),
            portal: "example.com".into(),
            link_url: "https://example.com/discord".into(),
            boost_two_role_id: String::new(),
            managed: None,
            managed_bot_token: None,
            memberships: Default::default(),
        }
    }

    pub fn giveaway(state: &str) -> Giveaway {
        let at = DateTime::from_timestamp(1_791_108_000, 0)
            .unwrap()
            .naive_utc();

        Giveaway {
            id: 3,
            guild_id: "100000000000000001".into(),
            channel_id: "100000000000000004".into(),
            message_id: Some("100000000000000005".into()),
            prize: "500 credits".into(),
            credits: 500,
            prize_kind: prize_kind::CREDITS.into(),
            winners_count: 1,
            ends: at,
            exclude_recent: 0,
            excluded: None,
            status: state.into(),
            created_by_discord_id: "100000000000000002".into(),
            created_by_account_id: None,
            created_by_user_uuid: None,
            ended: None,
            payout_status: "none".into(),
            payout_attempts: 0,
            payout_error: None,
            payout_reference: None,
            rerolls: 0,
            rendered_entries: 0,
            created: at,
            updated: at,
        }
    }

    #[test]
    fn payout_keys_are_what_noprofit_takes_and_our_own() {
        let pattern = regex::Regex::new(r"^rux:[A-Za-z0-9:_-]{1,60}$").unwrap();
        for key in [
            payout_key(1, 0),
            payout_key(i64::MAX, 0),
            payout_key(i64::MAX, i16::MAX),
        ] {
            assert!(pattern.is_match(&key), "{key}");
        }
        assert_eq!(payout_key(12, 0), "rux:calagopus:12");
        assert_eq!(payout_key(12, 2), "rux:calagopus:12:r2");
        // Pelican's are rux:<id> and rux:<id>:r<n>: never the same
        assert_ne!(payout_key(12, 0), "rux:12");
    }

    #[test]
    fn titles_promise_credits_only() {
        assert_eq!(
            credits_only_title("  Halloween giveaway ").unwrap(),
            "Halloween giveaway"
        );
        assert_eq!(credits_only_title("").unwrap(), "");
        for title in [
            "1 month of Plus",
            "500 tokens",
            "8 GB RAM",
            "8gb",
            "Free Nitro",
            "VIP role",
            "Premium",
            "2 vCores",
        ] {
            assert!(credits_only_title(title).is_err(), "{title}");
        }
        // a word that only contains one is fine
        assert!(credits_only_title("Programmers' party").is_ok());
        assert!(credits_only_title("Surplus week").is_ok());
    }

    #[test]
    fn titles_may_name_their_own_prize() {
        assert!(prize_title(prize_kind::TOKENS, "500 tokens party").is_ok());
        assert!(prize_title(prize_kind::TOKENS, "Plus week").is_err());
        assert!(prize_title(prize_kind::PLUS_DAYS, "A month of Plus").is_ok());
        assert!(prize_title(prize_kind::PLUS_DAYS, "Premium").is_err());
        assert!(prize_title(prize_kind::PLUSPLUS_DAYS, "Premium week").is_ok());
        assert!(prize_title(prize_kind::PLUSPLUS_DAYS, "8 GB RAM").is_err());
        assert!(prize_title(prize_kind::CREDITS, "500 tokens").is_err());
    }

    #[test]
    fn prizes_are_named_by_kind() {
        let en = config("en");
        assert_eq!(
            prize_label(&en, prize_kind::CREDITS, 500, false),
            "500 credits"
        );
        assert_eq!(prize_label(&en, prize_kind::TOKENS, 1, false), "1 token");
        assert_eq!(
            prize_label(&en, prize_kind::TOKENS, 15, true),
            "15 tokens for each winner"
        );
        assert_eq!(
            prize_label(&en, prize_kind::PLUS_DAYS, 30, false),
            "30 days of Plus"
        );
        assert_eq!(
            prize_label(&config("de"), prize_kind::PLUSPLUS_DAYS, 1, false),
            "1 Tag Premium"
        );
        assert!(prize_kind::panel_only(prize_kind::TOKENS));
        assert!(!prize_kind::panel_only(prize_kind::CREDITS));
    }

    #[test]
    fn excluded_people_are_read() {
        let mut giveaway = giveaway(status::ACTIVE);
        assert!(giveaway.excluded_ids().is_empty());
        giveaway.excluded =
            Some(json!({"discord_ids": ["100000000000000002", 100000000000000003i64]}));
        assert!(giveaway.excludes("100000000000000002"));
        assert!(giveaway.excludes("100000000000000003"));
        assert!(!giveaway.excludes("100000000000000009"));
        assert_eq!(
            giveaway.jump_url().as_deref(),
            Some(
                "https://discord.com/channels/100000000000000001/100000000000000004/100000000000000005"
            )
        );
        assert!(giveaway.is_open(giveaway.ends - TimeDelta::seconds(1)));
        assert!(!giveaway.is_open(giveaway.ends));
    }

    #[test]
    fn a_running_giveaway_has_an_enter_button() {
        let config = config("en");
        let payload = render_payload(&config, &giveaway(status::ACTIVE), 1234, &[]);
        let embed = &payload["embeds"][0];
        assert_eq!(embed["title"], "500 credits");
        // the title says it all: no prize line
        assert_eq!(
            embed["description"],
            "Press **Enter** to join. Ends <t:1791108000:R> (<t:1791108000:f>)."
        );
        assert_eq!(embed["color"], ACCENT);
        assert_eq!(embed["fields"][0]["value"], "1");
        assert_eq!(embed["fields"][1]["value"], "1,234");
        assert_eq!(
            embed["fields"][2]["value"],
            "- Your Discord account is linked to example.com."
        );
        assert_eq!(embed["footer"]["text"], "Giveaway #3");
        let button = &payload["components"][0]["components"][0];
        assert_eq!(button["type"], 2);
        assert_eq!(button["custom_id"], "rux:gw:enter:3");
        assert_eq!(button["disabled"], false);
        assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
    }

    #[test]
    fn an_ended_giveaway_shows_its_winners() {
        let config = config("de");
        let mut ended = giveaway(status::ENDED);
        ended.prize = "Halloween".into();
        ended.winners_count = 2;
        ended.exclude_recent = 2;
        let payload = render_payload(
            &config,
            &ended,
            2,
            &["100000000000000002".into(), "100000000000000003".into()],
        );
        let embed = &payload["embeds"][0];
        assert_eq!(
            embed["description"],
            "500 Credits für jeden Gewinner\n\nBeendet <t:1791108000:R>."
        );
        assert!(embed.get("color").is_none());
        assert_eq!(
            embed["fields"][0]["value"],
            "<@100000000000000002>, <@100000000000000003>"
        );
        assert_eq!(
            embed["fields"][2]["value"],
            "- Dein Discord-Konto ist mit example.com verknüpft.\n- Du hast keines der letzten 2 Gewinnspiele gewonnen."
        );
        let button = &payload["components"][0]["components"][0];
        assert_eq!(button["label"], "Beendet");
        assert_eq!(button["disabled"], true);
    }
}
