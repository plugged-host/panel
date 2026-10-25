// plugged.host: the neo on Discord settings section (Admin › Settings › neo on Discord, docs/features/neo-discord.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How neo answers on Discord ([`crate::neo::discord`]). Its switch and bot token are the
/// assistant bot's in the Discord section; the limits, the providers and how long messages are
/// kept are neo's.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsNeoDiscord {
    /// Also answer messages on the server that start with the bot's name ("hey neo, …"). Needs
    /// the Message Content intent in Discord's Developer Portal; without it the bot cannot connect.
    pub name_calls: bool,
    /// Messages one Discord account may send the bot a minute; the ones over it are ignored.
    pub messages_per_minute: i32,
    /// Model calls one answer may take (it only looks up accounts, so a few are plenty).
    pub max_steps: i32,
    /// How old an earlier answer in the same channel may be to be remembered, in minutes.
    pub history_minutes: i32,
    /// How many earlier answers in the same channel are remembered (0: none).
    pub history_turns: i32,
}

impl Default for AppSettingsNeoDiscord {
    fn default() -> Self {
        Self {
            name_calls: false,
            messages_per_minute: 6,
            max_steps: 6,
            history_minutes: 60,
            history_turns: 6,
        }
    }
}

/// The neo on Discord tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadNeoDiscord {
    #[garde(skip)]
    name_calls: Option<bool>,
    #[garde(inner(range(min = 1, max = 60)))]
    messages_per_minute: Option<i32>,
    #[garde(inner(range(min = 2, max = 20)))]
    max_steps: Option<i32>,
    #[garde(inner(range(min = 1, max = 1440)))]
    history_minutes: Option<i32>,
    #[garde(inner(range(min = 0, max = 20)))]
    history_turns: Option<i32>,
}

impl PayloadNeoDiscord {
    pub fn apply(self, settings: &mut AppSettingsNeoDiscord) {
        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            name_calls,
            messages_per_minute,
            max_steps,
            history_minutes,
            history_turns
        );
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsNeoDiscord {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("name_calls", self.name_calls.to_compact_string())
            .write_raw_setting(
                "messages_per_minute",
                self.messages_per_minute.to_compact_string(),
            )
            .write_raw_setting("max_steps", self.max_steps.to_compact_string())
            .write_raw_setting("history_minutes", self.history_minutes.to_compact_string())
            .write_raw_setting("history_turns", self.history_turns.to_compact_string()))
    }
}

pub struct AppSettingsNeoDiscordDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsNeoDiscordDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsNeoDiscord::default();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsNeoDiscord {
            name_calls: read!(name_calls),
            messages_per_minute: read!(messages_per_minute),
            max_steps: read!(max_steps),
            history_minutes: read!(history_minutes),
            history_turns: read!(history_turns),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_changes_only_what_it_names() {
        let mut settings = AppSettingsNeoDiscord::default();
        PayloadNeoDiscord {
            name_calls: Some(true),
            messages_per_minute: None,
            max_steps: Some(4),
            history_minutes: None,
            history_turns: Some(0),
        }
        .apply(&mut settings);

        assert!(settings.name_calls);
        assert_eq!(settings.messages_per_minute, 6);
        assert_eq!(settings.max_steps, 4);
        assert_eq!(settings.history_minutes, 60);
        assert_eq!(settings.history_turns, 0);
    }

    #[test]
    fn ships_quiet() {
        let settings = AppSettingsNeoDiscord::default();
        assert!(!settings.name_calls, "name calls need a privileged intent");
    }
}
