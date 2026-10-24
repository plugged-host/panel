//! plugged.host: switching a server between its game's software choices and, on Minecraft, its
//! Minecraft version, then reinstalling it (docs/features/server-settings.md).

use crate::{
    models::{
        UpdatableModel,
        server::{Server, ServerInstallOptions, UpdateServerOptions},
        server_variable::ServerVariable,
    },
    server_creation::ApiGameVariant,
    settings::games::Game,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(ToSchema, Serialize)]
pub struct ApiSoftware {
    pub variants: Vec<ApiGameVariant>,
    /// The choice the server runs now: its variable's value, else the first.
    pub current: Option<compact_str::CompactString>,
    /// The Minecraft version the server runs (`latest` or a version), when its egg has a version
    /// variable.
    pub version: Option<compact_str::CompactString>,
}

#[derive(Debug)]
pub enum SoftwareError {
    /// The server's egg is not a game with software choices.
    NotOffered,
    UnknownVariant,
    /// The server is not Minecraft, or its egg has no version variable.
    VersionNotOffered,
    UnknownVersion,
    Other(anyhow::Error),
}

impl SoftwareError {
    pub fn message(&self) -> String {
        match self {
            Self::NotOffered => "this server has no software to choose from".into(),
            Self::UnknownVariant => "that software is not offered for this server".into(),
            Self::VersionNotOffered => "this server's minecraft version cannot be chosen".into(),
            Self::UnknownVersion => {
                "that minecraft version is not offered for this software".into()
            }
            Self::Other(err) => err.to_string(),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotOffered
            | Self::UnknownVariant
            | Self::VersionNotOffered
            | Self::UnknownVersion => 400,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for SoftwareError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for SoftwareError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

/// The game of the server's egg, when it has software choices and a variable for them.
async fn game(state: &crate::State, server: &Server) -> Result<Option<Game>, anyhow::Error> {
    let settings = state.settings.get().await?;

    Ok(settings
        .games
        .game(server.egg.uuid)
        .filter(|game| !game.variants.is_empty() && game.variable.is_some())
        .cloned())
}

/// The choice whose value the variable holds, else the first (what the egg installs by default).
pub fn current<'a>(game: &'a Game, value: Option<&str>) -> Option<&'a str> {
    value
        .and_then(|value| game.variants.iter().find(|variant| variant.value == value))
        .or_else(|| game.variants.first())
        .map(|variant| variant.value.as_str())
}

/// The Minecraft version the server runs, when it is Minecraft and its egg has a version variable.
async fn minecraft_version(
    state: &crate::State,
    server: &Server,
) -> Result<Option<crate::minecraft::Software>, anyhow::Error> {
    if !crate::minecraft::is_minecraft(&server.egg) {
        return Ok(None);
    }

    let software = crate::minecraft::software(state, server).await?;
    Ok(software.version_variable.is_some().then_some(software))
}

/// What the server can switch to; `None` when its game offers no choice and no version.
pub async fn options(
    state: &crate::State,
    server: &Server,
) -> Result<Option<ApiSoftware>, anyhow::Error> {
    let game = game(state, server).await?;
    let version = minecraft_version(state, server)
        .await?
        .map(|software| software.version);
    if game.is_none() && version.is_none() {
        return Ok(None);
    }

    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;
    let current_value = game.as_ref().and_then(|game| {
        let value = variables
            .iter()
            .find(|variable| Some(&variable.variable.env_variable) == game.variable.as_ref())
            .map(|variable| variable.value.as_str());

        current(game, value).map(Into::into)
    });

    Ok(Some(ApiSoftware {
        current: current_value,
        variants: game
            .map(|game| {
                game.variants
                    .iter()
                    .map(|variant| ApiGameVariant {
                        value: variant.value.clone(),
                        label: variant.label.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        version,
    }))
}

/// The Minecraft versions a software choice offers, newest first (`None`: the server's own).
pub async fn versions(
    state: &crate::State,
    server: &Server,
    variant: Option<&str>,
) -> Result<Vec<compact_str::CompactString>, SoftwareError> {
    let Some(software) = minecraft_version(state, server).await? else {
        return Err(SoftwareError::VersionNotOffered);
    };
    let loader = match variant {
        Some(value) => {
            let game = game(state, server)
                .await?
                .ok_or(SoftwareError::NotOffered)?;
            if !game.variants.iter().any(|variant| variant.value == value) {
                return Err(SoftwareError::UnknownVariant);
            }
            crate::minecraft::Loader::from_value(value)
        }
        None => software.loader,
    };

    Ok(crate::minecraft::versions::for_software(state, loader).await?)
}

/// A version as a version variable may hold it.
fn plain_version(version: &str) -> bool {
    (1..=32).contains(&version.len())
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}

/// Writes the choice to the game's variable and takes its startup command and image, writes the
/// Minecraft version, then reinstalls the server (its files stay; the install puts the new
/// software in place).
pub async fn switch(
    state: &crate::State,
    server: &mut Server,
    value: Option<&str>,
    version: Option<&str>,
) -> Result<(), SoftwareError> {
    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;

    let mut variant = None;
    if let Some(value) = value {
        let Some(game) = game(state, server).await? else {
            return Err(SoftwareError::NotOffered);
        };
        let Some(chosen) = game.variants.iter().find(|variant| variant.value == value) else {
            return Err(SoftwareError::UnknownVariant);
        };
        let Some(variable) = variables
            .iter()
            .find(|variable| Some(&variable.variable.env_variable) == game.variable.as_ref())
        else {
            return Err(SoftwareError::NotOffered);
        };

        variant = Some((chosen.clone(), variable.variable.uuid));
    }

    let mut version_write = None;
    if let Some(version) = version.map(str::trim) {
        let Some(software) = minecraft_version(state, server).await? else {
            return Err(SoftwareError::VersionNotOffered);
        };
        if !version.eq_ignore_ascii_case(crate::minecraft::LATEST) {
            if !plain_version(version) {
                return Err(SoftwareError::UnknownVersion);
            }
            let loader = match &variant {
                Some((chosen, _)) => crate::minecraft::Loader::from_value(&chosen.value),
                None => software.loader,
            };
            // a source that does not answer leaves the check to the install
            if let Ok(offered) = crate::minecraft::versions::for_software(state, loader).await
                && !offered.is_empty()
                && !offered.iter().any(|offered| offered == version)
            {
                return Err(SoftwareError::UnknownVersion);
            }
        }

        let Some(variable) = variables.iter().find(|variable| {
            Some(&variable.variable.env_variable) == software.version_variable.as_ref()
        }) else {
            return Err(SoftwareError::VersionNotOffered);
        };
        let version = if version.eq_ignore_ascii_case(crate::minecraft::LATEST) {
            crate::minecraft::LATEST
        } else {
            version
        };
        version_write = Some((variable.variable.uuid, version.to_string()));
    }

    if let Some((chosen, variable_uuid)) = &variant {
        ServerVariable::create(&state.database, server.uuid, *variable_uuid, &chosen.value).await?;
    }
    if let Some((variable_uuid, version)) = &version_write {
        ServerVariable::create(&state.database, server.uuid, *variable_uuid, version).await?;

        // a build pinned for the old version does not exist for the new one
        if let Some(build) = variables
            .iter()
            .find(|variable| BUILD_VARIABLES.contains(&variable.variable.env_variable.as_str()))
        {
            ServerVariable::create(
                &state.database,
                server.uuid,
                build.variable.uuid,
                crate::minecraft::LATEST,
            )
            .await?;
        }
    }

    if let Some((chosen, _)) = &variant
        && let Some((startup, image)) = crate::server_creation::runtime(&server.egg, Some(chosen))
    {
        server
            .update(
                state,
                UpdateServerOptions {
                    startup: Some(startup),
                    image: Some(image),
                    ..Default::default()
                },
            )
            .await?;
    }

    server
        .install_with_options(
            state,
            ServerInstallOptions {
                truncate_directory: false,
                installation_script: None,
                start_on_completion: false,
            },
        )
        .await?;

    Ok(())
}

/// How long a reinstall may take before waiting for it gives up.
pub const INSTALL_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Waits until the server's install is over: `true` when it finished, `false` when it failed or
/// outlasted `timeout`. Polls the server's status every 5 seconds.
pub async fn wait_for_install(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    timeout: std::time::Duration,
) -> Result<bool, crate::database::DatabaseError> {
    use crate::models::{ByUuid, server::ServerStatus};

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match Server::by_uuid(&state.database, server_uuid).await?.status {
            Some(ServerStatus::Installing) if tokio::time::Instant::now() < deadline => {}
            Some(ServerStatus::Installing) | Some(ServerStatus::InstallFailed) => return Ok(false),
            _ => return Ok(true),
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// The variables a Minecraft egg pins a software build with, set back to `latest` with the
/// version.
const BUILD_VARIABLES: &[&str] = &["BUILD_NUMBER"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::games::GameVariant;

    #[test]
    fn the_current_choice_falls_back_to_the_first() {
        let game = Game {
            egg_uuid: uuid::Uuid::nil(),
            name: None,
            description: None,
            icon: None,
            variable: Some("SOFTWARE".into()),
            variants: ["paper", "fabric"]
                .into_iter()
                .map(|value| GameVariant {
                    value: value.into(),
                    label: value.into(),
                    startup: None,
                    image: None,
                })
                .collect(),
        };

        assert_eq!(current(&game, Some("fabric")), Some("fabric"));
        assert_eq!(current(&game, Some("forge")), Some("paper"));
        assert_eq!(current(&game, None), Some("paper"));
    }

    #[test]
    fn versions_are_plain() {
        assert!(plain_version("1.21.4"));
        assert!(plain_version("24w14a"));
        assert!(plain_version("1.20.1-pre2"));
        assert!(!plain_version("1.21; rm -rf /"));
        assert!(!plain_version(""));
    }
}
