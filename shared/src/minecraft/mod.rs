//! plugged.host: Minecraft: Java Edition servers: which servers they are, the software and version
//! they run, and the content sources the Content page browses (docs/features/minecraft.md).

pub mod bedrock;
pub mod config;
pub mod content;
pub mod curseforge;
pub mod files;
pub mod jars;
pub mod log_check;
pub mod modpacks;
pub mod modrinth;
pub mod onboarding;
pub mod players;
pub mod spigot;
pub mod versions;
pub mod worlds;

use crate::models::{nest_egg::NestEgg, server::Server, server_variable::ServerVariable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The egg variables a server's Minecraft version is read from, the first that exists.
pub const VERSION_VARIABLES: &[&str] = &["MINECRAFT_VERSION", "MC_VERSION", "VANILLA_VERSION"];
/// What a version variable holds while the server follows the newest release.
pub const LATEST: &str = "latest";
/// How long a source's answers about projects and versions are kept.
const TTL: u64 = 3600;
/// How long a search is kept: a new release should show within minutes.
const SEARCH_TTL: u64 = 300;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Whether the egg runs Minecraft: Java Edition, decided like upstream's EULA prompt: by the
/// egg's `eula` feature.
pub fn is_minecraft(egg: &NestEgg) -> bool {
    egg.features
        .iter()
        .any(|feature| feature == crate::server_creation::EULA_FEATURE)
}

/// Refuses the Minecraft pages for a server whose egg is not Minecraft's.
pub fn require(server: &Server) -> Result<(), crate::response::ApiResponse> {
    if is_minecraft(&server.egg) {
        Ok(())
    } else {
        Err(
            crate::response::ApiResponse::error("this server does not run minecraft")
                .with_status(axum::http::StatusCode::NOT_FOUND),
        )
    }
}

/// The software families content is made for.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    /// Paper, Purpur, Spigot, Folia: Bukkit plugins in `plugins/`.
    Paper,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Loader {
    /// From a software choice's value (`purpur`, `fabric`, ...).
    pub fn from_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "paper" | "purpur" | "folia" | "spigot" | "bukkit" | "pufferfish" | "leaf" => {
                Some(Self::Paper)
            }
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            "forge" => Some(Self::Forge),
            "neoforge" => Some(Self::NeoForge),
            _ => None,
        }
    }

    /// From an egg's name (`Paper`, `Forge Enhanced`), for eggs without software choices.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();

        if name.contains("neoforge") {
            Some(Self::NeoForge)
        } else if name.contains("forge") {
            Some(Self::Forge)
        } else if name.contains("quilt") {
            Some(Self::Quilt)
        } else if name.contains("fabric") {
            Some(Self::Fabric)
        } else if ["paper", "purpur", "folia", "spigot", "bukkit", "pufferfish"]
            .iter()
            .any(|word| name.contains(word))
        {
            Some(Self::Paper)
        } else {
            None
        }
    }

    /// The loader categories Modrinth tags content with. Plugin servers load what was made for
    /// any Bukkit platform.
    pub fn modrinth_loaders(self) -> &'static [&'static str] {
        match self {
            Self::Paper => &["paper", "purpur", "folia", "spigot", "bukkit"],
            Self::Fabric => &["fabric"],
            Self::Quilt => &["quilt", "fabric"],
            Self::Forge => &["forge"],
            Self::NeoForge => &["neoforge"],
        }
    }

    /// The folder single files go to.
    pub fn directory(self) -> &'static str {
        match self {
            Self::Paper => "plugins",
            _ => "mods",
        }
    }

    /// What players browse for this software.
    pub fn kinds(self) -> &'static [Kind] {
        match self {
            Self::Paper => &[Kind::Plugin],
            _ => &[Kind::Mod, Kind::Modpack],
        }
    }
}

/// What a project is.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Plugin,
    Mod,
    Modpack,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Mod => "mod",
            Self::Modpack => "modpack",
        }
    }
}

/// Where content comes from.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Modrinth,
    CurseForge,
    SpigotMc,
}

/// A project as every source is shown: one shape, whatever the source calls its fields.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub source: Source,
    pub id: compact_str::CompactString,
    pub slug: compact_str::CompactString,
    pub title: compact_str::CompactString,
    pub description: compact_str::CompactString,
    pub author: compact_str::CompactString,
    pub downloads: u64,
    pub icon: Option<compact_str::CompactString>,
    pub categories: Vec<compact_str::CompactString>,
    /// The project's page on its source.
    pub url: compact_str::CompactString,
    pub kind: Kind,
    /// The panel can download it (a SpigotMC resource hosted elsewhere cannot be).
    pub installable: bool,
}

/// One build of a project.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct Version {
    pub id: compact_str::CompactString,
    pub project_id: compact_str::CompactString,
    pub name: compact_str::CompactString,
    pub number: compact_str::CompactString,
    /// `release`, `beta` or `alpha`.
    pub channel: compact_str::CompactString,
    pub date: compact_str::CompactString,
    pub game_versions: Vec<compact_str::CompactString>,
    pub loaders: Vec<compact_str::CompactString>,
    /// The file it installs, when the source names it with the version.
    pub file: Option<VersionFile>,
    /// Projects it needs (required dependencies, by id).
    pub dependencies: Vec<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct VersionFile {
    pub name: compact_str::CompactString,
    pub url: Option<compact_str::CompactString>,
    pub size: u64,
}

/// One page of a search.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default)]
pub struct SearchPage {
    pub hits: Vec<Project>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

/// What a Minecraft server runs.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct Software {
    /// `None` for software no content is made for (Vanilla, an egg the panel cannot read).
    pub loader: Option<Loader>,
    /// The software choice's value (`purpur`), when its game has choices.
    pub value: Option<compact_str::CompactString>,
    /// The version variable's value: a version, or `latest`.
    pub version: compact_str::CompactString,
    /// The variable the version is written to, when the egg has one.
    #[serde(skip)]
    pub version_variable: Option<compact_str::CompactString>,
}

/// Reads the software and version from the server's variables and its game's choices.
pub async fn software(state: &crate::State, server: &Server) -> Result<Software, anyhow::Error> {
    let settings = state.settings.get().await?;
    let game = settings.games.game(server.egg.uuid).cloned();
    drop(settings);

    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;
    let value_of = |name: &str| {
        variables
            .iter()
            .find(|variable| variable.variable.env_variable == name)
    };

    let value = game.as_ref().and_then(|game| {
        let variable = game.variable.as_deref()?;
        crate::server_software::current(game, value_of(variable).map(|v| v.value.as_str()))
            .map(compact_str::CompactString::from)
    });
    let loader = match &value {
        Some(value) => Loader::from_value(value),
        None => Loader::from_name(&server.egg.name),
    };

    let version = VERSION_VARIABLES.iter().find_map(|name| value_of(name));

    Ok(Software {
        loader,
        value,
        version: version
            .map(|variable| variable.value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(LATEST)
            .into(),
        version_variable: version.map(|variable| variable.variable.env_variable.clone()),
    })
}

impl Software {
    /// The Minecraft version, with `latest` read as the newest release the loader has.
    pub async fn game_version(&self, state: &crate::State) -> Option<compact_str::CompactString> {
        if !self.version.eq_ignore_ascii_case(LATEST) {
            return Some(self.version.clone());
        }

        versions::list(state, self.loader?)
            .await
            .ok()?
            .into_iter()
            .next()
    }
}

/// The User-Agent the panel names itself with to Modrinth, CurseForge, Spiget and the loaders'
/// APIs: the setting, or `<name>-panel/<release> (+<url>)`.
pub async fn user_agent(state: &crate::State) -> String {
    let Ok(settings) = state.settings.get().await else {
        return format!("panel/{}", crate::RELEASE);
    };

    if !settings.games.minecraft_user_agent.trim().is_empty() {
        return settings.games.minecraft_user_agent.trim().to_string();
    }

    default_user_agent(&settings.app.name, &settings.app.url)
}

fn default_user_agent(name: &str, url: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let name = name.trim_matches('-');
    let name = if name.is_empty() { "panel" } else { name };

    format!("{name}-panel/{} (+{url})", crate::RELEASE)
}

/// A cache key from a source's name and the request's parts.
fn cache_key(source: &str, what: &str, parts: &str) -> String {
    use sha2::Digest;

    let digest = sha2::Sha256::digest(parts.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }

    format!("minecraft::{source}::{what}::{hex}")
}

/// A GET that answers JSON, with the panel's User-Agent.
async fn get_json(
    state: &crate::State,
    url: &str,
    query: &[(&str, String)],
    headers: &[(&str, String)],
) -> Result<serde_json::Value, anyhow::Error> {
    let mut request = state
        .client
        .get(url)
        .query(query)
        .timeout(TIMEOUT)
        .header("User-Agent", user_agent(state).await)
        .header("Accept", "application/json");
    for (name, value) in headers {
        request = request.header(*name, value);
    }

    Ok(request.send().await?.error_for_status()?.json().await?)
}

/// A POST of JSON that answers JSON.
async fn post_json(
    state: &crate::State,
    url: &str,
    body: &serde_json::Value,
    headers: &[(&str, String)],
) -> Result<serde_json::Value, anyhow::Error> {
    let mut request = state
        .client
        .post(url)
        .json(body)
        .timeout(TIMEOUT)
        .header("User-Agent", user_agent(state).await)
        .header("Accept", "application/json");
    for (name, value) in headers {
        request = request.header(*name, value);
    }

    Ok(request.send().await?.error_for_status()?.json().await?)
}

/// A string field of a JSON object, or "".
fn text(value: &serde_json::Value, key: &str) -> compact_str::CompactString {
    match value.get(key) {
        Some(serde_json::Value::String(s)) => s.as_str().into(),
        Some(serde_json::Value::Number(n)) => n.to_string().into(),
        _ => compact_str::CompactString::default(),
    }
}

/// The strings of a JSON array field.
fn texts(value: &serde_json::Value, key: &str) -> Vec<compact_str::CompactString> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(Into::into))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a file name is a plain name, safe to hand to Wings as a download's target.
pub fn plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && !name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaders_come_from_values_and_names() {
        assert_eq!(Loader::from_value("Purpur"), Some(Loader::Paper));
        assert_eq!(Loader::from_value("neoforge"), Some(Loader::NeoForge));
        assert_eq!(Loader::from_value("vanilla"), None);
        assert_eq!(Loader::from_name("Forge Enhanced"), Some(Loader::Forge));
        assert_eq!(Loader::from_name("NeoForge"), Some(Loader::NeoForge));
        assert_eq!(Loader::from_name("Paper"), Some(Loader::Paper));
        assert_eq!(Loader::from_name("Vanilla Minecraft"), None);
        assert_eq!(Loader::Paper.directory(), "plugins");
        assert_eq!(Loader::Quilt.directory(), "mods");
    }

    #[test]
    fn the_user_agent_names_the_panel() {
        assert_eq!(
            default_user_agent("My Panel!", "https://panel.example.com"),
            format!(
                "My-Panel-panel/{} (+https://panel.example.com)",
                crate::RELEASE
            )
        );
        assert!(default_user_agent("", "https://x").starts_with("panel-panel/"));
    }

    #[test]
    fn only_plain_names_are_downloaded() {
        assert!(plain_file_name("EssentialsX-2.21.0.jar"));
        assert!(!plain_file_name("../x.jar"));
        assert!(!plain_file_name("a/b.jar"));
        assert!(!plain_file_name(".hidden.jar"));
        assert!(!plain_file_name(""));
    }
}
