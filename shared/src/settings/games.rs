// plugged.host: the games settings section: what players may create and how much (docs/features/create-server.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// One software choice of a game (Purpur, Fabric, ...): written to the game's variable, with its
/// own startup command or image when it needs one.
#[derive(ToSchema, Validate, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameVariant {
    /// The value the game's variable gets (`purpur`).
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub value: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub label: compact_str::CompactString,
    /// A startup command instead of the egg's.
    #[garde(inner(length(chars, max = 8192)))]
    #[serde(default)]
    pub startup: Option<compact_str::CompactString>,
    /// An image instead of the egg's first: one of the egg's image names, or an image reference.
    #[garde(inner(length(chars, max = 255)))]
    #[serde(default)]
    pub image: Option<compact_str::CompactString>,
}

/// A game players can pick: an egg, with the name and description they see.
#[derive(ToSchema, Validate, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Game {
    #[garde(skip)]
    pub egg_uuid: uuid::Uuid,
    /// The name players see; empty uses the egg's.
    #[garde(inner(length(chars, max = 64)))]
    #[serde(default)]
    pub name: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 255)))]
    #[serde(default)]
    pub description: Option<compact_str::CompactString>,
    /// The egg variable the software choice is written to (`MC_SOFTWARE`). Without it, or
    /// without variants, the server runs what the egg sets up.
    #[garde(inner(length(chars, max = 255)))]
    #[serde(default)]
    pub variable: Option<compact_str::CompactString>,
    /// The software choices, the first being the default.
    #[garde(length(max = 32), dive)]
    #[schema(max_items = 32)]
    #[serde(default)]
    pub variants: Vec<GameVariant>,
}

impl Game {
    /// The variant with this value, or the first.
    pub fn variant(&self, value: Option<&str>) -> Option<&GameVariant> {
        match value {
            Some(value) => self.variants.iter().find(|variant| variant.value == value),
            None => self.variants.first(),
        }
    }
}

/// What players may create. Off by default: until an admin turns it on, only admins create
/// servers.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsGames {
    /// Players may create servers themselves.
    pub create_enabled: bool,
    /// The games offered, in this order.
    pub catalog: Vec<Game>,
    /// What every new server may have.
    pub limit_allocations: i32,
    pub limit_databases: i32,
    pub limit_backups: i32,
    pub limit_schedules: i32,
    /// Owners may change their servers' CPU, memory and disk within their allowance.
    pub allow_resizing: bool,
    /// Players may delete their own servers.
    pub allow_deleting: bool,
    /// Players may move their servers to another location (docs/features/transfers.md).
    pub transfer_enabled: bool,
    /// Hours between two finished moves of one server; 0 is none.
    pub transfer_cooldown_hours: i32,
    /// Nodes a move tries before it gives up.
    pub transfer_attempts: i32,
    /// Moves off one node at the same time ("Lighten load").
    pub transfer_parallel: i32,
    /// Servers an account may have; 0 is no limit.
    pub max_servers: i32,
    /// What every account may use in total without noprofit.space (CPU in %, memory and disk in
    /// MiB). With noprofit.space connected, the account's plan decides instead.
    pub allowance_cpu: i32,
    pub allowance_memory: i64,
    pub allowance_disk: i64,

    /// CurseForge's API key for the Minecraft pages; without it CurseForge is not offered
    /// (docs/features/minecraft.md). Stored encrypted, shown as a placeholder.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub minecraft_curseforge_key: Option<compact_str::CompactString>,
    /// SpigotMC (through Spiget) as a source for plugin servers.
    pub minecraft_spigot: bool,
    /// The User-Agent sent to Modrinth, CurseForge, Spiget and the loaders' APIs; empty is
    /// `<name>-panel/<release> (+<url>)`.
    pub minecraft_user_agent: compact_str::CompactString,
    /// Uploads Minecraft servers' logs to mclo.gs and shows the problems it finds above the
    /// console.
    pub minecraft_log_check: bool,
}

impl Default for AppSettingsGames {
    fn default() -> Self {
        Self {
            create_enabled: false,
            catalog: Vec::new(),
            limit_allocations: 1,
            limit_databases: 0,
            limit_backups: 0,
            limit_schedules: 0,
            allow_resizing: false,
            allow_deleting: false,
            transfer_enabled: false,
            transfer_cooldown_hours: 24,
            transfer_attempts: 2,
            transfer_parallel: 2,
            max_servers: 0,
            allowance_cpu: 0,
            allowance_memory: 0,
            allowance_disk: 0,
            minecraft_curseforge_key: None,
            minecraft_spigot: true,
            minecraft_user_agent: compact_str::CompactString::default(),
            minecraft_log_check: true,
        }
    }
}

impl AppSettingsGames {
    pub fn game(&self, egg_uuid: uuid::Uuid) -> Option<&Game> {
        self.catalog.iter().find(|game| game.egg_uuid == egg_uuid)
    }

    /// The allowance of every account without noprofit.space.
    pub fn allowance(&self) -> crate::allowance::Resources {
        crate::allowance::Resources::new(
            self.allowance_cpu as i64,
            self.allowance_memory,
            self.allowance_disk,
        )
    }
}

/// The games tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadGames {
    #[garde(skip)]
    create_enabled: Option<bool>,
    #[garde(length(max = 100), dive)]
    #[schema(max_items = 100)]
    catalog: Option<Vec<Game>>,
    #[garde(inner(range(min = 1, max = 100)))]
    limit_allocations: Option<i32>,
    #[garde(inner(range(min = 0, max = 100)))]
    limit_databases: Option<i32>,
    #[garde(inner(range(min = 0, max = 100)))]
    limit_backups: Option<i32>,
    #[garde(inner(range(min = 0, max = 100)))]
    limit_schedules: Option<i32>,
    #[garde(skip)]
    allow_resizing: Option<bool>,
    #[garde(skip)]
    allow_deleting: Option<bool>,
    #[garde(skip)]
    transfer_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 720)))]
    transfer_cooldown_hours: Option<i32>,
    #[garde(inner(range(min = 1, max = 5)))]
    transfer_attempts: Option<i32>,
    #[garde(inner(range(min = 1, max = 10)))]
    transfer_parallel: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000)))]
    max_servers: Option<i32>,
    #[garde(inner(range(min = 0)))]
    allowance_cpu: Option<i32>,
    #[garde(inner(range(min = 0)))]
    allowance_memory: Option<i64>,
    #[garde(inner(range(min = 0)))]
    allowance_disk: Option<i64>,
    /// An empty string or the placeholder keeps the stored key, `null` removes it.
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    minecraft_curseforge_key: Option<Option<compact_str::CompactString>>,
    #[garde(skip)]
    minecraft_spigot: Option<bool>,
    #[garde(inner(length(chars, max = 255)))]
    minecraft_user_agent: Option<compact_str::CompactString>,
    #[garde(skip)]
    minecraft_log_check: Option<bool>,
}

impl PayloadGames {
    /// Writes the given fields; an error names the first catalog entry that is not usable.
    pub fn apply(self, settings: &mut AppSettingsGames) -> Result<(), String> {
        if let Some(catalog) = self.catalog {
            let mut seen = std::collections::HashSet::new();
            for game in &catalog {
                if !seen.insert(game.egg_uuid) {
                    return Err(format!("the egg {} is in the catalog twice", game.egg_uuid));
                }

                let mut values = std::collections::HashSet::new();
                for variant in &game.variants {
                    if !values.insert(variant.value.as_str()) {
                        return Err(format!(
                            "the variant {} is in the game {} twice",
                            variant.value, game.egg_uuid
                        ));
                    }
                }
                if !game.variants.is_empty()
                    && game.variable.as_deref().is_none_or(|v| v.trim().is_empty())
                {
                    return Err(format!(
                        "the game {} has variants but no variable to write them to",
                        game.egg_uuid
                    ));
                }
            }

            settings.catalog = catalog
                .into_iter()
                .map(|mut game| {
                    game.name = trimmed(game.name);
                    game.description = trimmed(game.description);
                    game.variable = trimmed(game.variable);
                    for variant in &mut game.variants {
                        variant.startup = trimmed(variant.startup.take());
                        variant.image = trimmed(variant.image.take());
                    }
                    game
                })
                .collect();
        }

        macro_rules! set {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        set!(
            create_enabled,
            limit_allocations,
            limit_databases,
            limit_backups,
            limit_schedules,
            allow_resizing,
            allow_deleting,
            transfer_enabled,
            transfer_cooldown_hours,
            transfer_attempts,
            transfer_parallel,
            max_servers,
            allowance_cpu,
            allowance_memory,
            allowance_disk,
            minecraft_spigot,
            minecraft_log_check
        );

        match self.minecraft_curseforge_key {
            Some(Some(key))
                if key.trim().is_empty() || key == crate::censor::CENSORED_PLACEHOLDER => {}
            Some(key) => settings.minecraft_curseforge_key = key.map(|key| key.trim().into()),
            None => {}
        }
        if let Some(user_agent) = self.minecraft_user_agent {
            if user_agent.chars().any(char::is_control) {
                return Err("the user agent must be one line".into());
            }
            settings.minecraft_user_agent = user_agent.trim().into();
        }

        Ok(())
    }
}

fn trimmed(value: Option<compact_str::CompactString>) -> Option<compact_str::CompactString> {
    value
        .map(|value| value.trim().into())
        .filter(|value: &compact_str::CompactString| !value.is_empty())
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsGames {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let serializer = match &self.minecraft_curseforge_key {
            Some(key) => {
                serializer
                    .write_raw_encrypted_setting("minecraft_curseforge_key", key.clone())
                    .await?
            }
            None => serializer.write_raw_setting("minecraft_curseforge_key", ""),
        };

        Ok(serializer
            .write_raw_setting(
                "minecraft_spigot",
                self.minecraft_spigot.to_compact_string(),
            )
            .write_raw_setting("minecraft_user_agent", &*self.minecraft_user_agent)
            .write_raw_setting(
                "minecraft_log_check",
                self.minecraft_log_check.to_compact_string(),
            )
            .write_raw_setting("create_enabled", self.create_enabled.to_compact_string())
            .write_raw_setting("catalog", serde_json::to_string(&self.catalog)?)
            .write_raw_setting(
                "limit_allocations",
                self.limit_allocations.to_compact_string(),
            )
            .write_raw_setting("limit_databases", self.limit_databases.to_compact_string())
            .write_raw_setting("limit_backups", self.limit_backups.to_compact_string())
            .write_raw_setting("limit_schedules", self.limit_schedules.to_compact_string())
            .write_raw_setting("allow_resizing", self.allow_resizing.to_compact_string())
            .write_raw_setting("allow_deleting", self.allow_deleting.to_compact_string())
            .write_raw_setting(
                "transfer_enabled",
                self.transfer_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "transfer_cooldown_hours",
                self.transfer_cooldown_hours.to_compact_string(),
            )
            .write_raw_setting(
                "transfer_attempts",
                self.transfer_attempts.to_compact_string(),
            )
            .write_raw_setting(
                "transfer_parallel",
                self.transfer_parallel.to_compact_string(),
            )
            .write_raw_setting("max_servers", self.max_servers.to_compact_string())
            .write_raw_setting("allowance_cpu", self.allowance_cpu.to_compact_string())
            .write_raw_setting(
                "allowance_memory",
                self.allowance_memory.to_compact_string(),
            )
            .write_raw_setting("allowance_disk", self.allowance_disk.to_compact_string()))
    }
}

pub struct AppSettingsGamesDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsGamesDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsGames::default();
        let minecraft_curseforge_key = match deserializer
            .take_raw_setting("minecraft_curseforge_key")
        {
            Some(key) if !key.is_empty() => Some(deserializer.database.decrypt_base64(key).await?),
            _ => None,
        };

        macro_rules! parsed {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }

        Ok(Box::new(AppSettingsGames {
            create_enabled: parsed!(create_enabled),
            catalog: deserializer
                .take_raw_setting("catalog")
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            limit_allocations: parsed!(limit_allocations),
            limit_databases: parsed!(limit_databases),
            limit_backups: parsed!(limit_backups),
            limit_schedules: parsed!(limit_schedules),
            allow_resizing: parsed!(allow_resizing),
            allow_deleting: parsed!(allow_deleting),
            transfer_enabled: parsed!(transfer_enabled),
            transfer_cooldown_hours: parsed!(transfer_cooldown_hours),
            transfer_attempts: parsed!(transfer_attempts),
            transfer_parallel: parsed!(transfer_parallel),
            max_servers: parsed!(max_servers),
            allowance_cpu: parsed!(allowance_cpu),
            allowance_memory: parsed!(allowance_memory),
            allowance_disk: parsed!(allowance_disk),
            minecraft_curseforge_key,
            minecraft_spigot: parsed!(minecraft_spigot),
            minecraft_user_agent: deserializer
                .take_raw_setting("minecraft_user_agent")
                .unwrap_or_default(),
            minecraft_log_check: parsed!(minecraft_log_check),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(variants: &[&str], variable: Option<&str>) -> Game {
        Game {
            egg_uuid: uuid::Uuid::nil(),
            name: Some("  Minecraft ".into()),
            description: Some("   ".into()),
            variable: variable.map(Into::into),
            variants: variants
                .iter()
                .map(|value| GameVariant {
                    value: (*value).into(),
                    label: (*value).into(),
                    startup: Some(" ".into()),
                    image: None,
                })
                .collect(),
        }
    }

    fn payload(catalog: Vec<Game>) -> PayloadGames {
        PayloadGames {
            create_enabled: None,
            catalog: Some(catalog),
            limit_allocations: None,
            limit_databases: None,
            limit_backups: None,
            limit_schedules: None,
            allow_resizing: None,
            allow_deleting: None,
            transfer_enabled: None,
            transfer_cooldown_hours: None,
            transfer_attempts: None,
            transfer_parallel: None,
            max_servers: Some(3),
            allowance_cpu: None,
            allowance_memory: None,
            allowance_disk: None,
            minecraft_curseforge_key: None,
            minecraft_spigot: None,
            minecraft_user_agent: None,
            minecraft_log_check: None,
        }
    }

    #[test]
    fn a_catalog_is_trimmed_and_the_rest_left_alone() {
        let mut settings = AppSettingsGames::default();
        payload(vec![game(&["purpur", "fabric"], Some("MC_SOFTWARE"))])
            .apply(&mut settings)
            .unwrap();

        let stored = &settings.catalog[0];
        assert_eq!(stored.name.as_deref(), Some("Minecraft"));
        assert_eq!(stored.description, None);
        assert_eq!(stored.variants[0].startup, None);
        assert_eq!(settings.max_servers, 3);
        assert!(!settings.create_enabled);
        assert_eq!(stored.variant(None).unwrap().value, "purpur");
        assert_eq!(stored.variant(Some("fabric")).unwrap().value, "fabric");
        assert!(stored.variant(Some("forge")).is_none());
    }

    #[test]
    fn duplicates_and_variants_without_a_variable_are_refused() {
        let mut settings = AppSettingsGames::default();

        assert!(
            payload(vec![game(&[], None), game(&[], None)])
                .apply(&mut settings)
                .is_err()
        );
        assert!(
            payload(vec![game(&["purpur", "purpur"], Some("MC_SOFTWARE"))])
                .apply(&mut settings)
                .is_err()
        );
        assert!(
            payload(vec![game(&["purpur"], None)])
                .apply(&mut settings)
                .is_err()
        );
        assert!(settings.catalog.is_empty());
    }
}
