//! plugged.host: who is on a FiveM server now, from FXServer's own HTTP endpoints on the game
//! port: `/dynamic.json` (hostname, player count, slots) and `/players.json`
//! (docs/features/fivem.md).
//!
//! They are what the server browser reads, public by design. Since 2023 FXServer leaves the
//! identifiers out of players.json unless `sv_exposePlayerIdentifiersInHttpEndpoint` is set, so
//! they are shown when there and nothing is promised otherwise.

use crate::models::server::Server;
use serde::Serialize;
use utoipa::ToSchema;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

#[derive(ToSchema, Serialize, Debug, Clone, PartialEq)]
pub struct Player {
    pub id: i64,
    pub name: String,
    pub ping: i64,
    pub identifiers: Vec<String>,
}

#[derive(ToSchema, Serialize, Debug, Clone, Default, PartialEq)]
pub struct Players {
    /// FXServer answered.
    pub online: bool,
    pub hostname: Option<String>,
    pub clients: i64,
    pub slots: Option<i64>,
    pub players: Vec<Player>,
}

/// FiveM's `^0`–`^9` colour codes, which names and hostnames are full of.
pub fn strip_colors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' && chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
            continue;
        }
        out.push(c);
    }

    out.trim().to_string()
}

fn valid_identifier(identifier: &str) -> bool {
    identifier.split_once(':').is_some_and(|(kind, id)| {
        !kind.is_empty()
            && kind.bytes().all(|b| b.is_ascii_alphanumeric())
            && !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    })
}

fn integer(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// The two answers, read. `list` is `None` when players.json did not answer.
pub fn parse(dynamic: &serde_json::Value, list: Option<&serde_json::Value>) -> Players {
    let mut players: Vec<Player> = list
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|player| player.is_object())
        .map(|player| Player {
            id: integer(player.get("id")).unwrap_or_default(),
            // names are the players' own: plain text, and short
            name: strip_colors(
                player
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            )
            .chars()
            .take(64)
            .collect(),
            ping: integer(player.get("ping")).unwrap_or_default(),
            identifiers: player
                .get("identifiers")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .filter(|identifier| valid_identifier(identifier))
                .map(str::to_string)
                .collect(),
        })
        .collect();
    players.sort_by_key(|player| player.id);

    Players {
        online: true,
        hostname: dynamic
            .get("hostname")
            .and_then(serde_json::Value::as_str)
            .map(strip_colors)
            .filter(|hostname| !hostname.is_empty()),
        clients: integer(dynamic.get("clients")).unwrap_or(players.len() as i64),
        slots: integer(dynamic.get("sv_maxclients")),
        players,
    }
}

/// The address FXServer answers on: the primary allocation's alias or IP, the node's host for an
/// allocation bound to every address.
async fn base_url(state: &crate::State, server: &Server) -> Option<String> {
    let allocation = &server.allocation.as_ref()?.allocation;
    let host = match &allocation.ip_alias {
        Some(alias) if !alias.trim().is_empty() => alias.trim().to_string(),
        _ if !allocation.ip.ip().is_unspecified() => allocation.ip.ip().to_string(),
        _ => server
            .node
            .fetch_cached(&state.database)
            .await
            .ok()?
            .url
            .host_str()?
            .to_string(),
    };
    let host = match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V6(_)) if !host.starts_with('[') => format!("[{host}]"),
        _ => host,
    };

    Some(format!("http://{host}:{}", allocation.port))
}

pub async fn fetch(state: &crate::State, server: &Server) -> Players {
    let Some(base) = base_url(state, server).await else {
        return Players::default();
    };
    let get = |path: &'static str| {
        let request = state.client.get(format!("{base}/{path}")).timeout(TIMEOUT);
        async move {
            request
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .json::<serde_json::Value>()
                .await
                .ok()
        }
    };

    let (dynamic, list) = tokio::join!(get("dynamic.json"), get("players.json"));
    match dynamic {
        Some(dynamic) => parse(&dynamic, list.as_ref()),
        None => Players::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_go_and_identifiers_are_checked() {
        assert_eq!(strip_colors(" ^1Red^7 Server^ "), "Red Server^");
        let players = parse(
            &serde_json::json!({"hostname": "^2My ^0Server", "clients": 2, "sv_maxclients": "48"}),
            Some(&serde_json::json!([
                {"id": 7, "name": "^3Zed", "ping": 40, "identifiers": ["license:abc", "bad id", "discord:12"]},
                {"id": 2, "name": "Ann", "ping": "12"},
                "junk"
            ])),
        );
        assert_eq!(players.hostname.as_deref(), Some("My Server"));
        assert_eq!((players.clients, players.slots), (2, Some(48)));
        assert_eq!(players.players[0].name, "Ann");
        assert_eq!(players.players[0].ping, 12);
        assert_eq!(
            players.players[1].identifiers,
            vec!["license:abc", "discord:12"]
        );

        let empty = parse(&serde_json::json!({}), None);
        assert!(empty.online && empty.players.is_empty() && empty.clients == 0);
    }
}
