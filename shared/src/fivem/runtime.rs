//! plugged.host: one FiveM server's runtime: what its variables ask for, what is installed, and
//! how a change reaches the server (docs/features/fivem.md).
//!
//! The panel never downloads an artifact. The image does (`fivem-runtime`, at every start), so
//! changing the build or the edition is: set the variable, move the server onto the image when it
//! is not on it, and restart it when it runs. A stopped server picks the change up at its next
//! start, which is also when an auto-updating server gets a new build.

use crate::{
    minecraft::files,
    models::{
        UpdatableModel,
        server::{Server, UpdateServerOptions},
        server_variable::ServerVariable,
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The egg variables the image reads.
pub const VERSION: &str = "FIVEM_VERSION";
pub const EDITION: &str = "FIVEM_EDITION";
/// What `fivem-runtime` records about the artifact it installed (the image's own file name).
const MARKER: &str = "alpine/.plugged-host.json";

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Legacy,
    Enhanced,
}

impl Edition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Enhanced => "enhanced",
        }
    }

    fn from_value(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("enhanced") => Self::Enhanced,
            _ => Self::Legacy,
        }
    }
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Installed {
    pub edition: Edition,
    pub build: Option<u32>,
    pub installed_at: Option<String>,
}

/// `recommended`, `latest` or a build number, read the way `fivem-runtime` reads it.
pub fn version_of(value: Option<&str>) -> compact_str::CompactString {
    match value.map(str::trim) {
        Some(value)
            if super::artifacts::CHANNELS.contains(&value)
                || (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())) =>
        {
            value.into()
        }
        _ => "recommended".into(),
    }
}

pub struct Runtime {
    variables: Vec<ServerVariable>,
}

impl Runtime {
    pub async fn load(state: &crate::State, server: &Server) -> Result<Self, anyhow::Error> {
        Ok(Self {
            variables: ServerVariable::all_by_server_uuid_egg_uuid(
                &state.database,
                server.uuid,
                server.egg.uuid,
            )
            .await?,
        })
    }

    fn variable(&self, name: &str) -> Option<&ServerVariable> {
        self.variables
            .iter()
            .find(|variable| variable.variable.env_variable == name)
    }

    /// Whether the egg has the variable (an egg made for the image has both).
    pub fn has(&self, name: &str) -> bool {
        self.variable(name).is_some()
    }

    pub fn version(&self) -> compact_str::CompactString {
        version_of(
            self.variable(VERSION)
                .map(|variable| variable.value.as_str()),
        )
    }

    pub fn edition(&self) -> Edition {
        Edition::from_value(
            self.variable(EDITION)
                .map(|variable| variable.value.as_str()),
        )
    }

    /// Writes a variable; `false` when the egg does not have it.
    pub async fn set(
        &self,
        state: &crate::State,
        server: &Server,
        name: &str,
        value: &str,
    ) -> Result<bool, anyhow::Error> {
        let Some(variable) = self.variable(name) else {
            return Ok(false);
        };
        ServerVariable::create(&state.database, server.uuid, variable.variable.uuid, value).await?;

        Ok(true)
    }
}

/// The image FiveM servers run on (Admin › Settings › Hosting › FiveM), empty when not set.
pub async fn image(state: &crate::State) -> Result<compact_str::CompactString, anyhow::Error> {
    Ok(state.settings.get().await?.games.fivem_image.trim().into())
}

/// An image reference without Wings' `~` (do not pull) and without its tag: a server on another
/// tag of the same image still has `fivem-runtime`.
fn repository(image: &str) -> &str {
    let image = image.trim().trim_start_matches('~');
    match image.rfind(':') {
        Some(colon) if !image[colon..].contains('/') => &image[..colon],
        _ => image,
    }
}

pub fn on_image(server_image: &str, image: &str) -> bool {
    !image.is_empty() && repository(server_image) == repository(image)
}

/// What `fivem-runtime` last installed, `None` when it has not (a server not on the image).
pub async fn installed(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<Option<Installed>, anyhow::Error> {
    let Some(raw) = files::read_text(client, server, MARKER, 64 * 1024).await? else {
        return Ok(None);
    };
    let Ok(marker) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(None);
    };

    Ok(Some(Installed {
        edition: Edition::from_value(marker.get("edition").and_then(serde_json::Value::as_str)),
        build: marker
            .get("build")
            .and_then(|build| {
                build
                    .as_u64()
                    .or_else(|| build.as_str().and_then(|b| b.parse().ok()))
            })
            .and_then(|build| u32::try_from(build).ok()),
        installed_at: marker
            .get("installed_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    }))
}

/// Moves the server onto the image; Wings recreates the container with it at the next start.
pub async fn adopt(state: &crate::State, server: &mut Server) -> Result<(), anyhow::Error> {
    let image = image(state).await?;
    if image.is_empty() || on_image(&server.image, &image) {
        return Ok(());
    }

    server
        .update(
            state,
            UpdateServerOptions {
                image: Some(image),
                ..Default::default()
            },
        )
        .await?;

    Ok(())
}

/// When a change applies.
#[derive(ToSchema, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Restarting,
    NextStart,
}

/// After a variable change: moves the server onto the image and restarts it when it runs and
/// `restart` allows.
pub async fn apply(
    state: &crate::State,
    server: &mut Server,
    restart: bool,
) -> Result<Outcome, anyhow::Error> {
    adopt(state, server).await?;
    if !restart {
        return Ok(Outcome::NextStart);
    }

    let client = files::client(state, server).await?;
    if !matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Running | wings_api::ServerState::Starting)
    ) {
        return Ok(Outcome::NextStart);
    }

    client
        .post_servers_server_power(
            server.uuid,
            &wings_api::servers_server_power::post::RequestBody {
                action: wings_api::ServerPowerAction::Restart,
                wait_seconds: None,
            },
        )
        .await
        .map_err(|err| anyhow::anyhow!("{err:?}"))?;

    Ok(Outcome::Restarting)
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct BuildRow {
    pub build: u32,
    pub recommended: bool,
    pub latest: bool,
    pub installed: bool,
    pub selected: bool,
    /// The day Cfx stops supporting it; past it, it still runs but players are warned.
    pub support_ends: Option<chrono::NaiveDate>,
}

/// What the Artifacts page shows.
#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Overview {
    /// `recommended`, `latest` or a build number.
    pub version: compact_str::CompactString,
    pub edition: Edition,
    /// What the image last installed; `null` when it has not (or the node is unreachable).
    pub installed: Option<Installed>,
    pub unreachable: bool,
    /// The server runs on the image (else changes apply once the image is set and it restarts).
    pub on_image: bool,
    /// The egg has the variables, so the build and edition can be changed.
    pub changeable: bool,
    /// The build the next start installs, when it differs from what is installed.
    pub pending: Option<u32>,
    pub builds: Vec<BuildRow>,
    /// The FiveM for GTAV Enhanced build Cfx serves.
    pub enhanced_build: Option<u32>,
    /// Switching to Enhanced is offered (Admin › Settings); back to Legacy always is.
    pub enhanced_available: bool,
    /// RedM has no Enhanced edition.
    pub redm: bool,
}

pub async fn overview(state: &crate::State, server: &Server) -> Result<Overview, anyhow::Error> {
    let runtime = Runtime::load(state, server).await?;
    let (changelog, downloads) = tokio::join!(
        super::artifacts::changelog(state),
        super::artifacts::downloads(state)
    );
    let image = image(state).await?;
    let enhanced_available = state.settings.get().await?.games.fivem_enhanced;

    let client = files::client(state, server).await?;
    let (installed, unreachable) = match installed(&client, server).await {
        Ok(installed) => (installed, false),
        Err(err) => {
            tracing::debug!(server = %server.uuid, "fivem installed artifact: {err:#}");
            (None, true)
        }
    };
    let redm = match super::config::load(state, server).await {
        Ok(config) => config.redm,
        Err(_) => false,
    };

    let version = runtime.version();
    let edition = runtime.edition();
    let installed_legacy = installed
        .as_ref()
        .filter(|installed| installed.edition == Edition::Legacy)
        .and_then(|installed| installed.build);
    let wanted = super::artifacts::resolve(&changelog, &version);

    Ok(Overview {
        builds: downloads
            .builds
            .iter()
            .map(|build| BuildRow {
                build: build.build,
                recommended: changelog.recommended == Some(build.build),
                latest: changelog.latest == Some(build.build),
                installed: installed_legacy == Some(build.build),
                selected: version == build.build.to_string(),
                support_ends: changelog.support.get(&build.build).copied(),
            })
            .collect(),
        pending: (edition == Edition::Legacy)
            .then_some(wanted)
            .flatten()
            .filter(|wanted| Some(*wanted) != installed_legacy),
        version,
        edition,
        installed,
        unreachable,
        on_image: on_image(&server.image, &image),
        changeable: runtime.has(VERSION),
        enhanced_build: downloads.enhanced.map(|build| build.build),
        enhanced_available: enhanced_available && runtime.has(EDITION),
        redm,
    })
}

#[derive(Debug)]
pub enum RuntimeError {
    NotChangeable,
    UnknownBuild,
    NothingToPin,
    NotOffered,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RuntimeError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl RuntimeError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let (message, status) = match self {
            Self::NotChangeable => (
                "this server's egg has no fivem_version / fivem_edition variable",
                StatusCode::CONFLICT,
            ),
            Self::UnknownBuild => ("cfx does not offer that build", StatusCode::BAD_REQUEST),
            Self::NothingToPin => (
                "no build is installed yet to stay on: start the server once",
                StatusCode::CONFLICT,
            ),
            Self::NotOffered => (
                "fivem for gtav enhanced is not offered on this panel",
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => return crate::response::ApiResponse::from(err),
        };

        crate::response::ApiResponse::error(message).with_status(status)
    }
}

/// How the version is chosen: follow a channel, stay on the installed build, or install a build.
#[derive(ToSchema, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum Choice {
    Recommended,
    Latest,
    /// Stay on the build installed now.
    Pin,
    /// Install this build now (a running server restarts).
    Build {
        build: u32,
    },
}

/// Changes `FIVEM_VERSION`; answers the value written and when it applies.
pub async fn choose(
    state: &crate::State,
    server: &mut Server,
    choice: &Choice,
    may_restart: bool,
) -> Result<(compact_str::CompactString, Outcome), RuntimeError> {
    let runtime = Runtime::load(state, server).await?;
    if !runtime.has(VERSION) {
        return Err(RuntimeError::NotChangeable);
    }

    let (value, restart): (compact_str::CompactString, bool) = match choice {
        Choice::Recommended => ("recommended".into(), false),
        Choice::Latest => ("latest".into(), false),
        Choice::Pin => {
            let client = files::client(state, server).await?;
            let build = installed(&client, server)
                .await?
                .filter(|installed| installed.edition == Edition::Legacy)
                .and_then(|installed| installed.build)
                .ok_or(RuntimeError::NothingToPin)?;
            (build.to_string().into(), false)
        }
        Choice::Build { build } => {
            // only what Cfx lists: the page offers no other
            let downloads = super::artifacts::downloads(state).await;
            if !downloads
                .builds
                .iter()
                .any(|offered| offered.build == *build)
            {
                return Err(RuntimeError::UnknownBuild);
            }
            (build.to_string().into(), true)
        }
    };

    runtime.set(state, server, VERSION, &value).await?;
    let outcome = apply(state, server, restart && may_restart).await?;

    Ok((value, outcome))
}

/// Changes `FIVEM_EDITION`; answers when it applies.
pub async fn switch_edition(
    state: &crate::State,
    server: &mut Server,
    edition: Edition,
    may_restart: bool,
) -> Result<Outcome, RuntimeError> {
    let runtime = Runtime::load(state, server).await?;
    if !runtime.has(EDITION) {
        return Err(RuntimeError::NotChangeable);
    }
    if edition == Edition::Enhanced && !state.settings.get().await?.games.fivem_enhanced {
        return Err(RuntimeError::NotOffered);
    }

    runtime
        .set(state, server, EDITION, edition.as_str())
        .await?;

    Ok(apply(state, server, may_restart).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_editions_read_like_the_image() {
        assert_eq!(version_of(Some(" latest ")), "latest");
        assert_eq!(version_of(Some("17000")), "17000");
        assert_eq!(version_of(Some("1.2")), "recommended");
        assert_eq!(version_of(None), "recommended");
        assert_eq!(Edition::from_value(Some("enhanced")), Edition::Enhanced);
        assert_eq!(Edition::from_value(Some("x")), Edition::Legacy);
    }

    #[test]
    fn images_compare_without_tag_and_tilde() {
        assert!(on_image(
            "~ghcr.io/example/fivem:2.1",
            "ghcr.io/example/fivem:2.2"
        ));
        assert!(on_image(
            "registry.example.com:5000/fivem",
            "registry.example.com:5000/fivem:1"
        ));
        assert!(!on_image(
            "ghcr.io/example/yolks:debian",
            "ghcr.io/example/fivem:2.2"
        ));
        assert!(!on_image("ghcr.io/example/fivem:2.2", ""));
    }
}
