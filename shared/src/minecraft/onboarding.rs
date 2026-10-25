//! plugged.host: the setup a player's new Minecraft server starts with (docs/features/minecraft.md):
//! software and version or a modpack, Bedrock, who may join, and who runs it, asked in plain words
//! while the server installs, then applied once the install is over, after which the server
//! starts.
//!
//! A `server_onboardings` row is made when a player creates a Minecraft server; servers an admin
//! creates have none and never see the setup. The answers are stored on the row when submitted
//! (`completed`), and the work that follows runs after the request; the page polls its progress.

use super::{Loader, Source, bedrock, files, modpacks, players};
use crate::models::server::Server;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const RUNNING_TTL: u64 = 2 * 60 * 60;
const DONE_TTL: u64 = 60 * 60;
/// Names per list, as many as a first setup reasonably holds.
const MAX_NAMES: usize = 20;

#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModpackChoice {
    #[garde(skip)]
    pub source: Source,
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub project: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub version: compact_str::CompactString,
}

#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Answers {
    /// The software choice; `null` keeps what the server was created with.
    #[garde(inner(length(chars, min = 1, max = 64)))]
    #[schema(min_length = 1, max_length = 64)]
    #[serde(default)]
    pub variant: Option<compact_str::CompactString>,
    /// The Minecraft version (`latest` or a version); `null` keeps the server's.
    #[garde(inner(length(chars, min = 1, max = 32)))]
    #[schema(min_length = 1, max_length = 32)]
    #[serde(default)]
    pub version: Option<compact_str::CompactString>,
    /// A modpack, which brings its own software and version.
    #[garde(dive)]
    #[serde(default)]
    pub modpack: Option<ModpackChoice>,
    #[garde(skip)]
    #[serde(default)]
    pub bedrock: bool,
    /// `online-mode`: players need a bought Minecraft account.
    #[garde(skip)]
    pub online_mode: bool,
    /// Only the players on the whitelist join.
    #[garde(skip)]
    pub private: bool,
    #[garde(length(max = MAX_NAMES), inner(length(chars, min = 1, max = 16)))]
    #[schema(max_items = 20)]
    #[serde(default)]
    pub players: Vec<compact_str::CompactString>,
    #[garde(length(max = MAX_NAMES), inner(length(chars, min = 1, max = 16)))]
    #[schema(max_items = 20)]
    #[serde(default)]
    pub operators: Vec<compact_str::CompactString>,
}

impl Answers {
    /// Whether the answers change what the server runs (software, version or a modpack).
    pub fn reinstalls(&self) -> bool {
        self.modpack.is_some() || self.variant.is_some() || self.version.is_some()
    }

    /// The names to write: valid Minecraft names, each once (case-insensitively).
    fn names(names: &[compact_str::CompactString]) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for name in names.iter().map(|name| name.trim()) {
            if players::valid_name(name)
                && !seen.iter().any(|known| known.eq_ignore_ascii_case(name))
            {
                seen.push(name.to_string());
            }
        }

        seen
    }
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// The server's first install is still running.
    Waiting,
    Software,
    Modpack,
    /// server.properties, the whitelist and the operators.
    Settings,
    Bedrock,
    Starting,
    Done,
    Failed,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Progress {
    pub step: Step,
    /// Why the setup stopped, or what part of it did not work.
    pub error: Option<String>,
    /// Names that could not be added (no such Minecraft account).
    pub skipped: Vec<String>,
    pub started: chrono::DateTime<chrono::Utc>,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Onboarding {
    /// The server still waits for its setup.
    pub pending: bool,
    pub completed: Option<chrono::DateTime<chrono::Utc>>,
    pub progress: Option<Progress>,
    /// The software choices Bedrock players can join, by value.
    pub bedrock_variants: Vec<compact_str::CompactString>,
    /// The loaders Bedrock players can join, for a modpack's loader.
    pub bedrock_loaders: Vec<Loader>,
    /// Whether the software the server runs now lets Bedrock players join.
    pub bedrock_now: bool,
    /// The software choices that take modpacks, by value, with the loader their packs are for
    /// (the setup's modpack step; Paper-family software takes plugins instead).
    pub modpack_loaders: std::collections::BTreeMap<compact_str::CompactString, Loader>,
}

#[derive(Debug)]
pub enum OnboardingError {
    /// The server has no setup, or it was already done.
    NotPending,
    Other(anyhow::Error),
}

impl From<sqlx::Error> for OnboardingError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl OnboardingError {
    pub fn response(self) -> crate::response::ApiResponse {
        match self {
            Self::NotPending => crate::response::ApiResponse::error("this server's setup is done")
                .with_status(axum::http::StatusCode::CONFLICT),
            Self::Other(err) => crate::response::ApiResponse::from(err),
        }
    }
}

fn key(server_uuid: uuid::Uuid) -> String {
    format!("minecraft::onboarding::{server_uuid}")
}

async fn store(state: &crate::State, server_uuid: uuid::Uuid, progress: &Progress) {
    let ttl = match progress.step {
        Step::Done | Step::Failed => DONE_TTL,
        _ => RUNNING_TTL,
    };
    state.cache.set(&key(server_uuid), ttl, progress).await.ok();
}

/// Marks a player's new Minecraft server for setup.
pub async fn require(state: &crate::State, server_uuid: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO server_onboardings (server_uuid) VALUES ($1) ON CONFLICT (server_uuid) DO NOTHING",
    )
    .bind(server_uuid)
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// The server's setup, `None` when it has none.
pub async fn get(
    state: &crate::State,
    server: &Server,
) -> Result<Option<Onboarding>, anyhow::Error> {
    let completed: Option<Option<chrono::NaiveDateTime>> =
        sqlx::query_scalar("SELECT completed FROM server_onboardings WHERE server_uuid = $1")
            .bind(server.uuid)
            .fetch_optional(state.database.read())
            .await?;
    let Some(completed) = completed else {
        return Ok(None);
    };

    let settings = state.settings.get().await.map_err(anyhow::Error::from)?;
    let game = settings.games.game(server.egg.uuid);
    let bedrock_variants = game
        .map(|game| {
            game.variants
                .iter()
                .filter(|variant| bedrock::platform(Loader::from_value(&variant.value)).is_some())
                .map(|variant| variant.value.clone())
                .collect()
        })
        .unwrap_or_default();
    let modpack_loaders = modpack_loaders(
        game.iter()
            .flat_map(|game| game.variants.iter())
            .map(|variant| variant.value.as_str()),
    );
    drop(settings);

    Ok(Some(Onboarding {
        pending: completed.is_none(),
        completed: completed.map(|completed| completed.and_utc()),
        progress: state
            .cache
            .get::<Progress>(&key(server.uuid))
            .await
            .ok()
            .flatten(),
        bedrock_variants,
        bedrock_loaders: bedrock_loaders(),
        bedrock_now: bedrock::platform(super::software(state, server).await?.loader).is_some(),
        modpack_loaders,
    }))
}

/// Stores the answers as the server's setup (once) and its progress as started; [`apply`] then
/// makes them true.
pub async fn complete(
    state: &crate::State,
    server: &Server,
    answers: &Answers,
) -> Result<(), OnboardingError> {
    let updated = sqlx::query(
        "UPDATE server_onboardings SET completed = NOW(), answers = $2
         WHERE server_uuid = $1 AND completed IS NULL",
    )
    .bind(server.uuid)
    .bind(serde_json::to_value(answers).map_err(|err| OnboardingError::Other(err.into()))?)
    .execute(state.database.write())
    .await?;
    if updated.rows_affected() == 0 {
        return Err(OnboardingError::NotPending);
    }

    store(
        state,
        server.uuid,
        &Progress {
            step: Step::Waiting,
            error: None,
            skipped: Vec::new(),
            started: chrono::Utc::now(),
        },
    )
    .await;

    Ok(())
}

/// Applies the answers once the server's install is over, then starts it. Takes minutes, so it
/// runs after the request. A part that fails is reported and the rest still applied: a pack that
/// would not install should not also cost the player their whitelist.
pub async fn apply(state: &crate::State, server: Server, answers: Answers) -> Progress {
    let mut progress = Progress {
        step: Step::Waiting,
        error: None,
        skipped: Vec::new(),
        started: chrono::Utc::now(),
    };
    store(state, server.uuid, &progress).await;

    let result = run(state, server.clone(), &answers, &mut progress).await;
    match result {
        Ok(()) => progress.step = Step::Done,
        Err(message) => {
            progress.step = Step::Failed;
            progress.error = Some(message);
        }
    }
    store(state, server.uuid, &progress).await;

    progress
}

async fn run(
    state: &crate::State,
    server: Server,
    answers: &Answers,
    progress: &mut Progress,
) -> Result<(), String> {
    use crate::models::ByUuid;

    let failed = |err: &dyn std::fmt::Display| {
        tracing::warn!(server = %server.uuid, "minecraft setup: {err}");
        "the setup could not be applied, change it on the server's pages".to_string()
    };

    if !crate::server_software::wait_for_install(
        state,
        server.uuid,
        crate::server_software::INSTALL_WAIT,
    )
    .await
    .map_err(|err| failed(&err))?
    {
        return Err("the server's install did not finish; reinstall it from Settings".into());
    }
    let mut server = Server::by_uuid(&state.database, server.uuid)
        .await
        .map_err(|err| failed(&err))?;

    // what the server runs first: everything after writes into its files
    let mut problems: Vec<String> = Vec::new();
    if let Some(modpack) = &answers.modpack {
        progress.step = Step::Modpack;
        store(state, server.uuid, progress).await;

        match modpacks::start(
            state,
            &server,
            modpack.source,
            &modpack.project,
            &modpack.version,
            false,
        )
        .await
        {
            Ok(start) => {
                let install = modpacks::install(state, server.clone(), start).await;
                if let Some(error) = install.error {
                    problems.push(format!("the modpack did not install: {error}"));
                }
            }
            Err(err) => problems.push(format!("the modpack did not install: {}", err.message())),
        }
    } else if answers.variant.is_some() || answers.version.is_some() {
        progress.step = Step::Software;
        store(state, server.uuid, progress).await;

        let software = super::software(state, &server)
            .await
            .map_err(|err| failed(&err))?;
        let variant = answers
            .variant
            .as_deref()
            .filter(|variant| software.value.as_deref() != Some(*variant));
        let version = answers
            .version
            .as_deref()
            .filter(|version| !software.version.eq_ignore_ascii_case(version));

        if variant.is_some() || version.is_some() {
            match crate::server_software::switch(state, &mut server, variant, version).await {
                Ok(()) => {
                    if !crate::server_software::wait_for_install(
                        state,
                        server.uuid,
                        crate::server_software::INSTALL_WAIT,
                    )
                    .await
                    .map_err(|err| failed(&err))?
                    {
                        return Err(
                            "reinstalling with the chosen software failed; reinstall it from Settings"
                                .into(),
                        );
                    }
                }
                Err(err) => {
                    problems.push(format!("the software was not changed: {}", err.message()))
                }
            }
        }
    }
    let server = Server::by_uuid(&state.database, server.uuid)
        .await
        .map_err(|err| failed(&err))?;

    progress.step = Step::Settings;
    store(state, server.uuid, progress).await;
    write_settings(state, &server, answers, progress)
        .await
        .map_err(|err| failed(&err))?;

    if answers.bedrock {
        progress.step = Step::Bedrock;
        store(state, server.uuid, progress).await;

        let software = super::software(state, &server)
            .await
            .map_err(|err| failed(&err))?;
        let result = match bedrock::platform(software.loader) {
            Some(_) => bedrock::install(state, &server).await.map(|_| ()),
            None => Err(bedrock::BedrockError::Unsupported),
        };
        if let Err(err) = result {
            problems.push(format!(
                "bedrock support was not set up: {}",
                bedrock_message(err)
            ));
        }
    }

    progress.step = Step::Starting;
    store(state, server.uuid, progress).await;
    let client = files::client(state, &server)
        .await
        .map_err(|err| failed(&err))?;
    if matches!(
        files::power_state(&client, &server).await,
        Some(wings_api::ServerState::Offline)
    ) {
        client
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Start,
                    wait_seconds: None,
                },
            )
            .await
            .map_err(|err| failed(&format!("{err:?}")))?;
    }

    if !problems.is_empty() {
        progress.error = Some(problems.join("; "));
    }

    Ok(())
}

fn bedrock_message(err: bedrock::BedrockError) -> String {
    match err {
        bedrock::BedrockError::Unsupported => {
            "geyser runs on paper and its forks, fabric and neoforge".into()
        }
        bedrock::BedrockError::NoPort => "the node has no free port left".into(),
        bedrock::BedrockError::Content(err) => err.message(),
        bedrock::BedrockError::Wings(err) | bedrock::BedrockError::Other(err) => {
            tracing::warn!("minecraft setup bedrock: {err:#}");
            "try it again from Plugins & Mods".into()
        }
    }
}

/// `online-mode` first (it decides which UUID a name stands for), then the whitelist, then the
/// operators: a player has to get in before being an operator is of any use.
async fn write_settings(
    state: &crate::State,
    server: &Server,
    answers: &Answers,
    progress: &mut Progress,
) -> Result<(), anyhow::Error> {
    let client = files::client(state, server).await?;
    let raw = files::read_text(&client, server, "server.properties", 1024 * 1024)
        .await?
        .unwrap_or_default();
    files::write_text(
        &client,
        server,
        "server.properties",
        super::config::set_properties(&raw, &[("online-mode", answers.online_mode.to_string())]),
    )
    .await?;

    players::enforce(state, server, answers.private, false)
        .await
        .map_err(|err| anyhow::anyhow!("{err:?}"))?;

    let whitelist = if answers.private {
        Answers::names(&answers.players)
    } else {
        Vec::new()
    };
    let changes = whitelist
        .into_iter()
        .map(|name| (name, players::Change::Whitelist(true)))
        .chain(
            Answers::names(&answers.operators)
                .into_iter()
                .map(|name| (name, players::Change::Operator(true))),
        );
    for (name, change) in changes {
        if let Err(err) = players::change(state, server, &name, change, false).await {
            tracing::debug!(server = %server.uuid, "minecraft setup {name}: {err:?}");
            if !progress.skipped.contains(&name) {
                progress.skipped.push(name);
            }
        }
    }

    Ok(())
}

/// The loaders Bedrock support exists for, for the setup page.
pub fn bedrock_loaders() -> Vec<Loader> {
    [
        Loader::Paper,
        Loader::Fabric,
        Loader::Quilt,
        Loader::Forge,
        Loader::NeoForge,
    ]
    .into_iter()
    .filter(|loader| bedrock::platform(Some(*loader)).is_some())
    .collect()
}

/// The software choices (by value) whose loader takes modpacks, with that loader: every mod
/// loader, not the Paper family (the same rule as [`modpacks::loaders`]).
fn modpack_loaders<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> std::collections::BTreeMap<compact_str::CompactString, Loader> {
    values
        .into_iter()
        .filter_map(|value| {
            Loader::from_value(value)
                .filter(|loader| *loader != Loader::Paper)
                .map(|loader| (value.into(), loader))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modpacks_are_for_mod_loaders_only() {
        let loaders =
            modpack_loaders(["purpur", "fabric", "NeoForge", "forge", "vanilla", "paper"]);

        assert_eq!(
            loaders.into_iter().collect::<Vec<_>>(),
            vec![
                ("NeoForge".into(), Loader::NeoForge),
                ("fabric".into(), Loader::Fabric),
                ("forge".into(), Loader::Forge),
            ]
        );
    }

    fn answers() -> Answers {
        Answers {
            variant: None,
            version: None,
            modpack: None,
            bedrock: false,
            online_mode: true,
            private: true,
            players: Vec::new(),
            operators: Vec::new(),
        }
    }

    #[test]
    fn names_are_valid_and_once() {
        assert_eq!(
            Answers::names(&[
                "Notch".into(),
                "notch".into(),
                " jeb_ ".into(),
                "no spaces".into()
            ]),
            vec!["Notch".to_string(), "jeb_".to_string()]
        );
    }

    #[test]
    fn what_reinstalls() {
        assert!(!answers().reinstalls());
        assert!(
            Answers {
                version: Some("1.21.4".into()),
                ..answers()
            }
            .reinstalls()
        );
        assert_eq!(
            bedrock_loaders(),
            vec![Loader::Paper, Loader::Fabric, Loader::NeoForge]
        );
    }
}
