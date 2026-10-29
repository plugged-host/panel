//! plugged.host: where the credits rux gives are paid. While noprofit.space is connected, its rux
//! routes pay (the credits live there); without it, the panel's own credits
//! ([`crate::economy`]), when credits are on. Giveaways of tokens or membership days are always
//! paid by the panel ([`Payer::Panel`]), since tokens and memberships are always its own. Also
//! who is linked (only linked accounts can enter a giveaway or get credits) and who won the last
//! giveaways.

use super::super::ToolError;
use crate::rux::noprofit::{Client, Unavailable, parse_accounts};
use serde_json::{Value, json};
use std::collections::HashSet;

/// Who pays.
pub enum Payer {
    /// noprofit.space's rux routes.
    Noprofit(Client),
    /// The panel's own credits.
    Panel,
}

/// Who pays what rux gives right now; refused when nobody can (noprofit.space connected without
/// its rux routes set up, or the panel's credits off).
pub async fn payer(state: &crate::State) -> Result<Payer, ToolError> {
    let (connected, credits_enabled) = {
        let settings = state.settings.get().await?;
        (
            settings.noprofit.config().is_some(),
            settings.economy.credits_enabled,
        )
    };

    if connected {
        return match Client::load(state).await? {
            Some(client) if client.configured() => Ok(Payer::Noprofit(client)),
            _ => Err(ToolError::Unavailable(Unavailable(
                "noprofit.space is connected but its rux routes are not set up (no rux or API secret), so no credits can be paid. Tell an admin."
                    .into(),
            ))),
        };
    }
    if !credits_enabled {
        return Err(ToolError::invalid(
            "Credits are turned off on this panel, so no credits can be given. Tell an admin.",
        ));
    }

    Ok(Payer::Panel)
}

fn failed(err: impl std::fmt::Display) -> Unavailable {
    Unavailable(format!("the account directory failed: {err}"))
}

/// The ones among these Discord ids that are linked to an account (noprofit.space's while it
/// is connected and its rux routes are set up, else the panel's own records).
pub async fn linked(
    state: &crate::State,
    discord_ids: &[String],
    timeout: u64,
) -> Result<HashSet<String>, Unavailable> {
    let mut found = HashSet::new();
    if discord_ids.is_empty() {
        return Ok(found);
    }

    if let Some(client) = Client::load(state).await.map_err(failed)?
        && client.configured()
    {
        for chunk in discord_ids.chunks(1000) {
            let answer = client
                .post("/accounts", &json!({ "discord_ids": chunk }), timeout)
                .await?;
            found.extend(parse_accounts(&answer).into_keys());
        }

        return Ok(found);
    }

    let context = crate::discord::Context::load(state).await.map_err(failed)?;
    let rows: Vec<String> = if context.connected {
        sqlx::query_scalar(
            "SELECT DISTINCT discord->>'id' FROM user_noprofit_accounts
            WHERE discord->>'id' = ANY($1) AND deleted IS NULL",
        )
        .bind(discord_ids)
        .fetch_all(state.database.read())
        .await
        .map_err(failed)?
    } else if let Some(provider) = context.settings.oauth_provider_uuid {
        sqlx::query_scalar(
            "SELECT DISTINCT identifier FROM user_oauth_links
            WHERE oauth_provider_uuid = $1 AND identifier = ANY($2)",
        )
        .bind(provider)
        .bind(discord_ids)
        .fetch_all(state.database.read())
        .await
        .map_err(failed)?
    } else {
        Vec::new()
    };
    found.extend(rows);

    Ok(found)
}

/// The panel account a Discord account is linked to (the panel paying itself).
pub async fn panel_user(
    state: &crate::State,
    discord_id: &str,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let context = crate::discord::Context::load(state).await?;

    Ok(crate::discord::user_for_discord_id(state, &context, discord_id).await?)
}

/// One of the last giveaways and its winners.
#[derive(Debug, Clone, PartialEq)]
pub struct Recent {
    pub id: i64,
    pub prize: String,
    /// When it was drawn (ISO 8601).
    pub at: Option<String>,
    pub winners: Vec<String>,
}

/// The giveaways in a `/giveaways/recent` answer.
pub fn parse_recent(answer: &Value) -> Vec<Recent> {
    answer["giveaways"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|giveaway| {
            Some(Recent {
                id: giveaway["id"].as_i64()?,
                prize: giveaway["prize"].as_str().unwrap_or_default().to_string(),
                at: giveaway["created_at"].as_str().map(str::to_string),
                winners: giveaway["winners"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| match id {
                        Value::String(id) => Some(id.clone()),
                        Value::Number(id) => Some(id.to_string()),
                        _ => None,
                    })
                    .collect(),
            })
        })
        .collect()
}

/// The last `limit` giveaways that had winners, newest first: noprofit.space's (every giveaway
/// paid there), or the panel's own rux giveaways.
pub async fn recent_giveaways(
    state: &crate::State,
    payer: &Payer,
    limit: i64,
) -> Result<Vec<Recent>, ToolError> {
    match payer {
        Payer::Noprofit(client) => {
            let answer = client
                .post(
                    "/giveaways/recent",
                    &json!({ "limit": limit.clamp(1, 20) }),
                    8,
                )
                .await?;

            Ok(parse_recent(&answer))
        }
        Payer::Panel => {
            let rows: Vec<(i64, String, Option<chrono::NaiveDateTime>, Vec<String>)> =
                sqlx::query_as(
                    "SELECT g.id, g.prize, g.ended, array_agg(e.discord_id::text ORDER BY e.id)
                    FROM rux_giveaways g
                    JOIN rux_giveaway_entries e ON e.giveaway_id = g.id AND e.won IS NOT NULL
                    WHERE g.status = 'ended'
                    GROUP BY g.id
                    ORDER BY g.ended DESC NULLS LAST, g.id DESC
                    LIMIT $1",
                )
                .bind(limit.clamp(1, 20))
                .fetch_all(state.database.read())
                .await?;

            Ok(rows
                .into_iter()
                .map(|(id, prize, ended, winners)| Recent {
                    id,
                    prize,
                    at: ended.map(|ended| ended.and_utc().to_rfc3339()),
                    winners,
                })
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_giveaways_are_read() {
        let answer = json!({"giveaways": [
            {"id": 7, "prize": "500 credits", "created_at": "2026-10-04T10:00:00+00:00", "winners": ["100000000000000002", 100000000000000003i64]},
            {"prize": "no id"},
        ]});

        assert_eq!(
            parse_recent(&answer),
            vec![Recent {
                id: 7,
                prize: "500 credits".into(),
                at: Some("2026-10-04T10:00:00+00:00".into()),
                winners: vec!["100000000000000002".into(), "100000000000000003".into()],
            }]
        );
        assert!(parse_recent(&Value::Null).is_empty());
    }
}
