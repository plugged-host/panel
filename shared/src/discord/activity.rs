//! plugged.host: message counts (docs/features/discord.md#activity).
//!
//! The moderation bot sees every message on its servers ([`crate::rux::watch`]); each one a
//! member sends (not spam, not a bot's) adds one to their count for that server and day in
//! `discord_message_counts`. noprofit.space asks for the counts of the last days
//! (`/api/noprofit/discord/activity`) to hold rewards back from members who are not active
//! enough. Counts older than [`KEEP_DAYS`] are deleted once a day.

use std::collections::HashMap;

/// How long daily counts are kept, and the longest window that can be asked for.
pub const KEEP_DAYS: i32 = 60;

/// One more message from `discord_id` on `guild_id` today (UTC).
pub async fn record(state: &crate::State, guild_id: &str, discord_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO discord_message_counts (guild_id, discord_id, day, messages)
        VALUES ($1, $2, (NOW() AT TIME ZONE 'UTC')::date, 1)
        ON CONFLICT (guild_id, discord_id, day)
        DO UPDATE SET messages = discord_message_counts.messages + 1",
    )
    .bind(guild_id)
    .bind(discord_id)
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// Messages per member on each of `guild_ids` in the last `days` days (today included), only
/// members who sent any: guild id => Discord id => messages.
pub async fn counts(
    state: &crate::State,
    guild_ids: &[String],
    days: i32,
) -> Result<HashMap<String, HashMap<String, i64>>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT guild_id, discord_id, SUM(messages)::bigint FROM discord_message_counts
        WHERE guild_id = ANY($1) AND day > (NOW() AT TIME ZONE 'UTC')::date - $2
        GROUP BY guild_id, discord_id",
    )
    .bind(guild_ids)
    .bind(window(days))
    .fetch_all(state.database.read())
    .await?;

    let mut counts: HashMap<String, HashMap<String, i64>> =
        guild_ids.iter().map(|id| (id.clone(), HashMap::new())).collect();
    for (guild_id, discord_id, messages) in rows {
        counts.entry(guild_id).or_default().insert(discord_id, messages);
    }

    Ok(counts)
}

/// The days asked for, within 1 and [`KEEP_DAYS`].
pub fn window(days: i32) -> i32 {
    days.clamp(1, KEEP_DAYS)
}

/// Deletes counts older than [`KEEP_DAYS`].
pub async fn prune(state: &crate::State) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM discord_message_counts WHERE day < (NOW() AT TIME ZONE 'UTC')::date - $1",
    )
    .bind(KEEP_DAYS)
    .execute(state.database.write())
    .await?
    .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_stays_within_what_is_kept() {
        assert_eq!(window(0), 1);
        assert_eq!(window(7), 7);
        assert_eq!(window(365), KEEP_DAYS);
    }
}
