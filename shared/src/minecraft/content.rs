//! plugged.host: browsing Modrinth, CurseForge and SpigotMC for a server and installing one
//! plugin or mod onto it (docs/features/minecraft.md). The panel picks the build; Wings
//! downloads it into `plugins/` or `mods/`.

use super::{Kind, Loader, Project, SearchPage, Software, Source, Version};
use crate::models::server::Server;
use serde::Serialize;
use utoipa::ToSchema;

/// What the Content page may offer for the server.
#[derive(ToSchema, Serialize)]
pub struct ContentOptions {
    pub software: Software,
    /// The version `latest` stands for, or the server's own.
    pub game_version: Option<compact_str::CompactString>,
    pub sources: Vec<Source>,
    pub kinds: Vec<Kind>,
    /// The folder single files go to.
    pub directory: Option<&'static str>,
    /// The server can take a modpack (its game can run a mod loader).
    pub modpacks: bool,
}

/// The sources this server can browse, Modrinth first.
pub async fn sources(state: &crate::State, loader: Loader) -> Vec<Source> {
    let mut sources = vec![Source::Modrinth];
    if super::curseforge::key(state).await.is_some() {
        sources.push(Source::CurseForge);
    }
    if loader == Loader::Paper && super::spigot::enabled(state).await {
        sources.push(Source::SpigotMc);
    }

    sources
}

pub async fn options(
    state: &crate::State,
    server: &Server,
) -> Result<ContentOptions, anyhow::Error> {
    let software = super::software(state, server).await?;
    let game_version = software.game_version(state).await;

    let (sources, kinds, directory) = match software.loader {
        Some(loader) => (
            sources(state, loader).await,
            loader.kinds().to_vec(),
            Some(loader.directory()),
        ),
        None => (Vec::new(), Vec::new(), None),
    };

    Ok(ContentOptions {
        software,
        game_version,
        sources,
        kinds,
        directory,
        modpacks: !super::modpacks::loaders(state, server).await?.is_empty(),
    })
}

const SOURCE_FAILED: &str = "the content source did not answer, try again later";

#[derive(Debug)]
pub enum ContentError {
    /// The server runs software no content is made for.
    NoLoader,
    SourceNotOffered,
    KindNotOffered,
    UnknownProject,
    /// A modpack replaces the server's software; it has its own install.
    Modpack,
    NoCompatibleVersion,
    NotDownloadable,
    /// SpigotMC serves only the newest build.
    SpigotLatestOnly,
    WingsBusy,
    Wings(String),
    /// The source did not answer.
    Source(anyhow::Error),
    Other(anyhow::Error),
}

impl ContentError {
    pub fn message(&self) -> String {
        match self {
            Self::NoLoader => "this server runs software without plugins or mods".into(),
            Self::SourceNotOffered => "that source is not offered for this server".into(),
            Self::KindNotOffered => "that kind of content is not offered for this server".into(),
            Self::UnknownProject => "that project does not exist".into(),
            Self::Modpack => "a modpack is installed from its own dialog".into(),
            Self::NoCompatibleVersion => "no build of that project fits this server".into(),
            Self::NotDownloadable => {
                "that project is not hosted where the panel can download it".into()
            }
            Self::SpigotLatestOnly => "only the newest build can be installed from spigotmc".into(),
            Self::WingsBusy => {
                "the node is busy with other downloads, try again in a moment".into()
            }
            Self::Wings(message) => message.clone(),
            Self::Source(_) => SOURCE_FAILED.into(),
            Self::Other(err) => err.to_string(),
        }
    }

    /// The error as the API answers it; a source's failure is logged.
    pub fn response(self) -> crate::response::ApiResponse {
        let status = axum::http::StatusCode::from_u16(self.status())
            .unwrap_or(axum::http::StatusCode::BAD_REQUEST);
        match self {
            Self::Other(err) => crate::response::ApiResponse::from(err),
            Self::Source(err) => {
                tracing::warn!("a minecraft content source failed: {err:?}");
                crate::response::ApiResponse::error(SOURCE_FAILED).with_status(status)
            }
            err => crate::response::ApiResponse::error(&err.message()).with_status(status),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::UnknownProject => 404,
            Self::WingsBusy => 409,
            Self::Source(_) | Self::Wings(_) => 502,
            Self::Other(_) => 500,
            _ => 400,
        }
    }
}

/// The loader to filter builds by; modpacks bring their own.
fn filter_loader(loader: Loader, kind: Kind) -> Option<Loader> {
    (kind != Kind::Modpack).then_some(loader)
}

/// The Modrinth loaders a build may be for: modpacks may be for any loader a modpack install can
/// switch the server to.
fn modrinth_loaders(loader: Loader, kind: Kind) -> &'static [&'static str] {
    if kind == Kind::Modpack {
        &["fabric", "quilt", "forge", "neoforge"]
    } else {
        loader.modrinth_loaders()
    }
}

/// One page of projects. `game_version` `None` uses the server's, `Some("")` none at all.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    state: &crate::State,
    loader: Loader,
    server_version: Option<&str>,
    source: Source,
    kind: Option<Kind>,
    query: &str,
    game_version: Option<&str>,
    page: u64,
) -> Result<(SearchPage, Kind, Option<compact_str::CompactString>), ContentError> {
    if !sources(state, loader).await.contains(&source) {
        return Err(ContentError::SourceNotOffered);
    }
    let kind = match kind {
        Some(kind) if loader.kinds().contains(&kind) => kind,
        Some(_) => return Err(ContentError::KindNotOffered),
        None => loader.kinds()[0],
    };
    let game_version = match game_version {
        None => server_version.map(Into::into),
        Some("") => None,
        Some(version) => Some(compact_str::CompactString::from(version)),
    };

    let page = match source {
        // SpigotMC filters by nothing; compatibility shows on each build
        Source::SpigotMc => super::spigot::search(state, query, page).await,
        Source::CurseForge => {
            let Some(key) = super::curseforge::key(state).await else {
                return Err(ContentError::SourceNotOffered);
            };
            super::curseforge::search(
                state,
                &key,
                kind,
                Some(loader),
                query,
                game_version.as_deref(),
                page,
            )
            .await
        }
        Source::Modrinth => {
            super::modrinth::search(
                state,
                kind,
                modrinth_loaders(loader, kind),
                query,
                game_version.as_deref(),
                page,
            )
            .await
        }
    }
    .map_err(ContentError::Source)?;

    Ok((
        page,
        kind,
        game_version.filter(|_| source != Source::SpigotMc),
    ))
}

/// A project's builds for the server, newest first. When none fits the server's Minecraft
/// version, every build is offered and `loose` says so (a plugin built for 1.21.1 often runs on
/// 1.21.4).
pub async fn versions(
    state: &crate::State,
    loader: Loader,
    game_version: Option<&str>,
    source: Source,
    project: &str,
    kind: Kind,
) -> Result<(Vec<Version>, bool), ContentError> {
    if !sources(state, loader).await.contains(&source) {
        return Err(ContentError::SourceNotOffered);
    }

    match source {
        Source::SpigotMc => {
            let id = project.parse().map_err(|_| ContentError::UnknownProject)?;
            super::spigot::versions(state, id, game_version)
                .await
                .map_err(ContentError::Source)
        }
        Source::CurseForge => {
            let key = super::curseforge::key(state)
                .await
                .ok_or(ContentError::SourceNotOffered)?;
            let id = project.parse().map_err(|_| ContentError::UnknownProject)?;
            let loader = filter_loader(loader, kind);

            let exact = super::curseforge::files(state, &key, id, game_version, loader)
                .await
                .map_err(ContentError::Source)?;
            if !exact.is_empty() || game_version.is_none() {
                return Ok((exact, false));
            }

            super::curseforge::files(state, &key, id, None, loader)
                .await
                .map(|files| (files, true))
                .map_err(ContentError::Source)
        }
        Source::Modrinth => {
            let loaders = modrinth_loaders(loader, kind);

            let exact = super::modrinth::versions(state, project, loaders, game_version)
                .await
                .map_err(ContentError::Source)?;
            if !exact.is_empty() || game_version.is_none() {
                return Ok((exact, false));
            }

            super::modrinth::versions(state, project, loaders, None)
                .await
                .map(|versions| (versions, true))
                .map_err(ContentError::Source)
        }
    }
}

/// What an install put where.
#[derive(ToSchema, Serialize)]
pub struct Installed {
    pub file: compact_str::CompactString,
    pub directory: &'static str,
    pub version: Version,
    /// The build is not made for the server's Minecraft version.
    pub loose: bool,
    /// Projects the build needs, by id, at the same source.
    pub dependencies: Vec<compact_str::CompactString>,
}

/// Installs one plugin or mod: the build asked for, or the newest release that fits.
pub async fn install(
    state: &crate::State,
    server: &Server,
    software: &Software,
    game_version: Option<&str>,
    source: Source,
    project: &str,
    version_id: Option<&str>,
) -> Result<Installed, ContentError> {
    let loader = software.loader.ok_or(ContentError::NoLoader)?;
    let project = project.trim();
    if project.is_empty() {
        return Err(ContentError::UnknownProject);
    }

    let kind = if loader == Loader::Paper {
        Kind::Plugin
    } else {
        Kind::Mod
    };
    let (versions, loose) = versions(state, loader, game_version, source, project, kind).await?;

    let version = match version_id.filter(|id| !id.is_empty()) {
        Some(id) => match versions.iter().find(|v| v.id == id) {
            Some(version) => version.clone(),
            None if source == Source::SpigotMc && !versions.is_empty() => {
                return Err(ContentError::SpigotLatestOnly);
            }
            // a build from the loose list while an exact one exists
            None => match self::versions(state, loader, None, source, project, kind)
                .await?
                .0
                .into_iter()
                .find(|v| v.id == id)
            {
                Some(version) => version,
                None => return Err(ContentError::NoCompatibleVersion),
            },
        },
        None => match versions
            .iter()
            .find(|v| v.channel == "release")
            .or_else(|| versions.first())
        {
            Some(version) => version.clone(),
            None if source == Source::SpigotMc => return Err(ContentError::NotDownloadable),
            None => return Err(ContentError::NoCompatibleVersion),
        },
    };

    let file = version.file.clone().ok_or(ContentError::NotDownloadable)?;
    if file.name.to_ascii_lowercase().ends_with(".mrpack") {
        return Err(ContentError::Modpack);
    }
    if !super::plain_file_name(&file.name) {
        return Err(ContentError::NotDownloadable);
    }

    let url = match (source, file.url.clone()) {
        (_, Some(url)) => url,
        (Source::CurseForge, None) => {
            let key = super::curseforge::key(state)
                .await
                .ok_or(ContentError::SourceNotOffered)?;
            super::curseforge::download_url(
                state,
                &key,
                project.parse().unwrap_or(0),
                version.id.parse().unwrap_or(0),
            )
            .await
            .map_err(ContentError::Source)?
            .ok_or(ContentError::NotDownloadable)?
        }
        _ => return Err(ContentError::NotDownloadable),
    };

    let directory = loader.directory();
    pull(state, server, &url, &file.name, directory).await?;

    Ok(Installed {
        file: file.name,
        directory,
        dependencies: version.dependencies.clone(),
        version,
        loose,
    })
}

/// Hands a download to Wings, which fetches it in the background. Retries while Wings is busy
/// with other downloads of the server.
pub async fn pull(
    state: &crate::State,
    server: &Server,
    url: &str,
    file_name: &str,
    directory: &str,
) -> Result<(), ContentError> {
    pull_with(state, server, url, file_name, directory, false).await
}

/// Like [`pull`], but answers once Wings has the file, and waits longer for a free download slot
/// (a modpack pulls hundreds of files, a few at a time).
pub async fn pull_now(
    state: &crate::State,
    server: &Server,
    url: &str,
    file_name: &str,
    directory: &str,
) -> Result<(), ContentError> {
    pull_with(state, server, url, file_name, directory, true).await
}

/// Wings refusing a download because the server already has as many running as it allows: older
/// Wings say "simultaneous" with a 400, this one "too many concurrent pulls" with a 417.
fn busy(status: reqwest::StatusCode, error: &str) -> bool {
    (status == reqwest::StatusCode::BAD_REQUEST
        || status == reqwest::StatusCode::EXPECTATION_FAILED)
        && (error.contains("simultaneous") || error.contains("concurrent pulls"))
}

async fn pull_with(
    state: &crate::State,
    server: &Server,
    url: &str,
    file_name: &str,
    directory: &str,
    foreground: bool,
) -> Result<(), ContentError> {
    let client = server
        .node
        .fetch_cached(&state.database)
        .await
        .map_err(|err| ContentError::Other(err.into()))?
        .api_client(&state.database)
        .await
        .map_err(ContentError::Other)?;

    // a server that never started has no plugins/ or mods/ yet
    let _ = client
        .post_servers_server_files_create_directory(
            server.uuid,
            &wings_api::servers_server_files_create_directory::post::RequestBody {
                root: "/".into(),
                name: directory.into(),
            },
        )
        .await;

    let body = wings_api::servers_server_files_pull::post::RequestBody {
        root: format!("/{directory}").into(),
        url: url.into(),
        file_name: Some(file_name.into()),
        use_header: false,
        foreground,
    };

    // 5 seconds for one install, 3 minutes for a file of a modpack
    let attempts = if foreground { 360 } else { 10 };
    for _ in 0..attempts {
        match client
            .post_servers_server_files_pull(server.uuid, &body)
            .await
        {
            Ok(_) => return Ok(()),
            Err(wings_api::client::ApiHttpError::Http(status, err))
                if busy(status, &format!("{err:?}")) =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            Err(err) => {
                return Err(ContentError::Wings(format!(
                    "the node did not take the download: {err:?}"
                )));
            }
        }
    }

    Err(ContentError::WingsBusy)
}

/// A project of any source by id, for naming it.
pub async fn project(
    state: &crate::State,
    source: Source,
    id: &str,
    kind: Kind,
) -> Result<Option<Project>, anyhow::Error> {
    match source {
        Source::Modrinth => super::modrinth::project_by_id(state, id, kind).await,
        Source::CurseForge => match super::curseforge::key(state).await {
            Some(key) => {
                super::curseforge::project_by_id(state, &key, id.parse().unwrap_or(0)).await
            }
            None => Ok(None),
        },
        Source::SpigotMc => super::spigot::project_by_id(state, id.parse().unwrap_or(0)).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_wings_is_told_from_a_failed_download() {
        assert!(busy(
            reqwest::StatusCode::EXPECTATION_FAILED,
            "too many concurrent pulls"
        ));
        assert!(busy(
            reqwest::StatusCode::BAD_REQUEST,
            "too many simultaneous downloads"
        ));
        assert!(!busy(
            reqwest::StatusCode::EXPECTATION_FAILED,
            "failed to create pull: 404"
        ));
    }

    #[test]
    fn modpacks_are_for_any_loader_a_pack_can_switch_to() {
        assert_eq!(modrinth_loaders(Loader::Forge, Kind::Mod), &["forge"]);
        assert!(modrinth_loaders(Loader::Forge, Kind::Modpack).contains(&"fabric"));
        assert_eq!(filter_loader(Loader::Fabric, Kind::Modpack), None);
    }
}
