//! plugged.host: rux, the moderation and toolbox bot on Discord (docs/features/rux.md).
//!
//! rux runs on the Discord section's moderation bot (its own token and switch, Admin › Settings ›
//! Accounts › Discord) and works on the servers of its own section (Admin › Settings › Accounts ›
//! rux): the main one and up to nine more. Admins (noprofit.space's, or the panel's without it)
//! have it do anything by DM, @mention or "hey rux"; members of the roles they make rux moderator
//! roles have it moderate their server. It also checks members' messages by itself (spam, watched
//! channels, standing rules). Off by default: nothing runs until the moderation bot is switched on
//! with a token and rux has a main server.
//!
//! The parts:
//!  - [`handler`]: the moderation bot's gateway handler ([`register`]): orders to the [`inbox`] and
//!    the [`agent`], every message of rux's servers to the [`watch`], giveaway buttons to the
//!    giveaways.
//!  - [`inbox`] (which messages are orders), [`agent`] (one order, start to finish), [`prompt`]
//!    (its system prompt), [`tools`] (what the model can have done: the [`tools::Tool`] trait,
//!    the [`tools::Registry`] and the tool groups).
//!  - [`access`]: who rux obeys and whom it leaves alone; [`context::Context`]: who ordered what,
//!    where, handed to every tool.
//!  - [`ai`]: neo's providers in rux's own scope (premium models first for orders, never for
//!    checks).
//!  - [`config`] and [`noprofit`]: the settings rux runs on (the panel's, or noprofit.space's laid
//!    over them while it keeps them) and noprofit.space's rux routes.
//!  - [`discord`] (Discord's REST API with rux's token) and [`resolver`] (what an admin wrote,
//!    made into the Discord object it means).
//!  - [`models`]: the `rux_*` tables; [`say`] (what rux posts, English and German), [`text`],
//!    [`time`], [`images`]: helpers.
//!
//! [`tick`] does the housekeeping (orders that lost their worker, the tool groups' and the
//! watch's ticks, old messages), every ten seconds on the primary instance.

pub mod access;
pub mod agent;
pub mod ai;
pub mod config;
pub mod context;
pub mod discord;
mod handler;
pub mod images;
pub mod inbox;
pub mod models;
pub mod noprofit;
pub mod prompt;
pub mod resolver;
pub mod say;
pub mod text;
pub mod time;
pub mod tools;
pub mod watch;

pub use handler::{me, register};

use config::Config;
use discord::Rest;
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use utoipa::ToSchema;

/// Why rux does not run (or cannot do its job), for Admin › Settings › rux.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    /// The Discord section is off or names no server.
    DiscordOff,
    /// The Discord section's moderation bot is switched off.
    BotOff,
    /// The moderation bot has no token.
    NoToken,
    /// rux has no main server (its own, or the Discord section's).
    NoServer,
    /// noprofit.space keeps rux's settings and switched it off there.
    SwitchedOff,
    /// Nobody can be told an admin: without noprofit.space, Discord accounts are linked through
    /// the Discord section's OAuth provider, and none is set.
    NoAccounts,
    /// No AI provider rux can use: it cannot think.
    NoAi,
}

/// What stops rux from running, from the settings alone (no network).
pub fn blockers(
    discord: &crate::settings::discord::AppSettingsDiscord,
    config: &Config,
    noprofit_connected: bool,
    ai_usable: bool,
) -> Vec<Blocker> {
    let mut blockers = Vec::new();
    if !discord.active() {
        blockers.push(Blocker::DiscordOff);
    }
    if !discord.moderation_bot_enabled {
        blockers.push(Blocker::BotOff);
    }
    if discord
        .moderation_bot_token
        .as_deref()
        .is_none_or(|token| token.trim().is_empty())
    {
        blockers.push(Blocker::NoToken);
    }
    if config.guilds.is_empty() {
        blockers.push(Blocker::NoServer);
    }
    if !config.enabled {
        blockers.push(Blocker::SwitchedOff);
    }
    if !noprofit_connected && discord.oauth_provider_uuid.is_none() {
        blockers.push(Blocker::NoAccounts);
    }
    if !ai_usable {
        blockers.push(Blocker::NoAi);
    }

    blockers
}

/// The settings rux runs on and its REST client, while it runs: the moderation bot is on with a
/// token, rux has a main server and (while noprofit.space keeps its settings) is on there. The
/// gateway only connects the bot while the Discord section is on.
pub async fn running(state: &crate::State) -> Result<Option<(Arc<Config>, Rest)>, anyhow::Error> {
    let bot_on = {
        let settings = state.settings.get().await?;
        settings.discord.active() && settings.discord.moderation_bot_enabled
    };
    if !bot_on {
        return Ok(None);
    }

    let config = config::load(state).await?;
    if !config.enabled || config.guilds.is_empty() {
        return Ok(None);
    }
    let Some(rest) = Rest::load(state).await? else {
        return Ok(None);
    };

    Ok(Some((config, rest)))
}

/// When the last once-a-minute part of [`tick`] ran (unix seconds; the task runs on the primary
/// instance only, so memory is enough).
static LAST_MINUTE: AtomicI64 = AtomicI64::new(0);

/// Orders still pending this long after they came in are given up.
const ORDER_GIVE_UP_MINUTES: i64 = 10;
/// Orders still pending this long after they came in lost their worker (a restart) and get a
/// new one.
const ORDER_RETRY_MINUTES: i64 = 1;
/// Orders still running this long after they came in lost their worker mid-way (an order ends
/// within [`agent::ORDER_DEADLINE`]).
const ORDER_RUNNING_MINUTES: i64 = 15;
/// rux's short memory is kept this long.
const MESSAGES_DAYS: i64 = 30;

/// rux's housekeeping, for a background task every ten seconds on the primary instance (where
/// the gateway runs). Every run: the giveaways' due draws ([`tools::giveaways::due`]). Once a
/// minute: orders that lost their worker get a new one (or are given up), then the tool groups'
/// and the watch's ticks. Once a day: messages older than 30 days are deleted. One run at a time;
/// nothing while rux does not run.
pub async fn tick(state: &crate::State) -> Result<(), anyhow::Error> {
    let Ok(_lock) = state.cache.lock("rux::tick", Some(300), Some(100)).await else {
        return Ok(());
    };
    let Some((config, rest)) = running(state).await? else {
        return Ok(());
    };

    if let Err(err) = tools::giveaways::due(state, &config, &rest).await {
        tracing::warn!("rux: giveaway draws failed: {err:#}");
    }

    let now = chrono::Utc::now().timestamp();
    if now - LAST_MINUTE.load(Ordering::Relaxed) < 55 {
        return Ok(());
    }
    LAST_MINUTE.store(now, Ordering::Relaxed);

    if let Err(err) = recover_orders(state).await {
        tracing::warn!("rux: recovering orders failed: {err:#}");
    }
    if let Err(err) = tools::giveaways::tick(state, &config, &rest).await {
        tracing::warn!("rux: the giveaways' tick failed: {err:#}");
    }
    if let Err(err) = tools::moderation::tick(state, &config, &rest).await {
        tracing::warn!("rux: the moderation tick failed: {err:#}");
    }
    if let Err(err) = watch::tick(state, &config, &rest).await {
        tracing::warn!("rux: the watch's tick failed: {err:#}");
    }

    // once a day (the limiter lets one run through per day)
    if state
        .cache
        .ratelimit("rux::prune", 2, 86400, "messages")
        .await
        .is_ok()
    {
        let deleted = sqlx::query(
            "DELETE FROM rux_messages WHERE created < NOW() - make_interval(days => $1)",
        )
        .bind(MESSAGES_DAYS as i32)
        .execute(state.database.write())
        .await?
        .rows_affected();
        if deleted > 0 {
            tracing::info!(deleted, "rux: pruned old messages");
        }
    }

    Ok(())
}

/// Orders whose worker is gone (a crash, a restart). Still pending a minute after they came in:
/// a new worker, unless the same person sent another order since (people repeat an order that
/// seems stuck; running both would do it twice) or it is ten minutes old. Still running after
/// fifteen minutes: given up.
async fn recover_orders(state: &crate::State) -> Result<(), anyhow::Error> {
    sqlx::query(
        "UPDATE rux_messages SET status = 'failed', meta = COALESCE(meta, '{}'::jsonb) || '{\"error\": \"stale\"}'::jsonb
        WHERE role = 'user' AND (
            (status = 'pending' AND created < NOW() - make_interval(mins => $1))
            OR (status = 'running' AND created < NOW() - make_interval(mins => $2))
        )",
    )
    .bind(ORDER_GIVE_UP_MINUTES as i32)
    .bind(ORDER_RUNNING_MINUTES as i32)
    .execute(state.database.write())
    .await?;

    let stuck: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, conversation FROM rux_messages
        WHERE role = 'user' AND status = 'pending' AND created < NOW() - make_interval(mins => $1)
        ORDER BY id LIMIT 5",
    )
    .bind(ORDER_RETRY_MINUTES as i32)
    .fetch_all(state.database.read())
    .await?;
    if stuck.is_empty() {
        return Ok(());
    }
    // the bot's ids come with the gateway's READY; without them the order waits
    let Some(me) = handler::me() else {
        return Ok(());
    };

    for (id, conversation) in stuck {
        if models::Message::superseded(&state.database, &conversation, id).await? {
            if models::Message::claim(
                &state.database,
                id,
                models::status::PENDING,
                models::status::FAILED,
            )
            .await?
            {
                models::Message::finish(
                    &state.database,
                    id,
                    models::status::FAILED,
                    serde_json::json!({ "error": "superseded" }),
                )
                .await?;
            }
            continue;
        }

        tracing::info!(
            order = id,
            "rux: an order lost its worker; starting another"
        );
        agent::spawn(state.clone(), me.clone(), id);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{discord::AppSettingsDiscord, rux::AppSettingsRux};

    fn config(enabled: bool, guild: &str) -> Config {
        let settings = AppSettingsRux::default();

        Config {
            guilds: config::guilds(&settings, guild, "Example"),
            settings,
            enabled,
            app_name: "Example".into(),
            portal: "Example".into(),
            link_url: "https://panel.example.com".into(),
            boost_two_role_id: String::new(),
            managed: None,
            managed_bot_token: None,
        }
    }

    #[test]
    fn everything_blocks_until_set_up() {
        let discord = AppSettingsDiscord::default();
        assert_eq!(
            blockers(&discord, &config(true, ""), false, false),
            vec![
                Blocker::DiscordOff,
                Blocker::BotOff,
                Blocker::NoToken,
                Blocker::NoServer,
                Blocker::NoAccounts,
                Blocker::NoAi
            ]
        );
    }

    #[test]
    fn nothing_blocks_once_set_up() {
        let guild = "100000000000000001";
        let mut discord = AppSettingsDiscord::default();
        discord.enabled = true;
        discord.guild_id = guild.into();
        discord.moderation_bot_enabled = true;
        discord.moderation_bot_token = Some("token".into());

        // with noprofit.space, admins are noprofit.space's
        assert!(blockers(&discord, &config(true, guild), true, true).is_empty());
        // without it, panel admins link Discord through the OAuth provider
        assert_eq!(
            blockers(&discord, &config(true, guild), false, true),
            vec![Blocker::NoAccounts]
        );
        discord.oauth_provider_uuid = Some(uuid::Uuid::nil());
        assert!(blockers(&discord, &config(true, guild), false, true).is_empty());
        // noprofit.space's switch
        assert_eq!(
            blockers(&discord, &config(false, guild), true, true),
            vec![Blocker::SwitchedOff]
        );
    }
}
