//! plugged.host: who ordered what, and where: handed to every tool.
//!
//! The actor is an admin (a noprofit.space admin, or a panel admin without noprofit.space), a
//! member of a rux moderator role of the server (fewer tools), or rux itself acting on a
//! standing rule or a watched channel (`evidence` is then the message that was caught, kept with
//! the case). `guild_id` is the one server everything in this order acts on: the server the
//! order came from, or for a DM the server picked for it (the agent decides before the model is
//! ever called; a moderator's level is that server's).

use super::{access::Level, config::Config, discord::Rest};
use std::sync::Arc;

/// The bot's own ids, from the gateway's READY.
#[derive(Clone, Debug, Default)]
pub struct Me {
    pub user_id: String,
    pub application_id: String,
}

#[derive(Clone)]
pub struct Context {
    pub state: crate::State,
    pub config: Arc<Config>,
    pub rest: Rest,
    pub me: Me,
    pub guild_id: String,
    pub actor_discord_id: String,
    pub actor_name: String,
    /// The actor's noprofit.space account, while noprofit.space is connected.
    pub actor_account_id: Option<i64>,
    /// The actor's panel user, when they have one.
    pub actor_user_uuid: Option<uuid::Uuid>,
    /// Where the order was written (a DM channel for a DM).
    pub channel_id: String,
    pub direct: bool,
    pub source_message_id: Option<String>,
    /// The order (`rux_messages.id`), when this is one.
    pub order_id: Option<i64>,
    pub level: Level,
    /// The message rux acted on by itself.
    pub evidence: Option<String>,
}

impl Context {
    /// rux acting by itself on a server (a standing rule, a watched channel, the spam checks):
    /// the actor is the bot, with an admin's tools, and `evidence` is the message it caught.
    pub fn itself(
        state: &crate::State,
        config: Arc<Config>,
        rest: Rest,
        me: Me,
        guild_id: &str,
        channel_id: &str,
        evidence: Option<String>,
    ) -> Self {
        Self {
            state: state.clone(),
            actor_discord_id: me.user_id.clone(),
            actor_name: config.settings.name.to_string(),
            config,
            rest,
            me,
            guild_id: guild_id.to_string(),
            actor_account_id: None,
            actor_user_uuid: None,
            channel_id: channel_id.to_string(),
            direct: false,
            source_message_id: None,
            order_id: None,
            level: Level::Admin,
            evidence,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.level == Level::Admin
    }

    /// The server this order acts on.
    pub fn guild(&self) -> Option<&super::config::Guild> {
        self.config.guild(&self.guild_id)
    }

    /// The same order, acting on another server from here on (a DM's `switch_server`).
    pub fn with_guild(&self, guild_id: &str) -> Self {
        Self {
            guild_id: guild_id.to_string(),
            ..self.clone()
        }
    }

    /// What Discord's audit log shows for an action rux took on this order.
    pub fn audit_reason(&self, reason: Option<&str>) -> String {
        audit_reason(&self.config.settings.name, &self.actor_name, reason)
    }

    /// A line rux posts, in its language.
    pub fn say(&self, key: &str, replace: &[(&str, &str)]) -> String {
        self.config.say(key, replace)
    }
}

/// "rux for Ann: spam"
pub fn audit_reason(bot: &str, actor: &str, reason: Option<&str>) -> String {
    match reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("{bot} for {actor}: {reason}"),
        None => format!("{bot} for {actor}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_reasons_name_who_asked() {
        assert_eq!(
            audit_reason("rux", "Ann", Some(" spam ")),
            "rux for Ann: spam"
        );
        assert_eq!(audit_reason("rux", "Ann", Some("")), "rux for Ann");
        assert_eq!(audit_reason("rux", "Ann", None), "rux for Ann");
    }
}
