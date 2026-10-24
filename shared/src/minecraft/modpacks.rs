//! plugged.host: installing a modpack (docs/features/minecraft.md): a Modrinth `.mrpack` or a
//! CurseForge pack. The panel reads the pack, switches the server to the pack's loader and
//! Minecraft version (a reinstall, see `server_software.rs`) and waits for it, then has Wings
//! download every server file the pack lists and unpacks the pack's overrides on top. It runs
//! after the request; the page polls the state stored here.

use super::{Kind, Loader, SearchPage, Source, Version, content, curseforge, files, modrinth};
use crate::models::{
    ByUuid,
    server::{Server, ServerStatus},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use utoipa::ToSchema;

/// A pack is a list of files plus configs; one bigger than this bundles more than it should.
const MAX_PACK_BYTES: u64 = 256 * 1024 * 1024;
/// The overrides are held in memory twice (read, then zipped again); past this they are left
/// out and the player is told to upload them.
const MAX_OVERRIDES_BYTES: u64 = 256 * 1024 * 1024;
const OVERRIDES_ARCHIVE: &str = ".modpack-overrides.zip";
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
/// How long a reinstall for the pack's software may take.
const INSTALL_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
/// Wings runs three downloads per server at a time by default.
const PARALLEL_PULLS: usize = 3;
const RUNNING_TTL: u64 = 2 * 60 * 60;
const DONE_TTL: u64 = 60 * 60;
/// Where a `.mrpack` may download its files from (Modrinth's rules for the format).
const MRPACK_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Download,
    Software,
    Files,
    Overrides,
    Done,
    Failed,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Install {
    pub source: Source,
    pub project: String,
    pub version: String,
    /// The pack's name and version, as players read it.
    pub name: String,
    pub step: Step,
    /// Files downloaded so far, of `total`.
    pub files: u32,
    pub total: u32,
    /// Files the pack lists that could not be downloaded (a CurseForge author who forbids
    /// downloads by other apps, a dead link); the player adds them by hand.
    pub skipped: Vec<String>,
    /// The overrides were too big to unpack through the panel.
    pub overrides_skipped: bool,
    pub minecraft: Option<String>,
    pub loader: Option<Loader>,
    pub error: Option<String>,
    pub started: chrono::DateTime<chrono::Utc>,
}

/// What the Modpacks tab offers for the server.
#[derive(ToSchema, Serialize)]
pub struct Modpacks {
    /// The loaders a pack may be for: the server's game's choices, or its own loader.
    pub loaders: Vec<Loader>,
    pub sources: Vec<Source>,
    pub running: bool,
    pub install: Option<Install>,
}

#[derive(Debug)]
pub enum ModpackError {
    NoLoaders,
    SourceNotOffered,
    UnknownVersion,
    NotDownloadable,
    Running,
    Installing,
    /// The pack is not a readable `.mrpack` / CurseForge pack.
    Broken(String),
    /// The pack needs a loader the server cannot switch to.
    Unsupported(String),
    TooLarge,
    InstallFailed,
    Software(crate::server_software::SoftwareError),
    Source(anyhow::Error),
    Wings(anyhow::Error),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for ModpackError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for ModpackError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl ModpackError {
    pub fn message(&self) -> String {
        match self {
            Self::NoLoaders => {
                "this server cannot run a mod loader, so it cannot take a modpack".into()
            }
            Self::SourceNotOffered => "that source is not offered".into(),
            Self::UnknownVersion => "that modpack version was not found".into(),
            Self::NotDownloadable => {
                "that modpack version cannot be downloaded by the panel".into()
            }
            Self::Running => "stop the server to install a modpack".into(),
            Self::Installing => "a modpack is already being installed".into(),
            Self::Broken(reason) => format!("the modpack could not be read: {reason}"),
            Self::Unsupported(loader) => {
                format!("the modpack needs {loader}, which this server cannot run")
            }
            Self::TooLarge => "the modpack is too large to install through the panel".into(),
            Self::InstallFailed => "reinstalling the server for the modpack failed".into(),
            Self::Software(err) => err.message(),
            Self::Source(_) => "the modpack's source did not answer, try again later".into(),
            Self::Wings(_) => {
                "the server's files could not be reached, try again in a moment".into()
            }
            Self::Other(err) => err.to_string(),
        }
    }

    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let status = match &self {
            Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Running | Self::Installing => StatusCode::CONFLICT,
            Self::UnknownVersion => StatusCode::NOT_FOUND,
            Self::Source(err) | Self::Wings(err) => {
                tracing::warn!("minecraft modpack: {err:#}");
                StatusCode::BAD_GATEWAY
            }
            _ => StatusCode::BAD_REQUEST,
        };

        match self {
            Self::Other(err) => crate::response::ApiResponse::from(err),
            other => crate::response::ApiResponse::error(&other.message()).with_status(status),
        }
    }
}

fn key(server: &Server) -> String {
    format!("minecraft::modpack::{}", server.uuid)
}

pub async fn current(state: &crate::State, server: &Server) -> Option<Install> {
    state
        .cache
        .get::<Install>(&key(server))
        .await
        .ok()
        .flatten()
}

async fn store(state: &crate::State, server: &Server, install: &Install) {
    let ttl = match install.step {
        Step::Done | Step::Failed => DONE_TTL,
        _ => RUNNING_TTL,
    };
    state.cache.set(&key(server), ttl, install).await.ok();
}

fn running(install: &Install) -> bool {
    !matches!(install.step, Step::Done | Step::Failed)
}

/// The loaders a pack installed on this server may be for.
pub async fn loaders(state: &crate::State, server: &Server) -> Result<Vec<Loader>, anyhow::Error> {
    let settings = state.settings.get().await?;
    let game = settings
        .games
        .game(server.egg.uuid)
        .filter(|game| !game.variants.is_empty() && game.variable.is_some())
        .cloned();
    drop(settings);

    let mut loaders: Vec<Loader> = Vec::new();
    for loader in game
        .iter()
        .flat_map(|game| game.variants.iter())
        .filter_map(|variant| Loader::from_value(&variant.value))
    {
        if loader != Loader::Paper && !loaders.contains(&loader) {
            loaders.push(loader);
        }
    }
    if loaders.is_empty()
        && let Some(loader) = super::software(state, server).await?.loader
        && loader != Loader::Paper
    {
        loaders.push(loader);
    }

    Ok(loaders)
}

fn modrinth_names(loaders: &[Loader]) -> Vec<&'static str> {
    loaders
        .iter()
        .map(|loader| match loader {
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
            Loader::Paper => "paper",
        })
        .collect()
}

async fn sources(state: &crate::State) -> Vec<Source> {
    let mut sources = vec![Source::Modrinth];
    if curseforge::key(state).await.is_some() {
        sources.push(Source::CurseForge);
    }

    sources
}

pub async fn options(state: &crate::State, server: &Server) -> Result<Modpacks, ModpackError> {
    let client = files::client(state, server).await?;

    Ok(Modpacks {
        loaders: loaders(state, server).await?,
        sources: sources(state).await,
        running: !matches!(
            files::power_state(&client, server).await,
            Some(wings_api::ServerState::Offline)
        ),
        install: current(state, server).await,
    })
}

pub async fn search(
    state: &crate::State,
    server: &Server,
    source: Source,
    query: &str,
    page: u64,
) -> Result<SearchPage, ModpackError> {
    let loaders = loaders(state, server).await?;
    if loaders.is_empty() {
        return Err(ModpackError::NoLoaders);
    }

    match source {
        Source::Modrinth => modrinth::search(
            state,
            Kind::Modpack,
            &modrinth_names(&loaders),
            query,
            None,
            page,
        )
        .await
        .map_err(ModpackError::Source),
        Source::CurseForge => {
            let key = curseforge::key(state)
                .await
                .ok_or(ModpackError::SourceNotOffered)?;
            // CurseForge filters by one loader at most
            let loader = (loaders.len() == 1).then(|| loaders[0]);

            curseforge::search(state, &key, Kind::Modpack, loader, query, None, page)
                .await
                .map_err(ModpackError::Source)
        }
        Source::SpigotMc => Err(ModpackError::SourceNotOffered),
    }
}

pub async fn versions(
    state: &crate::State,
    server: &Server,
    source: Source,
    project: &str,
) -> Result<Vec<Version>, ModpackError> {
    let loaders = loaders(state, server).await?;
    if loaders.is_empty() {
        return Err(ModpackError::NoLoaders);
    }

    match source {
        Source::Modrinth => modrinth::versions(state, project, &modrinth_names(&loaders), None)
            .await
            .map_err(ModpackError::Source),
        Source::CurseForge => {
            let key = curseforge::key(state)
                .await
                .ok_or(ModpackError::SourceNotOffered)?;
            let id = project.parse().map_err(|_| ModpackError::UnknownVersion)?;

            curseforge::files(state, &key, id, None, None)
                .await
                .map_err(ModpackError::Source)
        }
        Source::SpigotMc => Err(ModpackError::SourceNotOffered),
    }
}

/// Where a started install downloads its pack from.
pub struct Start {
    pub install: Install,
    url: String,
    clean: bool,
}

/// Checks that the pack can be installed and stores the install as started; [`install`] then
/// does the work.
pub async fn start(
    state: &crate::State,
    server: &Server,
    source: Source,
    project: &str,
    version: &str,
    clean: bool,
) -> Result<Start, ModpackError> {
    if loaders(state, server).await?.is_empty() {
        return Err(ModpackError::NoLoaders);
    }
    if !sources(state).await.contains(&source) {
        return Err(ModpackError::SourceNotOffered);
    }
    let client = files::client(state, server).await?;
    if !matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Offline)
    ) {
        return Err(ModpackError::Running);
    }
    if current(state, server)
        .await
        .is_some_and(|install| running(&install))
    {
        return Err(ModpackError::Installing);
    }

    let (pack, url) = match source {
        Source::Modrinth => {
            let pack = modrinth::version_by_id(state, version)
                .await
                .map_err(ModpackError::Source)?
                .filter(|pack| pack.project_id == project)
                .ok_or(ModpackError::UnknownVersion)?;
            let url = pack
                .file
                .as_ref()
                .filter(|file| file.name.to_ascii_lowercase().ends_with(".mrpack"))
                .and_then(|file| file.url.clone())
                .filter(|url| url.starts_with("https://cdn.modrinth.com/"))
                .ok_or(ModpackError::NotDownloadable)?;

            (pack, url.to_string())
        }
        Source::CurseForge => {
            let key = curseforge::key(state)
                .await
                .ok_or(ModpackError::SourceNotOffered)?;
            let id: u64 = project.parse().map_err(|_| ModpackError::UnknownVersion)?;
            let file_id: u64 = version.parse().map_err(|_| ModpackError::UnknownVersion)?;
            let pack = curseforge::file_by_id(state, &key, id, file_id)
                .await
                .map_err(ModpackError::Source)?
                .ok_or(ModpackError::UnknownVersion)?;
            let url = curseforge::download_url(state, &key, id, file_id)
                .await
                .map_err(ModpackError::Source)?
                .ok_or(ModpackError::NotDownloadable)?;

            (pack, url.to_string())
        }
        Source::SpigotMc => return Err(ModpackError::SourceNotOffered),
    };

    let title = content::project(state, source, project, Kind::Modpack)
        .await
        .ok()
        .flatten()
        .map(|project| project.title.to_string())
        .unwrap_or_else(|| pack.name.to_string());
    let install = Install {
        source,
        project: project.to_string(),
        version: version.to_string(),
        name: format!("{title} {}", pack.number),
        step: Step::Download,
        files: 0,
        total: 0,
        skipped: Vec::new(),
        overrides_skipped: false,
        minecraft: None,
        loader: None,
        error: None,
        started: chrono::Utc::now(),
    };
    store(state, server, &install).await;

    Ok(Start {
        install,
        url,
        clean,
    })
}

/// Installs a started pack. Takes minutes (a reinstall, then hundreds of downloads), so it runs
/// after the request.
pub async fn install(state: &crate::State, server: Server, start: Start) -> Install {
    let mut install = start.install.clone();
    let result = run(state, server.clone(), &start, &mut install).await;

    match result {
        Ok(()) => install.step = Step::Done,
        Err(err) => {
            if let ModpackError::Other(err) | ModpackError::Wings(err) | ModpackError::Source(err) =
                &err
            {
                tracing::warn!(server = %server.uuid, "minecraft modpack install failed: {err:#}");
            }
            install.step = Step::Failed;
            install.error = Some(err.message());
        }
    }
    store(state, &server, &install).await;

    install
}

async fn run(
    state: &crate::State,
    mut server: Server,
    start: &Start,
    install: &mut Install,
) -> Result<(), ModpackError> {
    let archive = download(state, &start.url).await?;
    let pack = tokio::task::spawn_blocking(move || read_pack(&archive))
        .await
        .map_err(anyhow::Error::from)??;
    install.minecraft = Some(pack.minecraft.clone());
    install.loader = Some(pack.loader);
    install.overrides_skipped = pack.overrides_skipped;
    install.step = Step::Software;
    store(state, &server, install).await;

    switch_software(state, &mut server, &pack).await?;

    let client = files::client(state, &server).await?;
    if start.clean {
        files::remove(
            state,
            &client,
            &server,
            "/",
            vec![pack.loader.directory().into()],
        )
        .await
        .map_err(ModpackError::Wings)?;
    }

    install.step = Step::Files;
    install.total = pack.files.len() as u32;
    store(state, &server, install).await;
    pull_files(state, &server, pack.files, install).await;

    if let Some(overrides) = pack.overrides {
        install.step = Step::Overrides;
        store(state, &server, install).await;
        unpack_overrides(&client, &server, overrides).await?;
    }

    Ok(())
}

async fn download(state: &crate::State, url: &str) -> Result<Vec<u8>, ModpackError> {
    let mut response = state
        .client
        .get(url)
        .timeout(DOWNLOAD_TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|err| ModpackError::Source(err.into()))?;
    if response.content_length().unwrap_or(0) > MAX_PACK_BYTES {
        return Err(ModpackError::TooLarge);
    }

    let mut archive = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| ModpackError::Source(err.into()))?
    {
        if archive.len() as u64 + chunk.len() as u64 > MAX_PACK_BYTES {
            return Err(ModpackError::TooLarge);
        }
        archive.extend_from_slice(&chunk);
    }

    Ok(archive)
}

/// Writes the pack's loader and Minecraft version, when they differ from the server's, and
/// waits for the reinstall that follows.
async fn switch_software(
    state: &crate::State,
    server: &mut Server,
    pack: &Pack,
) -> Result<(), ModpackError> {
    let software = super::software(state, server).await?;
    let settings = state.settings.get().await?;
    let variant = settings
        .games
        .game(server.egg.uuid)
        .filter(|game| game.variable.is_some())
        .and_then(|game| {
            game.variants
                .iter()
                .find(|variant| Loader::from_value(&variant.value) == Some(pack.loader))
                .map(|variant| variant.value.clone())
        });
    drop(settings);

    if variant.is_none() && software.loader != Some(pack.loader) {
        return Err(ModpackError::Unsupported(loader_name(pack.loader).into()));
    }
    let new_variant = variant.filter(|variant| software.value.as_ref() != Some(variant));
    let new_version = software
        .version_variable
        .is_some()
        .then_some(pack.minecraft.as_str())
        .filter(|version| software.version != *version);
    if new_variant.is_none() && new_version.is_none() {
        return Ok(());
    }

    crate::server_software::switch(state, server, new_variant.as_deref(), new_version)
        .await
        .map_err(ModpackError::Software)?;

    let deadline = tokio::time::Instant::now() + INSTALL_WAIT;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        match Server::by_uuid(&state.database, server.uuid).await?.status {
            Some(ServerStatus::Installing) if tokio::time::Instant::now() < deadline => {}
            Some(ServerStatus::Installing) | Some(ServerStatus::InstallFailed) => {
                return Err(ModpackError::InstallFailed);
            }
            _ => return Ok(()),
        }
    }
}

fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Paper => "Paper",
        Loader::Fabric => "Fabric",
        Loader::Quilt => "Quilt",
        Loader::Forge => "Forge",
        Loader::NeoForge => "NeoForge",
    }
}

/// Downloads the pack's files, a few at a time. A file that fails is listed as skipped rather
/// than failing the pack: one dead link should cost one mod, not the install.
async fn pull_files(
    state: &crate::State,
    server: &Server,
    list: Vec<PackFile>,
    install: &mut Install,
) {
    let curseforge_key = curseforge::key(state).await;

    let mut pulls = futures_util::stream::iter(list.into_iter().map(|file| {
        let curseforge_key = curseforge_key.clone();

        async move {
            let label = file.label();
            let target = match file.source {
                FileSource::Url { url, directory, name } => Some((url, directory, name)),
                FileSource::CurseForge { project, file: id } => match &curseforge_key {
                    Some(key) => curseforge_target(state, key, project, id).await,
                    None => None,
                },
            };
            let Some((url, directory, name)) = target else {
                return Err(label);
            };

            match content::pull_now(state, server, &url, &name, &directory).await {
                Ok(()) => Ok(()),
                Err(err) => {
                    tracing::debug!(server = %server.uuid, "modpack file {name}: {}", err.message());
                    Err(name)
                }
            }
        }
    }))
    .buffer_unordered(PARALLEL_PULLS);

    while let Some(result) = pulls.next().await {
        match result {
            Ok(()) => install.files += 1,
            Err(name) => install.skipped.push(name),
        }
        if (install.files + install.skipped.len() as u32) % 5 == 0 {
            store(state, server, install).await;
        }
    }
}

/// A CurseForge file's URL and place: jars go to `mods/`, anything else (resource and shader
/// packs, which only clients load) is left out.
async fn curseforge_target(
    state: &crate::State,
    key: &str,
    project: u64,
    file: u64,
) -> Option<(String, String, String)> {
    let version = curseforge::file_by_id(state, key, project, file)
        .await
        .ok()??;
    let name = version.file?.name.to_string();
    if !super::plain_file_name(&name) || !name.to_ascii_lowercase().ends_with(".jar") {
        return None;
    }
    let url = curseforge::download_url(state, key, project, file)
        .await
        .ok()??;

    Some((url.to_string(), "mods".into(), name))
}

/// Writes the overrides as one archive and has Wings unpack it over the server's files.
async fn unpack_overrides(
    client: &wings_api::client::WingsClient,
    server: &Server,
    overrides: Vec<u8>,
) -> Result<(), ModpackError> {
    client
        .post_servers_server_files_write(
            server.uuid,
            wings_api::client::AsyncRequestReader::new(std::io::Cursor::new(overrides)),
            &wings_api::servers_server_files_write::post::Query {
                file: Some(format!("/{OVERRIDES_ARCHIVE}").into()),
                ..Default::default()
            },
        )
        .await
        .map_err(|err| ModpackError::Wings(err.into()))?;

    let unpacked = client
        .post_servers_server_files_decompress(
            server.uuid,
            &wings_api::servers_server_files_decompress::post::RequestBody {
                root: "/".into(),
                file: OVERRIDES_ARCHIVE.into(),
                foreground: true,
            },
        )
        .await;
    let _ = client
        .post_servers_server_files_delete(
            server.uuid,
            &wings_api::servers_server_files_delete::post::RequestBody {
                root: "/".into(),
                files: vec![OVERRIDES_ARCHIVE.into()],
            },
        )
        .await;
    unpacked.map_err(|err| ModpackError::Wings(err.into()))?;

    Ok(())
}

#[derive(Debug, PartialEq)]
enum FileSource {
    Url {
        url: String,
        directory: String,
        name: String,
    },
    CurseForge {
        project: u64,
        file: u64,
    },
}

#[derive(Debug, PartialEq)]
struct PackFile {
    source: FileSource,
}

impl PackFile {
    fn label(&self) -> String {
        match &self.source {
            FileSource::Url { name, .. } => name.clone(),
            FileSource::CurseForge { project, file } => format!("CurseForge {project}/{file}"),
        }
    }
}

struct Pack {
    minecraft: String,
    loader: Loader,
    files: Vec<PackFile>,
    /// The overrides as a zip to unpack at the server root.
    overrides: Option<Vec<u8>>,
    overrides_skipped: bool,
}

/// A relative path inside the server, `None` for one that could leave it.
fn safe_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.contains('\0') {
        return None;
    }

    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| *part == "." || *part == ".." || part.contains(':'))
    {
        return None;
    }

    Some(parts.join("/"))
}

fn allowed_download(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };

    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| MRPACK_HOSTS.contains(&host))
}

/// The loader a pack's dependency key or CurseForge loader id names.
fn pack_loader(name: &str) -> Option<Loader> {
    match name {
        "fabric-loader" | "fabric" => Some(Loader::Fabric),
        "quilt-loader" | "quilt" => Some(Loader::Quilt),
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::NeoForge),
        _ => None,
    }
}

/// `modrinth.index.json`: the Minecraft version, the loader and the files a server needs.
fn read_mrpack_index(index: &serde_json::Value) -> Result<(String, Loader, Vec<PackFile>), String> {
    let dependencies = index["dependencies"]
        .as_object()
        .ok_or("it names no dependencies")?;
    let minecraft = dependencies
        .get("minecraft")
        .and_then(|version| version.as_str())
        .ok_or("it names no minecraft version")?
        .to_string();
    let loader = dependencies
        .keys()
        .find_map(|name| pack_loader(name))
        .ok_or("it needs no mod loader the panel knows")?;

    let mut files = Vec::new();
    for file in index["files"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        // packs list client-only mods too
        if file["env"]["server"].as_str() == Some("unsupported") {
            continue;
        }
        let Some(path) = file["path"].as_str().and_then(safe_path) else {
            continue;
        };
        let Some(url) = file["downloads"].as_array().and_then(|urls| {
            urls.iter()
                .filter_map(|url| url.as_str())
                .find(|url| allowed_download(url))
        }) else {
            continue;
        };
        let (directory, name) = match path.rsplit_once('/') {
            Some((directory, name)) => (directory.to_string(), name.to_string()),
            None => (String::new(), path.clone()),
        };
        if !super::plain_file_name(&name) {
            continue;
        }

        files.push(PackFile {
            source: FileSource::Url {
                url: url.to_string(),
                directory,
                name,
            },
        });
    }

    Ok((minecraft, loader, files))
}

/// `manifest.json` of a CurseForge pack.
fn read_curseforge_manifest(
    manifest: &serde_json::Value,
) -> Result<(String, Loader, Vec<PackFile>, String), String> {
    let minecraft = manifest["minecraft"]["version"]
        .as_str()
        .ok_or("it names no minecraft version")?
        .to_string();
    let loaders = manifest["minecraft"]["modLoaders"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let loader = loaders
        .iter()
        .find(|loader| loader["primary"].as_bool() == Some(true))
        .or_else(|| loaders.first())
        .and_then(|loader| loader["id"].as_str())
        .and_then(|id| pack_loader(id.split('-').next().unwrap_or_default()))
        .ok_or("it needs no mod loader the panel knows")?;

    let files = manifest["files"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|file| file["required"].as_bool() != Some(false))
        .filter_map(|file| {
            Some(PackFile {
                source: FileSource::CurseForge {
                    project: file["projectID"].as_u64()?,
                    file: file["fileID"].as_u64()?,
                },
            })
        })
        .collect();
    let overrides = manifest["overrides"]
        .as_str()
        .and_then(safe_path)
        .unwrap_or_else(|| "overrides".into());

    Ok((minecraft, loader, files, overrides))
}

fn read_json<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<Result<serde_json::Value, String>> {
    let mut file = archive.by_name(name).ok()?;
    if file.size() > 16 * 1024 * 1024 {
        return Some(Err(format!("{name} is too large")));
    }
    let mut raw = String::new();
    if let Err(err) = file.read_to_string(&mut raw) {
        return Some(Err(format!("{name}: {err}")));
    }

    Some(serde_json::from_str(&raw).map_err(|err| format!("{name}: {err}")))
}

fn read_pack(archive: &[u8]) -> Result<Pack, ModpackError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|_| ModpackError::Broken("it is not a zip archive".into()))?;

    let (minecraft, loader, files, prefixes) = if let Some(index) =
        read_json(&mut zip, "modrinth.index.json")
    {
        let (minecraft, loader, files) = read_mrpack_index(&index.map_err(ModpackError::Broken)?)
            .map_err(ModpackError::Broken)?;
        // server-overrides come last, so they win over overrides
        (
            minecraft,
            loader,
            files,
            vec!["overrides".to_string(), "server-overrides".to_string()],
        )
    } else if let Some(manifest) = read_json(&mut zip, "manifest.json") {
        let (minecraft, loader, files, overrides) =
            read_curseforge_manifest(&manifest.map_err(ModpackError::Broken)?)
                .map_err(ModpackError::Broken)?;
        (minecraft, loader, files, vec![overrides])
    } else {
        return Err(ModpackError::Broken(
            "it has neither modrinth.index.json nor manifest.json".into(),
        ));
    };

    let (overrides, overrides_skipped) = repack_overrides(&mut zip, &prefixes)?;

    Ok(Pack {
        minecraft,
        loader,
        files,
        overrides,
        overrides_skipped,
    })
}

/// The files under the override folders, in a new zip rooted at the server root. `None` when
/// there are none; skipped when they are too big to hold.
fn repack_overrides<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    prefixes: &[String],
) -> Result<(Option<Vec<u8>>, bool), ModpackError> {
    let mut entries: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut total = 0u64;

    for prefix in prefixes {
        for index in 0..zip.len() {
            let Ok(file) = zip.by_index(index) else {
                continue;
            };
            if file.is_dir() {
                continue;
            }
            let Ok(name) = file.name() else {
                continue;
            };
            let Some(path) = name
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(safe_path)
            else {
                continue;
            };

            total += file.size();
            entries.insert(path, index);
        }
    }

    if entries.is_empty() {
        return Ok((None, false));
    }
    if total > MAX_OVERRIDES_BYTES {
        return Ok((None, true));
    }

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (path, index) in entries {
        let mut file = zip
            .by_index(index)
            .map_err(|err| ModpackError::Broken(err.to_string()))?;
        let mut contents = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut contents)
            .map_err(|err| ModpackError::Broken(err.to_string()))?;

        writer
            .start_file(path, options)
            .and_then(|_| writer.write_all(&contents).map_err(Into::into))
            .map_err(|err| ModpackError::Other(err.into()))?;
    }

    let written = writer
        .finish()
        .map_err(|err| ModpackError::Other(err.into()))?;

    Ok((Some(written.into_inner()), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_server() {
        assert_eq!(
            safe_path("mods/sodium.jar").as_deref(),
            Some("mods/sodium.jar")
        );
        assert_eq!(
            safe_path("config\\x.toml").as_deref(),
            Some("config/x.toml")
        );
        assert_eq!(safe_path("mods//a.jar").as_deref(), Some("mods/a.jar"));
        assert_eq!(safe_path("../etc/passwd"), None);
        assert_eq!(safe_path("mods/../../x"), None);
        assert_eq!(safe_path("/etc/passwd"), None);
        assert_eq!(safe_path("C:/x"), None);
        assert_eq!(safe_path(""), None);
    }

    #[test]
    fn mrpack_files_for_servers_only() {
        let index = serde_json::json!({
            "dependencies": { "minecraft": "1.20.1", "fabric-loader": "0.15.11" },
            "files": [
                {
                    "path": "mods/lithium.jar",
                    "env": { "client": "optional", "server": "required" },
                    "downloads": ["https://cdn.modrinth.com/data/x/lithium.jar"]
                },
                {
                    "path": "mods/sodium.jar",
                    "env": { "client": "required", "server": "unsupported" },
                    "downloads": ["https://cdn.modrinth.com/data/y/sodium.jar"]
                },
                { "path": "mods/evil.jar", "downloads": ["https://example.com/evil.jar"] },
                { "path": "../evil.jar", "downloads": ["https://cdn.modrinth.com/evil.jar"] },
            ]
        });

        let (minecraft, loader, files) = read_mrpack_index(&index).unwrap();
        assert_eq!(minecraft, "1.20.1");
        assert_eq!(loader, Loader::Fabric);
        assert_eq!(
            files,
            vec![PackFile {
                source: FileSource::Url {
                    url: "https://cdn.modrinth.com/data/x/lithium.jar".into(),
                    directory: "mods".into(),
                    name: "lithium.jar".into(),
                },
            }]
        );
        assert!(
            read_mrpack_index(&serde_json::json!({ "dependencies": { "minecraft": "1.21" } }))
                .is_err()
        );
    }

    #[test]
    fn curseforge_manifests() {
        let manifest = serde_json::json!({
            "minecraft": {
                "version": "1.20.1",
                "modLoaders": [{ "id": "forge-47.2.0", "primary": true }]
            },
            "files": [
                { "projectID": 238222, "fileID": 4712345, "required": true },
                { "projectID": 1, "fileID": 2, "required": false },
            ],
            "overrides": "overrides"
        });

        let (minecraft, loader, files, overrides) = read_curseforge_manifest(&manifest).unwrap();
        assert_eq!((minecraft.as_str(), loader), ("1.20.1", Loader::Forge));
        assert_eq!(
            files,
            vec![PackFile {
                source: FileSource::CurseForge {
                    project: 238222,
                    file: 4712345,
                },
            }]
        );
        assert_eq!(overrides, "overrides");
    }

    #[test]
    fn overrides_are_repacked_at_the_root() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in [
            ("modrinth.index.json", "{}"),
            ("overrides/config/a.toml", "a = 1"),
            ("overrides/../escape.txt", "no"),
            ("server-overrides/config/a.toml", "a = 2"),
            ("client-overrides/options.txt", "client"),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        let archive = writer.finish().unwrap().into_inner();

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).unwrap();
        let (overrides, skipped) = repack_overrides(
            &mut zip,
            &["overrides".to_string(), "server-overrides".to_string()],
        )
        .unwrap();
        assert!(!skipped);

        let mut repacked = zip::ZipArchive::new(std::io::Cursor::new(overrides.unwrap())).unwrap();
        assert_eq!(repacked.len(), 1);
        let mut contents = String::new();
        repacked
            .by_name("config/a.toml")
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "a = 2");
    }

    #[test]
    fn downloads_come_from_the_format_hosts() {
        assert!(allowed_download("https://cdn.modrinth.com/data/a/b.jar"));
        assert!(!allowed_download("http://cdn.modrinth.com/data/a/b.jar"));
        assert!(!allowed_download(
            "https://cdn.modrinth.com.example.com/b.jar"
        ));
    }
}
