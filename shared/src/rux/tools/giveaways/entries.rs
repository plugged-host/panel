//! plugged.host: the giveaway post's buttons. Enter (`rux:gw:enter:<id>`) puts someone in after
//! the checks a draw makes too (still running, not kept out, linked); Leave (`rux:gw:leave:<id>`,
//! on the "you are already in" answer) takes them out again. The answer is only for them
//! (ephemeral) and is sent from a task of its own, so a slow account lookup (2 s at most) never
//! holds up the bot's other events and stays inside Discord's 3-second limit.

use super::super::credits::payouts;
use crate::rux::{
    config::Config,
    context::Me,
    discord::Rest,
    models::{GIVEAWAY_COLUMNS, Giveaway},
    say, text, time,
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::{Arc, LazyLock};

/// How long a "this Discord account is linked" answer is believed.
const LINKED_SECONDS: u64 = 600;

static CUSTOM_ID: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^rux:gw:(enter|leave):(\d{1,18})$").unwrap());

/// What a button press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Enter(i64),
    Leave(i64),
}

/// The press a button's `custom_id` stands for (`None`: not one of the giveaways' buttons).
pub fn parse_custom_id(custom_id: &str) -> Option<Press> {
    let found = CUSTOM_ID.captures(custom_id)?;
    let id = found[2].parse::<i64>().ok()?;

    Some(match &found[1] {
        "enter" => Press::Enter(id),
        _ => Press::Leave(id),
    })
}

/// An answer only the person who pressed sees.
fn ephemeral(content: String, components: Option<Value>) -> Value {
    let mut data = json!({
        "content": content,
        "flags": 64,
        "allowed_mentions": { "parse": [] },
    });
    if let Some(components) = components {
        data["components"] = components;
    }

    data
}

fn ends(giveaway: &Giveaway) -> String {
    time::discord(giveaway.ends.and_utc(), 'R')
}

fn already_in(config: &Config, giveaway: &Giveaway) -> Value {
    ephemeral(
        config.say("entry.already", &[("relative", &ends(giveaway))]),
        Some(json!([{
            "type": 1,
            "components": [{
                "type": 2,
                "style": 2,
                "label": config.say("entry.leave", &[]),
                "custom_id": format!("{}leave:{}", super::BUTTON_PREFIX, giveaway.id),
            }],
        }])),
    )
}

fn linked_key(discord_id: &str) -> String {
    format!("rux::giveaways::linked::{discord_id}")
}

/// Answers a press on a giveaway's button. `false` for anything that is not one; the answer
/// itself is sent from a task of its own.
pub async fn on_interaction(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    _me: &Me,
    interaction: &Value,
) -> Result<bool, anyhow::Error> {
    if interaction["type"].as_i64() != Some(3) {
        return Ok(false);
    }
    let Some(press) = interaction["data"]["custom_id"]
        .as_str()
        .and_then(parse_custom_id)
    else {
        return Ok(false);
    };
    let (Some(id), Some(token)) = (interaction["id"].as_str(), interaction["token"].as_str())
    else {
        return Ok(false);
    };

    let state = state.clone();
    let config = Arc::new(config.clone());
    let rest = rest.clone();
    let interaction = interaction.clone();
    let (id, token) = (id.to_string(), token.to_string());
    tokio::spawn(async move {
        let data = match answer(&state, &config, press, &interaction).await {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!("rux: a giveaway button failed: {err:#}");
                ephemeral(config.say("entry.unavailable", &[]), None)
            }
        };
        if let Err(err) = rest
            .interaction_callback(&id, &token, &json!({ "type": 4, "data": data }))
            .await
        {
            tracing::warn!("rux: could not answer a giveaway button: {err}");
        }
    });

    Ok(true)
}

async fn answer(
    state: &crate::State,
    config: &Config,
    press: Press,
    interaction: &Value,
) -> Result<Value, anyhow::Error> {
    let user = if interaction["member"]["user"].is_object() {
        &interaction["member"]["user"]
    } else {
        &interaction["user"]
    };
    let discord_id = user["id"].as_str().unwrap_or_default();
    let giveaway_id = match press {
        Press::Enter(id) | Press::Leave(id) => id,
    };
    let giveaway = sqlx::query_as::<_, Giveaway>(sqlx::AssertSqlSafe(format!(
        "SELECT {GIVEAWAY_COLUMNS} FROM rux_giveaways WHERE id = $1"
    )))
    .bind(giveaway_id)
    .fetch_optional(state.database.read())
    .await?
    // only from the giveaway's own server: a button anywhere else cannot enter it
    .filter(|giveaway| interaction["guild_id"].as_str() == Some(giveaway.guild_id.as_str()));

    let Some(giveaway) = giveaway.filter(|_| crate::discord::is_snowflake(discord_id)) else {
        return Ok(ephemeral(config.say("entry.closed", &[]), None));
    };
    if !giveaway.is_open(Utc::now().naive_utc()) {
        return Ok(ephemeral(config.say("entry.closed", &[]), None));
    }

    if let Press::Leave(_) = press {
        sqlx::query(
            "DELETE FROM rux_giveaway_entries WHERE giveaway_id = $1 AND discord_id = $2 AND won IS NULL",
        )
        .bind(giveaway.id)
        .bind(discord_id)
        .execute(state.database.write())
        .await?;

        return Ok(ephemeral(config.say("entry.left", &[]), None));
    }

    let entered: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM rux_giveaway_entries WHERE giveaway_id = $1 AND discord_id = $2)",
    )
    .bind(giveaway.id)
    .bind(discord_id)
    .fetch_one(state.database.read())
    .await?;
    if entered {
        return Ok(already_in(config, &giveaway));
    }
    if giveaway.excludes(discord_id) {
        return Ok(ephemeral(
            say::say_count(
                config.locale(),
                "entry.excluded",
                i64::from(giveaway.exclude_recent),
                &[],
            ),
            None,
        ));
    }

    if !state
        .cache
        .exists(&linked_key(discord_id))
        .await
        .unwrap_or(false)
    {
        match payouts::linked(state, &[discord_id.to_string()], 2).await {
            Err(err) => {
                tracing::warn!(
                    giveaway = giveaway.id,
                    "rux: could not check a giveaway entry: {}",
                    err.0
                );
                return Ok(ephemeral(config.say("entry.unavailable", &[]), None));
            }
            Ok(linked) if !linked.contains(discord_id) => {
                let link = config.link_url.starts_with("https://")
                    || config.link_url.starts_with("http://");

                return Ok(ephemeral(
                    config.say("entry.unlinked", &[]),
                    link.then(|| {
                        json!([{
                            "type": 1,
                            "components": [{
                                "type": 2,
                                "style": 5,
                                "label": config.say("entry.link", &[]),
                                "url": config.link_url,
                            }],
                        }])
                    }),
                ));
            }
            Ok(_) => {
                let _ = state
                    .cache
                    .set(&linked_key(discord_id), LINKED_SECONDS, &true)
                    .await;
            }
        }
    }

    let username = text::name(user["username"].as_str().unwrap_or_default(), 64);
    let joined: Option<i64> = sqlx::query_scalar(
        "INSERT INTO rux_giveaway_entries (giveaway_id, discord_id, username)
        VALUES ($1, $2, $3)
        ON CONFLICT (giveaway_id, discord_id) DO NOTHING
        RETURNING id",
    )
    .bind(giveaway.id)
    .bind(discord_id)
    .bind((!username.is_empty()).then_some(username))
    .fetch_optional(state.database.write())
    .await?;

    Ok(match joined {
        Some(_) => ephemeral(
            config.say("entry.joined", &[("relative", &ends(&giveaway))]),
            None,
        ),
        None => already_in(config, &giveaway),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_buttons_are_pressed() {
        assert_eq!(parse_custom_id("rux:gw:enter:12"), Some(Press::Enter(12)));
        assert_eq!(parse_custom_id("rux:gw:leave:3"), Some(Press::Leave(3)));
        for custom_id in [
            "rux:gw:enter:",
            "rux:gw:enter:x",
            "rux:gw:join:1",
            "rux:gw:enter:1:2",
            "rux:gw:enter:1234567890123456789",
            " rux:gw:enter:1",
            "neo:gw:enter:1",
        ] {
            assert_eq!(parse_custom_id(custom_id), None, "{custom_id}");
        }
        assert!("rux:gw:enter:1".starts_with(super::super::BUTTON_PREFIX));
    }

    #[test]
    fn answers_are_for_the_presser_only() {
        let answer = ephemeral("You are in.".into(), None);
        assert_eq!(answer["flags"], 64);
        assert_eq!(answer["allowed_mentions"]["parse"], json!([]));
        assert!(answer.get("components").is_none());
    }
}
