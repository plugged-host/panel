//! plugged.host: which of rux's servers a DM acts on (`switch_server`). The tool finds the server
//! and answers `ToolResult::ok(..).with_guild(id)`; the agent checks the move (a DM only, a
//! server rux works on, a moderator only one they moderate), moves the rest of the order there
//! and remembers the pick for the DM for a day (`access::remember_dm_server`).

use super::{Tool, ToolError, ToolResult, required, schema};
use crate::rux::{
    access::{self, Level},
    config::{Config, Guild},
    context::Context,
    discord::Rest,
    resolver,
    say::Strings,
};
use serde_json::{Value, json};

/// The group's lines: `(key, English, German)`.
pub const STRINGS: Strings = &[];

/// The server `input` means among `guilds` (with their Discord names): its id, its host's name
/// or its Discord name (letters and digits only, any case), else the one whose name contains it.
pub fn find<'a>(guilds: &'a [(Guild, String)], input: &str) -> Option<&'a Guild> {
    let input = input.trim();
    let wanted = resolver::plain(input);
    let names = |guild: &Guild, discord: &str| {
        [resolver::plain(&guild.host_name), resolver::plain(discord)]
    };

    if let Some((guild, _)) = guilds.iter().find(|(guild, discord)| {
        guild.id == input || (!wanted.is_empty() && names(guild, discord).contains(&wanted))
    }) {
        return Some(guild);
    }
    if wanted.is_empty() {
        return None;
    }

    let partial: Vec<&Guild> = guilds
        .iter()
        .filter(|(guild, discord)| {
            names(guild, discord)
                .iter()
                .any(|name| name.contains(&wanted))
        })
        .map(|(guild, _)| guild)
        .collect();

    match partial.as_slice() {
        [only] => Some(only),
        _ => None,
    }
}

async fn named(rest: &Rest, config: &Config) -> Vec<(Guild, String)> {
    let mut guilds = Vec::new();
    for guild in &config.guilds {
        guilds.push((
            guild.clone(),
            access::server_name(rest, config, &guild.id).await,
        ));
    }

    guilds
}

pub struct SwitchServer;

#[async_trait::async_trait]
impl Tool for SwitchServer {
    fn name(&self) -> &'static str {
        "switch_server"
    }

    fn description(&self, context: &Context) -> String {
        format!(
            "Only in a DM: choose which of {}'s Discord servers this DM acts on from now on (members, channels, roles, rules, cases and giveaways are all per server). Call it before acting when they mean a server other than the one this DM acts on.",
            context.config.settings.name
        )
    }

    fn level(&self) -> Level {
        Level::Moderator
    }

    fn offered(&self, context: &Context) -> bool {
        context.direct && context.config.guilds.len() > 1
    }

    fn parameters(&self) -> Value {
        schema(
            json!({
                "server": { "type": "string", "description": "The server: its name or id, as the system prompt lists them." },
            }),
            &["server"],
        )
    }

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError> {
        if !context.direct {
            return Err(ToolError::invalid(
                "This order came from a server channel, so it acts on that server only. For another server, they ask there or in a DM.",
            ));
        }

        let input = required(arguments, "server")?;
        let guilds = named(&context.rest, &context.config).await;
        let Some(guild) = find(&guilds, &input) else {
            return Err(ToolError::Invalid(format!(
                "{} works on these servers: {}. Say which one is meant.",
                context.config.settings.name,
                context
                    .config
                    .guilds
                    .iter()
                    .map(Guild::label)
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        };
        let discord_name = guilds
            .iter()
            .find(|(candidate, _)| candidate.id == guild.id)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();

        // the agent checks the move (moderators only to a server they moderate) and remembers it
        Ok(ToolResult::json(&json!({
            "server": guild.label(),
            "discord_name": discord_name,
            "note": "From now on this DM acts on this server.",
        }))
        .with_guild(guild.id.clone()))
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(SwitchServer)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guild(id: &str, host: &str) -> Guild {
        Guild {
            id: id.into(),
            primary: false,
            host_name: host.into(),
            about: String::new(),
            giveaway_channel_id: None,
            mod_log_channel_id: None,
            trusted_domains: Vec::new(),
            locations: false,
        }
    }

    #[test]
    fn servers_are_found_by_id_or_name() {
        let guilds = vec![
            (
                guild("100000000000000001", "Example"),
                "Example Community".to_string(),
            ),
            (
                guild("100000000000000002", "Example Cloud"),
                "Cloud Lounge".to_string(),
            ),
            (
                guild("100000000000000003", "Other Host"),
                "Other".to_string(),
            ),
        ];
        let id = |input: &str| find(&guilds, input).map(|guild| guild.id.as_str());

        assert_eq!(id("100000000000000002"), Some("100000000000000002"));
        assert_eq!(id(" example "), Some("100000000000000001"));
        assert_eq!(id("example-cloud"), Some("100000000000000002"));
        assert_eq!(id("cloud lounge"), Some("100000000000000002"));
        assert_eq!(id("lounge"), Some("100000000000000002"));
        // two servers contain it: say which
        assert_eq!(id("exam"), None);
        assert_eq!(id(""), None);
        assert_eq!(id("nowhere"), None);
    }
}
