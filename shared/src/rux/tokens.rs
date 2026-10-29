//! plugged.host: rux and the panel's tokens (docs/features/rux.md#tokens). `/tokens` shows a
//! member whose Discord is linked to a panel account their tokens, today's daily streak and their
//! membership, only to them; [`announce_bonus`] posts a double-token window once when it starts,
//! in the settings' announcement channel.

use super::{config::Config, discord::Rest, say, say::Strings, text, time};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::sync::Arc;

/// The slash command's name.
pub const COMMAND: &str = "tokens";

/// The embeds' colour (the giveaway post's accent).
const ACCENT: u32 = super::tools::giveaways::service::ACCENT;

/// The lines of `/tokens` and the double-token post: `(key, English, German)`.
pub const STRINGS: Strings = &[
    ("tokens.title", "Your tokens", "Deine Tokens"),
    ("tokens.balance", "Balance", "Guthaben"),
    (
        "tokens.balance_value",
        "1 token|:count tokens",
        "1 Token|:count Tokens",
    ),
    ("tokens.streak", "Daily streak", "Tägliche Serie"),
    (
        "tokens.streak_ready",
        "Day :day is ready: claim it on the Tokens page today.",
        "Tag :day ist bereit: Hol ihn heute auf der Tokens-Seite ab.",
    ),
    (
        "tokens.streak_done",
        "Day :day claimed today. Come back tomorrow.",
        "Tag :day heute abgeholt. Komm morgen wieder.",
    ),
    ("tokens.membership", "Membership", "Mitgliedschaft"),
    (
        "tokens.membership_until",
        ":membership, ends :relative",
        ":membership, endet :relative",
    ),
    ("tokens.membership_none", "None", "Keine"),
    ("tokens.open", "Open the Tokens page", "Tokens-Seite öffnen"),
    (
        "tokens.unlinked",
        "Your Discord account is not linked to an :app account yet. Link it at :link, then try again.",
        "Dein Discord-Konto ist noch mit keinem :app-Konto verknüpft. Verknüpfe es auf :link und versuch es dann noch einmal.",
    ),
    (
        "tokens.off",
        "Tokens are turned off on :app.",
        "Tokens sind auf :app ausgeschaltet.",
    ),
    (
        "tokens.command_off",
        "This command is turned off.",
        "Dieser Befehl ist ausgeschaltet.",
    ),
    (
        "tokens.unavailable",
        "That does not work right now. Try again in a minute.",
        "Das klappt gerade nicht. Versuch es in einer Minute noch einmal.",
    ),
    // the double-token post
    ("bonus.double", "Double tokens", "Doppelte Tokens"),
    ("bonus.more", "+:percent% tokens", "+:percent % Tokens"),
    (
        "bonus.until",
        "Every token you earn counts :times× until :absolute (:relative).",
        "Jeder Token, den du verdienst, zählt :times× bis :absolute (:relative).",
    ),
    (
        "bonus.open",
        "Every token you earn counts :times× for a limited time.",
        "Jeder Token, den du verdienst, zählt für kurze Zeit :times×.",
    ),
    ("bonus.earn", "Earn them at :url", "Verdiene sie auf :url"),
];

/// The `/tokens` command (Discord's application command object), registered on the gateway's
/// server after each READY.
pub fn command() -> Value {
    json!({
        "name": COMMAND,
        "type": 1,
        "description": "Show your tokens, today's streak and your membership (only you see it)",
        "description_localizations": {
            "de": "Zeigt deine Tokens, die heutige Serie und deine Mitgliedschaft (nur für dich)",
        },
    })
}

/// An answer only the person who asked sees.
fn ephemeral(content: String) -> Value {
    json!({
        "content": content,
        "flags": 64,
        "allowed_mentions": { "parse": [] },
    })
}

/// An `INTERACTION_CREATE` that is `/tokens`; `false` for anything else. Answered from a task of
/// its own, so the account lookups never hold up the gateway's other events and stay inside
/// Discord's 3 seconds.
pub fn on_command(state: &crate::State, config: &Config, rest: &Rest, interaction: &Value) -> bool {
    if interaction["type"].as_i64() != Some(2)
        || interaction["data"]["name"].as_str() != Some(COMMAND)
    {
        return false;
    }
    let (Some(id), Some(token)) = (interaction["id"].as_str(), interaction["token"].as_str())
    else {
        return false;
    };
    let discord_id = interaction["member"]["user"]["id"]
        .as_str()
        .or(interaction["user"]["id"].as_str())
        .unwrap_or_default()
        .to_string();

    let state = state.clone();
    let config = Arc::new(config.clone());
    let rest = rest.clone();
    let (id, token) = (id.to_string(), token.to_string());
    tokio::spawn(async move {
        let data = match answer(&state, &config, &discord_id).await {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!("rux: /tokens failed: {err:#}");
                ephemeral(config.say("tokens.unavailable", &[]))
            }
        };
        if let Err(err) = rest
            .interaction_callback(&id, &token, &json!({ "type": 4, "data": data }))
            .await
        {
            tracing::warn!("rux: could not answer /tokens: {err}");
        }
    });

    true
}

/// What `/tokens` answers `discord_id`.
async fn answer(
    state: &crate::State,
    config: &Config,
    discord_id: &str,
) -> Result<Value, anyhow::Error> {
    let app = config.app_name.as_str();
    if !config.settings.tokens_command {
        return Ok(ephemeral(config.say("tokens.command_off", &[])));
    }
    let (economy, app_url) = {
        let settings = state.settings.get().await?;
        (
            settings.economy.clone(),
            settings.app.url.trim_end_matches('/').to_string(),
        )
    };
    if !economy.tokens_enabled {
        return Ok(ephemeral(config.say("tokens.off", &[("app", app)])));
    }
    if !crate::discord::is_snowflake(discord_id) {
        return Ok(ephemeral(config.say("tokens.unavailable", &[])));
    }

    // the panel account the Discord account is linked to (noprofit.space's link while it is
    // connected, else the Discord section's sign-in provider): tokens are always the panel's
    let Some(user_uuid) = super::tools::credits::payouts::panel_user(state, discord_id).await?
    else {
        return Ok(ephemeral(config.say(
            "tokens.unlinked",
            &[("app", app), ("link", &config.link_url)],
        )));
    };

    let locale = config.locale();
    let tokens: i64 = sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1")
        .bind(user_uuid)
        .fetch_optional(state.database.read())
        .await?
        .unwrap_or(0);

    let mut fields = vec![json!({
        "name": config.say("tokens.balance", &[]),
        "value": say::say_count(locale, "tokens.balance_value", tokens, &[]),
        "inline": true,
    })];

    if let Some(streak) = crate::streak::status(state, &economy, user_uuid).await? {
        let day = streak.day.to_string();
        fields.push(json!({
            "name": config.say("tokens.streak", &[]),
            "value": config.say(
                if streak.claimable { "tokens.streak_ready" } else { "tokens.streak_done" },
                &[("day", &day)],
            ),
            "inline": true,
        }));
    }

    if economy.membership_enabled {
        let membership = crate::economy::membership(&state.database, user_uuid).await?;
        let now = Utc::now().naive_utc();
        let running = if membership.has_plusplus(now) {
            membership
                .plusplus_until
                .map(|until| (config.memberships.plusplus.as_str(), until))
        } else if membership.has_plus(now) {
            membership
                .plus_runs_until() // plugged.host: bought or with the membership bundle
                .map(|until| (config.memberships.plus.as_str(), until))
        } else {
            None
        };

        fields.push(json!({
            "name": config.say("tokens.membership", &[]),
            "value": match running {
                Some((name, until)) => config.say(
                    "tokens.membership_until",
                    &[
                        ("membership", name),
                        ("relative", &time::discord(until.and_utc(), 'R')),
                    ],
                ),
                None => config.say("tokens.membership_none", &[]),
            },
            "inline": false,
        }));
    }

    let mut data = json!({
        "embeds": [{
            "title": config.say("tokens.title", &[]),
            "color": ACCENT,
            "fields": fields,
        }],
        "flags": 64,
        "allowed_mentions": { "parse": [] },
    });
    if app_url.starts_with("https://") || app_url.starts_with("http://") {
        data["components"] = json!([{
            "type": 1,
            "components": [{
                "type": 2,
                "style": 5,
                "label": config.say("tokens.open", &[]),
                "url": format!("{app_url}/account/tokens"),
            }],
        }]);
    }

    Ok(data)
}

/// A double-token window as the economy settings describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bonus {
    /// Percent more on every token earned (100: double).
    pub percent: i64,
    pub name: String,
    pub starts: Option<DateTime<Utc>>,
    pub ends: Option<DateTime<Utc>>,
}

impl Bonus {
    /// The cache key that remembers the window was announced: a changed window (percent, start or
    /// end) is a new one and is announced again.
    fn key(&self) -> String {
        let at = |time: Option<DateTime<Utc>>| {
            time.map_or_else(|| "open".to_string(), |time| time.timestamp().to_string())
        };

        format!(
            "rux::token_bonus::announced::{}::{}::{}",
            self.percent,
            at(self.starts),
            at(self.ends)
        )
    }

    /// How many times a token counts ("2", "1.5"), in the post's language.
    fn times(&self, locale: &str) -> String {
        let hundredths = 100 + self.percent;
        let mut times = format!("{}.{:02}", hundredths / 100, hundredths % 100);
        while times.ends_with('0') {
            times.pop();
        }
        if times.ends_with('.') {
            times.pop();
        }

        if locale == "de" {
            times.replace('.', ",")
        } else {
            times
        }
    }
}

impl From<crate::economy::ApiTokenBonus> for Bonus {
    fn from(bonus: crate::economy::ApiTokenBonus) -> Self {
        Self {
            percent: bonus.percent,
            // one line, as a post's title takes it
            name: bonus
                .name
                .map(|name| {
                    text::cut(
                        &name.split_whitespace().collect::<Vec<_>>().join(" "),
                        100,
                        "",
                    )
                })
                .unwrap_or_default(),
            starts: bonus.starts,
            ends: bonus.ends,
        }
    }
}

/// The post for a window ([`Bonus`], from [`crate::economy::token_bonus_at`]).
pub fn bonus_payload(config: &Config, bonus: &Bonus, app_url: &str) -> Value {
    let locale = config.locale();
    let percent = bonus.percent.to_string();
    let mut title = if bonus.percent == 100 {
        config.say("bonus.double", &[])
    } else {
        config.say("bonus.more", &[("percent", &percent)])
    };
    if !bonus.name.is_empty() {
        title = format!("{title}: {}", bonus.name);
    }

    let times = bonus.times(locale);
    let mut description = match bonus.ends {
        Some(ends) => config.say(
            "bonus.until",
            &[
                ("times", &times),
                ("absolute", &time::discord(ends, 'f')),
                ("relative", &time::discord(ends, 'R')),
            ],
        ),
        None => config.say("bonus.open", &[("times", &times)]),
    };
    if app_url.starts_with("https://") || app_url.starts_with("http://") {
        description.push_str("\n\n");
        description.push_str(&config.say(
            "bonus.earn",
            &[("url", &format!("{app_url}/account/tokens"))],
        ));
    }

    json!({
        "content": "",
        "embeds": [{
            "title": text::cut(&title, 256, ""),
            "description": description,
            "color": ACCENT,
        }],
        "allowed_mentions": { "parse": [] },
    })
}

/// Once a minute (`rux::tick`): posts the double-token window that runs now
/// ([`crate::economy::token_bonus_at`]) in the announcement channel, once per window (remembered in
/// the cache until a day after it ends). Nothing while the posts are off, no channel is set or
/// tokens are off.
pub async fn announce_bonus(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
) -> Result<(), anyhow::Error> {
    let settings = &config.settings;
    let channel = settings.announce_channel_id.trim();
    if !settings.token_bonus_posts || !crate::discord::is_snowflake(channel) {
        return Ok(());
    }

    let now = Utc::now();
    let (bonus, app_url) = {
        let all = state.settings.get().await?;
        (
            // the window running now (`None` while tokens are off)
            crate::economy::token_bonus_at(&all.economy, now).map(Bonus::from),
            all.app.url.trim_end_matches('/').to_string(),
        )
    };
    let Some(bonus) = bonus else {
        return Ok(());
    };
    let key = bonus.key();
    // a cache that cannot say posts nothing: better no post than the same one every minute
    if state.cache.exists(&key).await.unwrap_or(true) {
        return Ok(());
    }

    let remember = bonus
        .ends
        .map_or(30 * 86_400, |ends| {
            (ends - now).num_seconds().max(0) + 86_400
        })
        .max(86_400) as u64;
    let posted = rest
        .create_message(channel, &bonus_payload(config, &bonus, &app_url))
        .await;
    match posted {
        Ok(_) => {
            tracing::info!(
                percent = bonus.percent,
                "rux: announced a double-token window"
            );
            state.cache.set(&key, remember, &true).await?;
        }
        Err(err) => {
            tracing::warn!("rux: could not announce the double-token window: {err}");
            // tried again in an hour, not every minute
            state.cache.set(&key, 3600, &true).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_window_name_is_one_line() {
        let bonus = Bonus::from(crate::economy::ApiTokenBonus {
            percent: 100,
            name: Some("  Halloween \n weekend ".into()),
            starts: None,
            ends: Some(at("2026-11-01T22:00:00Z")),
        });
        assert_eq!(bonus.percent, 100);
        assert_eq!(bonus.name, "Halloween weekend");
        assert_eq!(bonus.ends, Some(at("2026-11-01T22:00:00Z")));
    }

    #[test]
    fn a_changed_window_is_a_new_one() {
        let bonus = Bonus {
            percent: 100,
            name: String::new(),
            starts: None,
            ends: Some(at("2026-11-01T22:00:00Z")),
        };
        let mut later = bonus.clone();
        later.ends = Some(at("2026-11-02T22:00:00Z"));
        assert_ne!(bonus.key(), later.key());
        let mut renamed = bonus.clone();
        renamed.name = "Halloween".into();
        assert_eq!(bonus.key(), renamed.key());
    }

    #[test]
    fn the_post_says_how_much_a_token_counts() {
        let bonus = Bonus {
            percent: 100,
            name: "Halloween".into(),
            starts: None,
            ends: Some(at("2026-11-01T22:00:00Z")),
        };
        assert_eq!(bonus.times("en"), "2");
        assert_eq!(
            Bonus {
                percent: 50,
                ..bonus.clone()
            }
            .times("de"),
            "1,5"
        );
        assert_eq!(
            Bonus {
                percent: 25,
                ..bonus.clone()
            }
            .times("en"),
            "1.25"
        );

        let config = super::super::tools::giveaways::service::tests::config("en");
        let payload = bonus_payload(&config, &bonus, "https://panel.example.com");
        let embed = &payload["embeds"][0];
        assert_eq!(embed["title"], "Double tokens: Halloween");
        assert_eq!(
            embed["description"],
            "Every token you earn counts 2× until <t:1793570400:f> (<t:1793570400:R>).\n\nEarn them at https://panel.example.com/account/tokens"
        );
        assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
    }

    #[test]
    fn the_command_is_a_chat_input_command() {
        let command = command();
        assert_eq!(command["name"], COMMAND);
        assert_eq!(command["type"], 1);
        assert!(command["description"].as_str().unwrap().chars().count() <= 100);
        assert!(
            command["description_localizations"]["de"]
                .as_str()
                .unwrap()
                .chars()
                .count()
                <= 100
        );
    }
}
