//! plugged.host: rux on the Discord gateway: the moderation bot's handler
//! ([`crate::discord::gateway::Handler`] for [`Bot::Moderation`]), plugged in by [`register`].
//!
//! The gateway hands it the events in order, in one worker, so it only does quick things itself:
//! a message that is an order is stored ([`super::inbox`]) and carried out by its own task
//! ([`super::agent::spawn`]); every message of a server rux works on then goes to the watch
//! ([`super::watch::on_message`], which queues or spawns its checks); a giveaway's button goes to
//! the giveaways, which answer it at once. Events of servers rux is in but does not work on are
//! dropped.

use super::{agent, context::Me, inbox, tools, watch};
use crate::discord::gateway::{self, Bot, Handler, INTENT_GUILDS, Session};
use serde_json::Value;
use std::sync::{Arc, LazyLock};

pub const INTENT_GUILD_MESSAGES: u64 = 1 << 9;
pub const INTENT_DIRECT_MESSAGES: u64 = 1 << 12;
/// Privileged: Message Content Intent in the Developer Portal. "hey rux", standing rules,
/// watched channels and the spam checks need it; DMs and @mentions work without it.
pub const INTENT_MESSAGE_CONTENT: u64 = 1 << 15;
/// Servers, their messages, DMs and the text of messages (37377).
pub const INTENTS: u64 =
    INTENT_GUILDS | INTENT_GUILD_MESSAGES | INTENT_DIRECT_MESSAGES | INTENT_MESSAGE_CONTENT;

/// The bot's own ids, from the gateway's last READY.
static ME: LazyLock<parking_lot::RwLock<Option<Me>>> = LazyLock::new(Default::default);

/// The bot's own ids, once the gateway is ready (for work started outside an event, like the
/// tick's recovered orders).
pub fn me() -> Option<Me> {
    ME.read().clone()
}

struct Rux;

#[async_trait::async_trait]
impl Handler for Rux {
    fn intents(&self) -> u64 {
        INTENTS
    }

    async fn event(
        &self,
        state: &crate::State,
        session: &Session,
        name: &str,
        data: &Value,
    ) -> Result<(), anyhow::Error> {
        let me = Me {
            user_id: session.user_id.clone(),
            application_id: session.application_id.clone(),
        };
        if name == "READY" {
            *ME.write() = Some(me);
            return Ok(());
        }

        match name {
            "MESSAGE_CREATE" => on_message(state, &me, data).await,
            "INTERACTION_CREATE" => on_interaction(state, &me, data).await,
            _ => Ok(()),
        }
    }
}

async fn on_message(state: &crate::State, me: &Me, message: &Value) -> Result<(), anyhow::Error> {
    if me.user_id.is_empty() {
        return Ok(());
    }
    let Some((config, rest)) = super::running(state).await? else {
        return Ok(());
    };

    match inbox::take(state, me, &config, message).await {
        Ok(Some(order)) => {
            tracing::info!(
                order,
                from = message["author"]["id"].as_str().unwrap_or_default(),
                "rux: order"
            );
            agent::spawn(state.clone(), me.clone(), order);
        }
        Ok(None) => {}
        // the watch still gets the message
        Err(err) => tracing::warn!("rux: could not take an order: {err:#}"),
    }

    // an order is still a member's message: staff are left alone by the checks themselves
    if config.serves(message["guild_id"].as_str()) {
        watch::on_message(state, &config, &rest, me, message).await?;
    }

    Ok(())
}

async fn on_interaction(
    state: &crate::State,
    me: &Me,
    interaction: &Value,
) -> Result<(), anyhow::Error> {
    let custom_id = interaction["data"]["custom_id"]
        .as_str()
        .unwrap_or_default();
    if !custom_id.starts_with(tools::giveaways::BUTTON_PREFIX) {
        return Ok(());
    }
    let Some((config, rest)) = super::running(state).await? else {
        return Ok(());
    };
    if let Some(guild_id) = interaction["guild_id"].as_str()
        && !config.serves(Some(guild_id))
    {
        return Ok(());
    }

    tools::giveaways::on_interaction(state, &config, &rest, me, interaction).await?;

    Ok(())
}

/// Plugs rux in as the moderation bot; it connects once Admin › Settings › Discord turns the
/// moderation bot on with its token. Called once at startup.
pub fn register() {
    gateway::register(Bot::Moderation, Arc::new(Rux));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_cover_messages_and_their_text() {
        assert_eq!(INTENTS, 37377);
        assert_eq!(Rux.intents(), INTENTS);
    }
}
