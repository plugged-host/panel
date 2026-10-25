//! plugged.host: neo on Discord, account support on the Discord server (docs/features/neo-discord.md).
//!
//! The assistant bot of Admin › Settings › Discord (its own token and switch) answers DMs, and on
//! the Discord server the messages that @mention it, reply to it or, with name calls on, start
//! with its name ([`inbox`]). Anyone may ask, linked or not: a Discord account linked to an
//! account uses that account's neo limits, one nobody linked a share of them ([`who`]).
//!
//! A message is stored (`neo_discord_messages`, [`messages`]) and answered by a worker of its
//! conversation (one person in one channel or DM), one answer at a time; messages sent while it
//! thinks are answered together next. The answer runs neo's usual loop ([`agent`]): a model turn,
//! the read-only account lookups it asks for (in DMs, for a linked panel account only), their
//! results back, until it answers in text, posted as a reply split at Discord's 2000 characters.
//! It never changes anything, and never pings anyone.

mod agent;
mod inbox;
mod messages;
mod rest;
mod who;

use crate::{
    discord::gateway::{self, Bot, Handler, INTENT_GUILDS, Session},
    settings::{neo::AppSettingsNeo, neo_discord::AppSettingsNeoDiscord},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const INTENT_GUILD_MESSAGES: u64 = 1 << 9;
const INTENT_DIRECT_MESSAGES: u64 = 1 << 12;
/// Privileged: Message Content Intent in the Developer Portal. Only name calls need it: Discord
/// sends the text of DMs, mentions and replies that ping the bot without it.
const INTENT_MESSAGE_CONTENT: u64 = 1 << 15;

/// The longest one answer may take, all model turns and lookups included.
const ANSWER_DEADLINE: Duration = Duration::from_secs(300);
/// How long the conversation's lock is held at most (an answer, and some room).
const LOCK_SECONDS: u64 = ANSWER_DEADLINE.as_secs() + 30;
/// How long a new message's worker waits for the one already answering in its conversation. That
/// one answers it next anyway, so giving up is harmless.
const LOCK_WAIT_MS: u64 = 150_000;
/// A message still waiting after this long has lost its worker: [`recover`] starts another.
const STUCK_SECONDS: i32 = 60;

/// What an answer works from, read when it starts.
pub struct Setup {
    pub neo: AppSettingsNeo,
    pub bot: AppSettingsNeoDiscord,
    pub discord: crate::discord::Context,
    /// noprofit.space's connection while it runs neo (it decides who is linked and meters turns).
    pub gateway: Option<crate::noprofit::Config>,
    /// neo is on (its master switch) and can think: noprofit.space runs it, or a provider is set
    /// up.
    pub configured: bool,
    /// The assistant bot's token, while the bot is on.
    pub token: Option<String>,
    pub app_name: String,
    pub app_url: String,
}

impl Setup {
    pub async fn load(state: &crate::State) -> Result<Self, anyhow::Error> {
        let discord = crate::discord::Context::load(state).await?;
        let settings = state.settings.get().await?;

        Ok(Self {
            neo: settings.neo.clone(),
            bot: settings.neo_discord.clone(),
            gateway: super::on_noprofit(&settings),
            configured: settings.neo.enabled && super::configured(&settings),
            token: assistant_token(&settings.discord).map(str::to_string),
            app_name: settings.app.name.to_string(),
            app_url: settings.app.url.trim_end_matches('/').to_string(),
            discord,
        })
    }

    /// Where accounts live, as people know it: noprofit.space while it is connected, else the
    /// panel's name.
    pub fn portal(&self) -> &str {
        if self.discord.accounts_url.is_some() {
            "noprofit.space"
        } else {
            &self.app_name
        }
    }

    /// Where someone links their Discord account: noprofit.space's settings while it is
    /// connected, else the panel's linked accounts page.
    pub fn link_url(&self) -> String {
        match &self.discord.accounts_url {
            Some(accounts_url) => format!("{}/settings", accounts_url.trim_end_matches('/')),
            None => format!("{}/account/oauth-links", self.app_url),
        }
    }
}

/// The assistant bot's token while it runs: the Discord section on, the bot on, its token set.
fn assistant_token(discord: &crate::settings::discord::AppSettingsDiscord) -> Option<&str> {
    discord
        .assistant_bot_token
        .as_deref()
        .filter(|token| discord.active() && discord.assistant_bot_enabled && !token.is_empty())
}

/// Name calls are on: read at startup, with every event and every minute ([`recover`]). The
/// gateway asks for the intents when a session starts, so a change applies to the next session.
static NAME_CALLS: AtomicBool = AtomicBool::new(false);

/// The assistant bot's handler.
#[derive(Default)]
pub struct Assistant {
    /// The pattern of a call by the bot's names, from READY.
    name_call: parking_lot::RwLock<Option<regex::Regex>>,
}

/// The intents the assistant connects with.
fn intents(name_calls: bool) -> u64 {
    let intents = INTENT_GUILDS | INTENT_GUILD_MESSAGES | INTENT_DIRECT_MESSAGES;

    if name_calls {
        intents | INTENT_MESSAGE_CONTENT
    } else {
        intents
    }
}

#[async_trait::async_trait]
impl Handler for Assistant {
    fn intents(&self) -> u64 {
        intents(NAME_CALLS.load(Ordering::Relaxed))
    }

    async fn event(
        &self,
        state: &crate::State,
        session: &Session,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let bot = state.settings.get().await?.neo_discord.clone();
        NAME_CALLS.store(bot.name_calls, Ordering::Relaxed);

        match name {
            "READY" => self.ready(state, session, &bot, data).await,
            "MESSAGE_CREATE" => self.message(state, session, &bot, data).await,
            _ => Ok(()),
        }
    }
}

impl Assistant {
    async fn ready(
        &self,
        state: &crate::State,
        session: &Session,
        bot: &AppSettingsNeoDiscord,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let user = &data["user"];
        let names: Vec<&str> = [&user["username"], &user["global_name"]]
            .into_iter()
            .filter_map(|name| name.as_str())
            .collect();
        *self.name_call.write() = inbox::name_call(&names);

        if bot.name_calls && session.intents & INTENT_MESSAGE_CONTENT == 0 {
            tracing::info!(
                "neo on Discord: name calls start when the assistant bot's session starts over (a restart, or the bot off and on)"
            );
        }

        // answers that waited through a restart or a reconnect
        for conversation in messages::waiting_conversations(&state.database).await? {
            spawn_worker(state.clone(), conversation);
        }

        Ok(())
    }

    async fn message(
        &self,
        state: &crate::State,
        session: &Session,
        bot: &AppSettingsNeoDiscord,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let name_call = self.name_call.read().clone();
        let me = inbox::Me {
            id: &session.user_id,
            guild_id: &session.guild_id,
            name_call: name_call.as_ref().filter(|_| bot.name_calls),
        };
        let Some(incoming) = inbox::accept(data, &me) else {
            return Ok(());
        };

        // the ones over the limit are ignored (Discord itself slows a flood down)
        let per_minute = bot.messages_per_minute.clamp(1, 60) as u64;
        if state
            .cache
            .ratelimit("neo::discord", per_minute + 1, 60, &incoming.author_id)
            .await
            .is_err()
        {
            tracing::debug!(author = %incoming.author_id, "neo on Discord: over the message limit");
            return Ok(());
        }

        messages::insert_incoming(&state.database, &incoming).await?;
        spawn_worker(state.clone(), incoming.conversation());

        Ok(())
    }
}

/// Answers what waits in the conversation, in the background (events run one after another, and
/// an answer takes a while).
fn spawn_worker(state: crate::State, conversation: String) {
    tokio::spawn(async move {
        if let Err(err) = work(&state, &conversation).await {
            tracing::warn!(%conversation, "neo on Discord could not answer: {err:#}");
        }
    });
}

/// One answer at a time per conversation, until nothing waits.
async fn work(state: &crate::State, conversation: &str) -> Result<(), anyhow::Error> {
    loop {
        let Ok(lock) = state
            .cache
            .lock(
                format!("neo::discord::{conversation}"),
                Some(LOCK_SECONDS),
                Some(LOCK_WAIT_MS),
            )
            .await
        else {
            // the worker that holds it answers this message next
            return Ok(());
        };

        // read before anything is claimed, so a failure leaves the messages waiting
        let setup = Setup::load(state).await?;
        if setup.token.is_none() {
            // turned off meanwhile: nobody to answer as (given up after a while)
            return Ok(());
        }
        let Some(rows) = messages::claim(&state.database, conversation).await? else {
            return Ok(());
        };
        agent::answer(state, &setup, &rows).await;
        drop(lock);
    }
}

/// Plugs the assistant bot in; it connects once Admin › Settings › Discord turns it on and has its
/// token. Called once at startup, before the gateway's first session: the intents it asks for
/// depend on the name calls setting.
pub async fn register(state: &crate::State) {
    match state.settings.get().await {
        Ok(settings) => NAME_CALLS.store(settings.neo_discord.name_calls, Ordering::Relaxed),
        Err(err) => tracing::warn!("neo on Discord could not read its settings: {err:#}"),
    }

    gateway::register(Bot::Assistant, Arc::new(Assistant::default()));
}

/// Gives up messages that waited too long for an answer, and starts a worker for those still
/// waiting whose worker is gone (it failed, or the panel restarted). For a background task every
/// minute (on the primary instance, like the gateway).
pub async fn recover(state: &crate::State) -> Result<(), anyhow::Error> {
    let given_up = messages::give_up(&state.database).await?;
    if given_up > 0 {
        tracing::info!(
            given_up,
            "neo on Discord gave up messages that waited too long"
        );
    }

    let on = {
        let settings = state.settings.get().await?;
        NAME_CALLS.store(settings.neo_discord.name_calls, Ordering::Relaxed);
        assistant_token(&settings.discord).is_some()
    };
    if on {
        for conversation in messages::stuck_conversations(&state.database, STUCK_SECONDS).await? {
            spawn_worker(state.clone(), conversation);
        }
    }

    Ok(())
}

/// Deletes neo on Discord's messages older than neo's retention days. For a daily background task.
pub async fn prune(state: &crate::State) -> Result<(), anyhow::Error> {
    let retention_days = state.settings.get().await?.neo.retention_days;
    let deleted = messages::prune(&state.database, retention_days).await?;
    if deleted > 0 {
        tracing::info!(deleted, "pruned neo on Discord's old messages");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_content_only_for_name_calls() {
        assert_eq!(intents(false) & INTENT_MESSAGE_CONTENT, 0);
        assert_ne!(intents(true) & INTENT_MESSAGE_CONTENT, 0);
        assert_eq!(
            intents(false),
            INTENT_GUILDS | INTENT_GUILD_MESSAGES | INTENT_DIRECT_MESSAGES
        );
        assert!(LOCK_SECONDS > ANSWER_DEADLINE.as_secs());
    }

    #[test]
    fn ships_without_name_calls() {
        let name_calls = AppSettingsNeoDiscord::default().name_calls;
        assert_eq!(intents(name_calls) & INTENT_MESSAGE_CONTENT, 0);
    }

    #[test]
    fn answers_only_while_the_bot_runs() {
        let mut discord = crate::settings::discord::AppSettingsDiscord::default();
        assert_eq!(assistant_token(&discord), None, "off by default");

        discord.enabled = true;
        discord.guild_id = "200000000000000002".into();
        discord.assistant_bot_token = Some("token".into());
        assert_eq!(assistant_token(&discord), None, "the bot is off");

        discord.assistant_bot_enabled = true;
        assert_eq!(assistant_token(&discord), Some("token"));

        discord.enabled = false;
        assert_eq!(
            assistant_token(&discord),
            None,
            "the Discord section is off"
        );

        discord.enabled = true;
        discord.assistant_bot_token = Some("".into());
        assert_eq!(assistant_token(&discord), None, "no token");
    }
}
