//! plugged.host: the watched channels of a server (`rux_watches`), where every member message is
//! checked. A row names its channel by id or by name ("#support", as the settings may write it);
//! [`watched_channel_ids`] resolves both among the server's text and announcement channels and
//! keeps the answer a minute.

use super::super::ToolError;
use crate::rux::{discord::Rest, resolver};

const SECONDS: u64 = 60;
/// Text and announcement channels: what a name can mean.
const TYPES: [i64; 2] = [0, 5];

fn key(guild_id: &str) -> String {
    format!("rux::watches::{guild_id}")
}

/// Whether a watch row names a channel by id.
pub fn is_id(channel: &str) -> bool {
    (17..=20).contains(&channel.len()) && channel.bytes().all(|b| b.is_ascii_digit())
}

/// The ids among `channels` (Discord's channel objects) that a watch row names.
pub fn resolve(wanted: &[String], channels: &[serde_json::Value]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut add = |id: String| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };

    for channel in wanted {
        if is_id(channel) {
            add(channel.clone());
            continue;
        }

        let plain = resolver::plain(channel.trim_start_matches('#'));
        if plain.is_empty() {
            continue;
        }
        for candidate in channels {
            if candidate["type"]
                .as_i64()
                .is_some_and(|kind| TYPES.contains(&kind))
                && resolver::plain(candidate["name"].as_str().unwrap_or_default()) == plain
                && let Some(id) = candidate["id"].as_str()
            {
                add(id.to_string());
            }
        }
    }

    ids
}

/// The ids of a server's watched channels (`fresh`: not from the minute's cache).
pub async fn watched_channel_ids(
    state: &crate::State,
    rest: &Rest,
    guild_id: &str,
    fresh: bool,
) -> Result<Vec<String>, ToolError> {
    if !fresh && let Ok(Some(ids)) = state.cache.get::<Vec<String>>(&key(guild_id)).await {
        return Ok(ids);
    }

    let wanted: Vec<String> =
        sqlx::query_scalar("SELECT channel FROM rux_watches WHERE guild_id = $1 ORDER BY id")
            .bind(guild_id)
            .fetch_all(state.database.read())
            .await?;
    let channels = if wanted.iter().all(|channel| is_id(channel)) {
        Vec::new()
    } else {
        rest.channels(guild_id, fresh).await?
    };

    let ids = resolve(&wanted, &channels);
    let _ = state.cache.set(&key(guild_id), SECONDS, &ids).await;

    Ok(ids)
}

/// Forgets the minute's answer (after a watch was added or removed).
pub async fn forget(state: &crate::State, guild_id: &str) {
    let _ = state.cache.invalidate(&key(guild_id)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_name_channels_by_id_or_name() {
        let channels = vec![
            json!({ "id": "100000000000000011", "name": "support", "type": 0 }),
            json!({ "id": "100000000000000012", "name": "Support", "type": 2 }),
            json!({ "id": "100000000000000013", "name": "news", "type": 5 }),
        ];
        let wanted = vec![
            "#Support".to_string(),
            "100000000000000013".to_string(),
            "news".to_string(),
            "#".to_string(),
            "gone".to_string(),
        ];

        assert_eq!(
            resolve(&wanted, &channels),
            vec!["100000000000000011", "100000000000000013"]
        );
        assert!(is_id("100000000000000013"));
        assert!(!is_id("1000"));
    }
}
