//! plugged.host: switching a server between its game's software choices, then reinstalling it
//! (docs/features/server-settings.md).

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
}

#[derive(Debug)]
pub enum SoftwareError {
    /// The server's egg is not a game with software choices.
    NotOffered,
    UnknownVariant,
    Other(anyhow::Error),
}

impl SoftwareError {
    pub fn message(&self) -> String {
        match self {
            Self::NotOffered => "this server has no software to choose from".into(),
            Self::UnknownVariant => "that software is not offered for this server".into(),
            Self::Other(err) => err.to_string(),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::NotOffered | Self::UnknownVariant => 400,
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

/// What the server can switch to; `None` when its game offers no choice.
pub async fn options(
    state: &crate::State,
    server: &Server,
) -> Result<Option<ApiSoftware>, anyhow::Error> {
    let Some(game) = game(state, server).await? else {
        return Ok(None);
    };

    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;
    let value = variables
        .iter()
        .find(|variable| Some(&variable.variable.env_variable) == game.variable.as_ref())
        .map(|variable| variable.value.as_str());

    Ok(Some(ApiSoftware {
        current: current(&game, value).map(Into::into),
        variants: game
            .variants
            .iter()
            .map(|variant| ApiGameVariant {
                value: variant.value.clone(),
                label: variant.label.clone(),
            })
            .collect(),
    }))
}

/// Writes the choice to the game's variable, takes its startup command and image, and reinstalls
/// the server (its files stay; the install puts the new software in place).
pub async fn switch(
    state: &crate::State,
    server: &mut Server,
    value: &str,
) -> Result<(), SoftwareError> {
    let Some(game) = game(state, server).await? else {
        return Err(SoftwareError::NotOffered);
    };
    let Some(variant) = game.variants.iter().find(|variant| variant.value == value) else {
        return Err(SoftwareError::UnknownVariant);
    };

    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;
    let Some(variable) = variables
        .iter()
        .find(|variable| Some(&variable.variable.env_variable) == game.variable.as_ref())
    else {
        return Err(SoftwareError::NotOffered);
    };

    ServerVariable::create(
        &state.database,
        server.uuid,
        variable.variable.uuid,
        &variant.value,
    )
    .await?;

    if let Some((startup, image)) = crate::server_creation::runtime(&server.egg, Some(variant)) {
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
}
