//! plugged.host: neo's Minecraft tools: the server's software and versions, plugins, mods and
//! modpacks from Modrinth, CurseForge and SpigotMC, and Geyser (docs/features/neo.md › Tools).
//! Offered only on Minecraft servers; they go through the same shared code as the Minecraft pages.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, support};
use crate::minecraft::{
    Kind, LATEST, Loader, Software, Source, Version,
    bedrock::BedrockError,
    content::ContentError,
    jars::{InstalledJar, JarError},
    modpacks::ModpackError,
};
use serde_json::{Value, json};
use std::borrow::Cow;

const SOURCES: [&str; 3] = ["modrinth", "curseforge", "spigotmc"];
const KINDS: [&str; 3] = ["plugin", "mod", "modpack"];
const MAX_RESULTS: usize = 10;
const MAX_VERSIONS: usize = 12;
const MAX_REMOVE: usize = 20;
/// Rounds of identifying jars `list_installed_content` waits for (each does a batch).
const IDENTIFY_ROUNDS: usize = 3;

fn is_minecraft(context: &ToolContext<'_>) -> bool {
    context
        .server
        .is_some_and(|server| crate::minecraft::is_minecraft(&server.egg))
}

/// The server's Minecraft software, refusing other servers.
async fn software(context: &ToolContext<'_>) -> Result<Software, ToolError> {
    let server = context.server()?;
    if !crate::minecraft::is_minecraft(&server.egg) {
        return Err(ToolError::invalid("This is not a Minecraft server."));
    }

    Ok(crate::minecraft::software(context.state, server).await?)
}

/// The software's loader; software without plugins or mods (Vanilla) has none.
fn loader(software: &Software) -> Result<Loader, ToolError> {
    software.loader.ok_or_else(|| {
        ToolError::invalid(
            "This server runs software without plugins or mods (like Vanilla); switch_server_software can change that.",
        )
    })
}

fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Paper => "paper (Bukkit/Spigot/Paper plugins)",
        Loader::Fabric => "fabric (mods)",
        Loader::Quilt => "quilt (mods)",
        Loader::Forge => "forge (mods)",
        Loader::NeoForge => "neoforge (mods)",
    }
}

fn source_name(source: Source) -> &'static str {
    match source {
        Source::Modrinth => "modrinth",
        Source::CurseForge => "curseforge",
        Source::SpigotMc => "spigotmc",
    }
}

fn source(arguments: &Value) -> Result<Source, ToolError> {
    match arguments
        .get("source")
        .and_then(Value::as_str)
        .map(|source| source.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("modrinth") => Ok(Source::Modrinth),
        Some("curseforge") => Ok(Source::CurseForge),
        Some("spigotmc") | Some("spigot") => Ok(Source::SpigotMc),
        Some(other) => Err(ToolError::invalid(format!(
            "Unknown source \"{other}\"; use one of {}.",
            SOURCES.join(", ")
        ))),
    }
}

fn kind(arguments: &Value) -> Result<Option<Kind>, ToolError> {
    match arguments
        .get("kind")
        .and_then(Value::as_str)
        .map(|kind| kind.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") => Ok(None),
        Some("plugin") => Ok(Some(Kind::Plugin)),
        Some("mod") => Ok(Some(Kind::Mod)),
        Some("modpack") => Ok(Some(Kind::Modpack)),
        Some(other) => Err(ToolError::invalid(format!(
            "Unknown kind \"{other}\"; use one of {}.",
            KINDS.join(", ")
        ))),
    }
}

/// A project id from search_content.
fn project(arguments: &Value) -> Result<String, ToolError> {
    let project = support::string(arguments, "project", 64, true)?.trim();
    if project.is_empty() {
        return Err(ToolError::invalid("Give a project id from search_content."));
    }

    Ok(project.to_string())
}

/// A bare jar name inside the content folder, never a path.
fn jar_name(value: Option<&Value>) -> Result<String, ToolError> {
    let name = value.and_then(Value::as_str).unwrap_or_default().trim();
    let lower = name.to_ascii_lowercase();
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name.contains("..")
        || !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled"))
    {
        return Err(ToolError::invalid(format!(
            "\"{name}\" is not a jar file name from list_installed_content."
        )));
    }

    Ok(name.to_string())
}

/// A content refusal as the model hears it; a source or node failure is logged.
fn content_error(err: ContentError) -> ToolError {
    ToolError::invalid(match err {
        ContentError::NoLoader => "This server runs software without plugins or mods; switch_server_software can change that.".to_string(),
        ContentError::SourceNotOffered => {
            "That source is not available for this server (minecraft_info lists the sources).".into()
        }
        ContentError::KindNotOffered => "That kind of content does not fit this server's software (minecraft_info says what it takes).".into(),
        ContentError::UnknownProject => {
            "That project does not exist there; use an id from search_content with the same source.".into()
        }
        ContentError::Modpack => "That project is a modpack; use install_modpack instead.".into(),
        ContentError::NoCompatibleVersion => "No build of this project fits this server's software and version (check content_versions or minecraft_info).".into(),
        ContentError::NotDownloadable => "That project is premium or hosted elsewhere, so it cannot be installed automatically. Look for it on another source, or give the user its page to download the jar and upload it.".into(),
        ContentError::SpigotLatestOnly => "Only the latest build of a SpigotMC resource can be installed; call install_content again without a version.".into(),
        ContentError::WingsBusy => {
            "The node is busy with other downloads for this server; try again in a moment.".into()
        }
        ContentError::Wings(message) => {
            tracing::warn!("neo: minecraft content download refused: {message}");
            "The download could not be started on the node.".into()
        }
        ContentError::Source(err) => {
            tracing::warn!("neo: a minecraft content source failed: {err:#}");
            "The content source did not answer; try again later or use another source.".into()
        }
        ContentError::Other(err) => return ToolError::Failed(err),
    })
}

fn modpack_error(err: ModpackError) -> ToolError {
    match err {
        ModpackError::Other(err) => ToolError::Failed(err),
        ModpackError::Source(ref inner) | ModpackError::Wings(ref inner) => {
            tracing::warn!("neo: minecraft modpack: {inner:#}");
            ToolError::invalid(support::sentence(&err.message()))
        }
        err => ToolError::invalid(support::sentence(&err.message())),
    }
}

fn jar_error(err: JarError) -> ToolError {
    match err {
        JarError::Other(err) => ToolError::Failed(err),
        JarError::NotInstalled => ToolError::invalid(
            "That file is not installed; use a name from list_installed_content.",
        ),
        JarError::Wings(message) => {
            tracing::warn!("neo: minecraft jar change refused: {message}");
            ToolError::invalid("The node did not take the change; try again in a moment.")
        }
        err => ToolError::invalid(support::sentence(&err.message())),
    }
}

/// One build as a line.
fn version_line(version: &Version) -> String {
    let games: Vec<&str> = version
        .game_versions
        .iter()
        .rev()
        .take(4)
        .rev()
        .map(|version| version.as_str())
        .collect();

    format!(
        "- version id {}: {} {} [{}] for {} ({})",
        version.id,
        version.number,
        version.name,
        version.channel,
        if games.is_empty() {
            "any version".to_string()
        } else {
            games.join(", ")
        },
        version.date.get(..10).unwrap_or(version.date.as_str())
    )
}

/// One installed jar as a line.
fn jar_line(jar: &InstalledJar) -> String {
    let mut facts = Vec::new();
    if let Some(version) = &jar.version {
        facts.push(format!("v{version}"));
    }
    if let Some(source) = jar.source {
        facts.push(
            if jar.confidence == crate::minecraft::jars::Confidence::Name {
                format!("{} (by name)", source_name(source))
            } else {
                source_name(source).to_string()
            },
        );
    }
    if jar.disabled {
        facts.push("disabled".into());
    }
    if let Some(update) = &jar.update {
        facts.push(format!("update available: {}", update.number));
    }
    if !jar.missing.is_empty() {
        facts.push(format!("MISSING dependencies: {}", jar.missing.join(", ")));
    }
    if jar.pending {
        facts.push("not identified yet".into());
    }

    format!(
        "- {}: {} ({}){}",
        jar.file,
        jar.name,
        support::human(jar.size),
        if facts.is_empty() {
            String::new()
        } else {
            format!(" [{}]", facts.join("; "))
        }
    )
}

pub struct MinecraftInfoTool;

const INFO_PERMISSIONS: [&str; 3] = [
    "minecraft.content",
    "settings.software",
    "minecraft.bedrock",
];

#[async_trait::async_trait]
impl Tool for MinecraftInfoTool {
    fn name(&self) -> &'static str {
        "minecraft_info"
    }

    fn description(&self) -> Cow<'static, str> {
        "Minecraft details for this server: software (paper takes Bukkit/Spigot/Paper plugins; fabric/forge/neoforge take mods), Minecraft version, installable versions, what content it takes and from which sources, whether it can take a modpack, the software it can switch to, and Geyser (Bedrock) status. Check this before searching or installing content.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn offered_with(&self) -> Vec<&'static str> {
        INFO_PERMISSIONS.to_vec()
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Check Minecraft setup".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        // offered with any of these; each part below shows only with its own
        if !INFO_PERMISSIONS
            .iter()
            .any(|permission| context.can(permission))
        {
            return Err(ToolError::invalid(
                "This needs the Minecraft content, software or Bedrock permission on this server.",
            ));
        }
        let server = context.server()?;
        let software = software(context).await?;
        let game_version = software.game_version(context.state).await;

        let mut lines = vec![format!(
            "software: {} ({})",
            software
                .value
                .as_deref()
                .unwrap_or(server.egg.name.as_str()),
            software.loader.map_or("no plugins or mods", loader_name)
        )];
        let version = if software.version.eq_ignore_ascii_case(LATEST) {
            format!(
                "latest ({})",
                game_version.as_deref().unwrap_or("not known right now")
            )
        } else {
            software.version.to_string()
        };
        lines.push(format!("minecraft_version: {version}"));

        if let Some(loader) = software.loader {
            lines.push(format!(
                "content_types: {}",
                loader
                    .kinds()
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            lines.push(format!("content_directory: /{}", loader.directory()));
            lines.push(format!(
                "sources: {}",
                crate::minecraft::content::sources(context.state, loader)
                    .await
                    .into_iter()
                    .map(source_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let modpack_loaders = crate::minecraft::modpacks::loaders(context.state, server)
            .await
            .unwrap_or_default();
        lines.push(format!(
            "modpacks: {}",
            if modpack_loaders.is_empty() {
                "no (this server cannot run a mod loader)".to_string()
            } else {
                format!(
                    "yes, for {}",
                    modpack_loaders
                        .iter()
                        .map(|loader| loader_name(*loader).split(' ').next().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ));

        if context.can("minecraft.bedrock") {
            lines.push(format!(
                "geyser: {}",
                match crate::minecraft::bedrock::status(context.state, server).await {
                    Ok(bedrock) if !bedrock.available => {
                        "unavailable (not supported on this software)".to_string()
                    }
                    Ok(bedrock) => match (bedrock.installed, bedrock.address, bedrock.port) {
                        (Some(jar), Some(address), Some(port)) => {
                            format!("installed ({jar}), Bedrock players join {address}:{port}")
                        }
                        (Some(jar), _, _) => format!("installed ({jar})"),
                        (None, _, _) => "not installed (install_geyser adds it)".to_string(),
                    },
                    Err(_) => "unknown right now".to_string(),
                }
            ));
        }

        let versions = crate::minecraft::versions::for_software(context.state, software.loader)
            .await
            .unwrap_or_default();
        if !versions.is_empty() {
            lines.push(format!(
                "installable_versions (newest first): {}",
                versions
                    .iter()
                    .take(15)
                    .map(|version| version.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if context.can("settings.software")
            && let Some(options) = crate::server_software::options(context.state, server).await?
            && !options.variants.is_empty()
        {
            lines.push(format!(
                "switchable_software: {} (switching reinstalls the server; the world is kept)",
                options
                    .variants
                    .iter()
                    .map(|variant| format!("{} = {}", variant.value, variant.label))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let display = format!(
            "{} {version}",
            software.value.as_deref().unwrap_or("Minecraft")
        );

        Ok(ToolResult::ok(lines.join("\n")).with_display(display))
    }
}

pub struct SearchContentTool;

#[async_trait::async_trait]
impl Tool for SearchContentTool {
    fn name(&self) -> &'static str {
        "search_content"
    }

    fn description(&self) -> Cow<'static, str> {
        "Search Modrinth (default), CurseForge or SpigotMC for plugins, mods or modpacks that fit this server's software and Minecraft version. SpigotMC is for plugins on Paper servers only, cannot filter by version, and only its latest builds install; prefer Modrinth when a project is on both. Returns project ids to use with content_versions and install_content (or install_modpack), with the same source.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "What to look for, e.g. \"EssentialsX\" or \"world edit\"."},
                "kind": {"type": "string", "enum": KINDS, "description": "Defaults to what the server takes."},
                "source": {"type": "string", "enum": SOURCES},
                "any_version": {"type": "boolean", "description": "Search all Minecraft versions instead of only the server's."},
                "page": {"type": "integer", "minimum": 1},
            },
            "required": ["query"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Search content for \"{}\"",
            support::short(support::shown(arguments, "query"), 40)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let software = software(context).await?;
        let query = support::string(arguments, "query", 100, true)?
            .trim()
            .to_string();
        let source = source(arguments)?;
        let kind = kind(arguments)?;
        let page = support::integer(arguments, "page", 1, 1, 500) as u64;

        let (results, kind, game_version) = if kind == Some(Kind::Modpack) {
            // a modpack brings its own loader: any the server's game can switch to
            let results =
                crate::minecraft::modpacks::search(context.state, server, source, &query, page)
                    .await
                    .map_err(modpack_error)?;
            (results, Kind::Modpack, None)
        } else {
            let loader = loader(&software)?;
            let server_version = software.game_version(context.state).await;
            crate::minecraft::content::search(
                context.state,
                loader,
                server_version.as_deref(),
                source,
                kind,
                &query,
                support::boolean(arguments, "any_version", false).then_some(""),
                page,
            )
            .await
            .map_err(content_error)?
        };

        let version = game_version.as_deref().unwrap_or("any").to_string();
        if results.hits.is_empty() {
            return Ok(ToolResult::ok(format!(
                "No {} results on {} for this server (version {version}).",
                kind.as_str(),
                source_name(source)
            ))
            .with_display("No results"));
        }

        let pages = results.total.div_ceil(results.per_page.max(1)).max(1);
        let mut lines = vec![format!(
            "{} {} results (version {version}, page {} of {pages}):",
            source_name(source),
            kind.as_str(),
            results.page.max(1)
        )];
        for hit in results.hits.iter().take(MAX_RESULTS) {
            lines.push(format!(
                "- {} (id {}, {}, {} downloads{}): {}",
                support::short(&hit.title, 80),
                hit.id,
                hit.kind.as_str(),
                hit.downloads,
                if hit.installable {
                    ""
                } else {
                    ", cannot be installed automatically: premium or hosted elsewhere"
                },
                support::short(&hit.description, 120)
            ));
        }

        Ok(ToolResult::ok(support::limit_output(&lines.join("\n")))
            .with_display(format!("{} results", results.hits.len())))
    }
}

pub struct ContentVersionsTool;

#[async_trait::async_trait]
impl Tool for ContentVersionsTool {
    fn name(&self) -> &'static str {
        "content_versions"
    }

    fn description(&self) -> Cow<'static, str> {
        "List the builds of a plugin, mod or modpack project that fit this server (newest first). install_content picks the newest stable release by itself; use this only when a specific version is needed, and for the version id install_modpack needs.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project": {"type": "string", "description": "Project id from search_content."},
                "source": {"type": "string", "enum": SOURCES},
                "kind": {"type": "string", "enum": KINDS, "description": "Use \"modpack\" for modpacks (their builds are not limited to the server's current loader)."},
            },
            "required": ["project"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "List versions of {}",
            support::short(support::shown(arguments, "project"), 40)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let software = software(context).await?;
        let project = project(arguments)?;
        let source = source(arguments)?;
        let kind = kind(arguments)?;

        let (versions, loose) = if kind == Some(Kind::Modpack) {
            let versions =
                crate::minecraft::modpacks::versions(context.state, server, source, &project)
                    .await
                    .map_err(modpack_error)?;
            (versions, false)
        } else {
            let loader = loader(&software)?;
            let game_version = software.game_version(context.state).await;
            crate::minecraft::content::versions(
                context.state,
                loader,
                game_version.as_deref(),
                source,
                &project,
                kind.unwrap_or(loader.kinds()[0]),
            )
            .await
            .map_err(content_error)?
        };

        if versions.is_empty() {
            return Ok(
                ToolResult::ok("No builds of this project fit this server's software.")
                    .with_display("No compatible builds"),
            );
        }

        let mut lines = Vec::new();
        if loose {
            lines.push(
                "No build lists this exact Minecraft version; these are the closest builds (may still work):"
                    .to_string(),
            );
        }
        lines.extend(versions.iter().take(MAX_VERSIONS).map(version_line));

        Ok(ToolResult::ok(support::limit_output(&lines.join("\n")))
            .with_display(format!("{} builds", versions.len())))
    }
}

pub struct InstallContentTool;

#[async_trait::async_trait]
impl Tool for InstallContentTool {
    fn name(&self) -> &'static str {
        "install_content"
    }

    fn description(&self) -> Cow<'static, str> {
        "Install a plugin or mod from Modrinth, CurseForge or SpigotMC onto this server (into plugins/ or mods/). Picks the newest stable build for the server unless a version id is given (SpigotMC: always its latest build). Not for modpacks (use install_modpack). Restart the server afterwards for it to load.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project": {"type": "string", "description": "Project id from search_content."},
                "version": {"type": "string", "description": "Optional version id from content_versions."},
                "source": {"type": "string", "enum": SOURCES, "description": "The source search_content found the project on."},
            },
            "required": ["project"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        // built from ids, never from a name the model supplies: text the model read (search
        // results, files, the console) could make the approval card say anything
        let source = source(arguments).map_or("?", source_name);
        let version = arguments
            .get("version")
            .and_then(Value::as_str)
            .filter(|version| !version.trim().is_empty())
            .map(|version| format!(" (version {})", support::short(version, 40)))
            .unwrap_or_default();

        format!(
            "Install {source} project {}{version}",
            support::short(support::shown(arguments, "project"), 40)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let software = software(context).await?;
        loader(&software)?;
        let project = project(arguments)?;
        let source = source(arguments)?;
        let version = support::string(arguments, "version", 64, false)?.trim();
        let version = (!version.is_empty()).then_some(version);

        let ratelimit = context
            .state
            .settings
            .get_as(|settings| settings.ratelimits.client_servers_files_pull)
            .await?;
        if context
            .state
            .cache
            .ratelimit(
                "client/servers/files/pull",
                ratelimit.hits,
                ratelimit.window_seconds,
                server.uuid.to_string(),
            )
            .await
            .is_err()
        {
            return Err(ToolError::invalid(
                "Too many downloads were started on this server just now; wait a minute and try again.",
            ));
        }

        let game_version = software.game_version(context.state).await;
        let installed = crate::minecraft::content::install(
            context.state,
            server,
            &software,
            game_version.as_deref(),
            source,
            &project,
            version,
        )
        .await
        .map_err(content_error)?;
        context
            .log(
                "content.install",
                json!({
                    "source": source,
                    "project": project,
                    "version": installed.version.id,
                    "file": format!("{}/{}", installed.directory, installed.file),
                }),
            )
            .await;

        let mut lines = vec![format!(
            "Downloading {} ({}) into /{}. The node downloads it in the background; it loads after a restart.",
            installed.file,
            if installed.version.number.is_empty() {
                installed.version.name.as_str()
            } else {
                installed.version.number.as_str()
            },
            installed.directory
        )];
        if installed.loose {
            lines.push(
                "No build lists this exact Minecraft version, so the newest build was used; it may not load."
                    .into(),
            );
        }
        if !installed.dependencies.is_empty() {
            lines.push(format!(
                "It requires these other projects (install them too, from {}): {}.",
                source_name(source),
                installed.dependencies.join(", ")
            ));
        }

        Ok(ToolResult::ok(lines.join("\n")).with_display(format!("Installing {}", installed.file)))
    }
}

pub struct ListInstalledContentTool;

#[async_trait::async_trait]
impl Tool for ListInstalledContentTool {
    fn name(&self) -> &'static str {
        "list_installed_content"
    }

    fn description(&self) -> Cow<'static, str> {
        "List the plugin/mod jars installed on this server (disabled ones end in .jar.disabled), with what each is, its version, available updates and missing dependencies where known. Set identify to wait until more of them are matched to their projects (slower).".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "identify": {"type": "boolean", "description": "Wait for more jars to be identified (slower)."},
            },
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "List installed plugins/mods".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let software = software(context).await?;
        let loader = loader(&software)?;
        let game_version = software.game_version(context.state).await;
        let rounds = if support::boolean(arguments, "identify", false) {
            IDENTIFY_ROUNDS
        } else {
            1
        };

        // each listing identifies a batch of the jars not identified yet
        let mut listed = (Vec::new(), 0);
        for _ in 0..rounds {
            listed = crate::minecraft::jars::list(
                context.state,
                server,
                loader,
                game_version.as_deref(),
            )
            .await?;
            if listed.1 == 0 {
                break;
            }
        }
        let (jars, pending) = listed;

        let directory = loader.directory();
        if jars.is_empty() {
            return Ok(
                ToolResult::ok(format!("Nothing installed in /{directory}."))
                    .with_display("0 installed"),
            );
        }

        let mut lines = vec![format!("/{directory}:")];
        lines.extend(jars.iter().map(jar_line));
        if pending > 0 {
            lines.push(format!(
                "{pending} not identified yet (call again with identify to wait for them)."
            ));
        }

        Ok(ToolResult::ok(support::limit_output(&lines.join("\n")))
            .with_display(format!("{} installed", jars.len())))
    }
}

pub struct RemoveContentTool;

/// The jar names to remove: 1 to [`MAX_REMOVE`] distinct ones.
fn jar_names(arguments: &Value) -> Result<Vec<String>, ToolError> {
    let mut names: Vec<String> = Vec::new();
    for name in support::strings(arguments, "files", MAX_REMOVE)? {
        let name = jar_name(Some(&Value::String(name)))?;
        if !names.contains(&name) {
            names.push(name);
        }
    }

    Ok(names)
}

#[async_trait::async_trait]
impl Tool for RemoveContentTool {
    fn name(&self) -> &'static str {
        "remove_content"
    }

    fn description(&self) -> Cow<'static, str> {
        "Uninstall plugins/mods by deleting their jars (names from list_installed_content). Their config folders are left alone. Consider toggle_content to only disable one.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "files": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_REMOVE, "description": "Jar file names, e.g. [\"EssentialsX-2.21.0.jar\"]."},
            },
            "required": ["files"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let files: Vec<String> = arguments
            .get("files")
            .and_then(Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .map(|file| support::short(file.as_str().unwrap_or("?"), 60))
                    .collect()
            })
            .unwrap_or_default();
        let more = files.len().saturating_sub(3);
        let shown = files.into_iter().take(3).collect::<Vec<_>>().join(", ");

        if more > 0 {
            format!("Uninstall {shown} and {more} more")
        } else {
            format!("Uninstall {shown}")
        }
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let names = jar_names(arguments)?;
        let software = software(context).await?;
        let loader = loader(&software)?;

        let mut removed = Vec::new();
        let mut missing = Vec::new();
        for name in &names {
            match crate::minecraft::jars::delete(context.state, server, loader, name, false).await {
                Ok(_) => removed.push(name.clone()),
                Err(JarError::NotInstalled) => missing.push(name.clone()),
                Err(err) => {
                    if !removed.is_empty() {
                        context
                            .log("content.remove", json!({"files": removed}))
                            .await;
                    }
                    return Err(jar_error(err));
                }
            }
        }

        if removed.is_empty() {
            return Err(ToolError::invalid(format!(
                "None of these is installed: {}. Use names from list_installed_content.",
                missing.join(", ")
            )));
        }
        context
            .log("content.remove", json!({"files": removed}))
            .await;

        let mut content = format!(
            "Removed from /{}: {}. Restart the server to unload them.",
            loader.directory(),
            removed.join(", ")
        );
        if !missing.is_empty() {
            content.push_str(&format!(" Not installed: {}.", missing.join(", ")));
        }

        Ok(ToolResult::ok(content).with_display(format!("Removed {} jar(s)", removed.len())))
    }
}

pub struct ToggleContentTool;

#[async_trait::async_trait]
impl Tool for ToggleContentTool {
    fn name(&self) -> &'static str {
        "toggle_content"
    }

    fn description(&self) -> Cow<'static, str> {
        "Disable a plugin/mod (renames it to .jar.disabled so it stops loading) or enable it again. Takes effect after a restart.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file": {"type": "string", "description": "Jar name from list_installed_content."},
                "enabled": {"type": "boolean"},
            },
            "required": ["file", "enabled"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "{} {}",
            if support::boolean(arguments, "enabled", false) {
                "Enable"
            } else {
                "Disable"
            },
            support::short(support::shown(arguments, "file"), 60)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let file = jar_name(arguments.get("file"))?;
        let Some(enabled) = arguments.get("enabled").and_then(Value::as_bool) else {
            return Err(ToolError::invalid("\"enabled\" must be true or false."));
        };
        let software = software(context).await?;
        let loader = loader(&software)?;

        let renamed = crate::minecraft::jars::toggle(context.state, server, loader, &file, enabled)
            .await
            .map_err(jar_error)?;
        if renamed.as_str() == file {
            return Ok(ToolResult::ok(format!(
                "{file} is already {}.",
                if enabled { "enabled" } else { "disabled" }
            )));
        }

        context
            .log(
                "content.toggle",
                json!({"file": format!("{}/{renamed}", loader.directory()), "enabled": enabled}),
            )
            .await;

        let base = crate::minecraft::jars::base_name(&renamed).to_string();
        Ok(ToolResult::ok(format!(
            "Renamed {file} to {renamed}. Restart the server to apply."
        ))
        .with_display(format!(
            "{} {base}",
            if enabled { "Enabled" } else { "Disabled" }
        )))
    }
}

pub struct InstallGeyserTool;

#[async_trait::async_trait]
impl Tool for InstallGeyserTool {
    fn name(&self) -> &'static str {
        "install_geyser"
    }

    fn description(&self) -> Cow<'static, str> {
        "Install or update Geyser + Floodgate so Bedrock (phone/console) players can join. Supported on paper (and its forks), fabric and neoforge, not forge. Opens a Bedrock port and restarts the server when it is running.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["minecraft.bedrock"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Install Geyser + Floodgate (restarts the server)".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        software(context).await?;

        let installed = match crate::minecraft::bedrock::install(context.state, server).await {
            Ok(installed) => installed,
            Err(BedrockError::Unsupported) => {
                return Err(ToolError::invalid(
                    "Geyser is not available for this server software (it works on paper and its forks, fabric and neoforge).",
                ));
            }
            Err(BedrockError::NoPort) => {
                return Err(ToolError::invalid(
                    "This server's node has no free port left for Bedrock.",
                ));
            }
            Err(BedrockError::Wings(err)) => {
                tracing::warn!("neo: minecraft bedrock: {err:#}");
                return Err(ToolError::invalid(
                    "The server's files could not be reached; try again in a moment.",
                ));
            }
            Err(BedrockError::Content(err)) => return Err(content_error(err)),
            Err(BedrockError::Other(err)) => return Err(ToolError::Failed(err)),
        };
        context
            .log(
                "geyser",
                json!({"updated": installed.updated, "floodgate": installed.floodgate, "port": installed.port}),
            )
            .await;

        Ok(ToolResult::ok(format!(
            "{} Geyser{}{}. Bedrock players join {}:{}.",
            if installed.updated {
                "Updated"
            } else {
                "Installed"
            },
            if installed.floodgate {
                " and Floodgate"
            } else {
                ""
            },
            if installed.restarted {
                " and restarted the server"
            } else {
                "; it loads at the next start"
            },
            installed.address,
            installed.port
        ))
        .with_display("Geyser installed"))
    }
}

pub struct InstallModpackTool;

#[async_trait::async_trait]
impl Tool for InstallModpackTool {
    fn name(&self) -> &'static str {
        "install_modpack"
    }

    fn description(&self) -> Cow<'static, str> {
        "Install a modpack (from search_content with kind \"modpack\" and a version id from content_versions with kind \"modpack\"). The server must be stopped. It is switched to the pack's loader and version and reinstalled, then the pack's files are placed; this runs in the background and shows on the server's Minecraft content page. Existing mods stay unless clean is set (they go to the trash when the server has one).".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project": {"type": "string"},
                "version": {"type": "string"},
                "source": {"type": "string", "enum": ["modrinth", "curseforge"]},
                "clean": {"type": "boolean", "description": "Remove the mods installed now first."},
            },
            "required": ["project", "version"],
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn offered_with(&self) -> Vec<&'static str> {
        vec!["settings.software"]
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        // a pack brings its own loader and version, which reinstalls the server
        vec!["minecraft.content", "settings.software"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        // built from ids, never from a name the model supplies
        format!(
            "Install {} modpack {} (version {}, reinstall{})",
            source(arguments).map_or("?", source_name),
            support::short(support::shown(arguments, "project"), 40),
            support::short(support::shown(arguments, "version"), 40),
            if support::boolean(arguments, "clean", false) {
                ", removing current mods"
            } else {
                ""
            }
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        software(context).await?;
        let project = project(arguments)?;
        let version = support::string(arguments, "version", 64, true)?
            .trim()
            .to_string();
        let source = source(arguments)?;
        let clean = support::boolean(arguments, "clean", false);

        let start = crate::minecraft::modpacks::start(
            context.state,
            server,
            source,
            &project,
            &version,
            clean,
        )
        .await
        .map_err(modpack_error)?;
        let name = start.install.name.clone();
        context
            .log(
                "modpack",
                json!({"source": source, "project": project, "version": version, "name": name, "clean": clean}),
            )
            .await;

        // a reinstall plus hundreds of downloads outlast the chat's turn; the Content page shows
        // the progress
        let state = context.state.clone();
        let server = server.clone();
        let user_uuid = context.user.uuid;
        tokio::spawn(async move {
            let (uuid, name) = (server.uuid, server.name.clone());
            let install = crate::minecraft::modpacks::install(&state, server, start).await;
            crate::notifications::modpack_ended(&state, user_uuid, uuid, &name, &install).await;
            tracing::info!(
                server = %uuid,
                step = ?install.step,
                files = install.files,
                skipped = install.skipped.len(),
                error = ?install.error,
                "neo's modpack install ended"
            );
        });

        Ok(ToolResult::ok(format!(
            "The install of {name} has started in the background: the server is switched to the pack's loader and version and reinstalled, then the files are placed. Its progress shows on the server's Minecraft content page."
        ))
        .with_display("Modpack install started"))
    }
}

pub struct SwitchSoftwareTool;

#[async_trait::async_trait]
impl Tool for SwitchSoftwareTool {
    fn name(&self) -> &'static str {
        "switch_server_software"
    }

    fn description(&self) -> Cow<'static, str> {
        "Change the Minecraft software (one of the switchable_software values from minecraft_info) and/or the Minecraft version. The server is reinstalled: the world and configs are kept, but installed plugins/mods stay and may no longer be compatible.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "software": {"type": "string", "description": "A switchable_software value from minecraft_info; omit to keep the current one."},
                "version": {"type": "string", "description": "\"latest\" or an exact version like \"1.21.4\"; omit to keep the current one."},
            },
        })
    }

    fn available_for(&self, context: &ToolContext<'_>) -> bool {
        is_minecraft(context)
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["settings.software"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let part = |key: &str| {
            arguments
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| support::short(value, 30))
        };

        let target = [part("software"), part("version")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");

        format!(
            "Switch to {} (reinstall)",
            if target.is_empty() { "?" } else { &target }
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let before = software(context).await?;
        let variant = support::string(arguments, "software", 64, false)?
            .trim()
            .to_ascii_lowercase();
        let version = support::string(arguments, "version", 64, false)?
            .trim()
            .to_string();
        if variant.is_empty() && version.is_empty() {
            return Err(ToolError::invalid("Give a software, a version, or both."));
        }

        let from = format!(
            "{} {}",
            before.value.as_deref().unwrap_or("the current software"),
            before.version
        );
        let to = format!(
            "{} {}",
            if variant.is_empty() {
                before.value.as_deref().unwrap_or("the current software")
            } else {
                variant.as_str()
            },
            if version.is_empty() {
                before.version.as_str()
            } else {
                version.as_str()
            }
        );

        let mut server = context.server()?.clone();
        let state = context.state.clone();
        let (switch_variant, switch_version) = (variant.clone(), version.clone());
        // spawned: the variables and the reinstall go together even when the chat goes away
        let switched = tokio::spawn(async move {
            crate::server_software::switch(
                &state,
                &mut server,
                (!switch_variant.is_empty()).then_some(switch_variant.as_str()),
                (!switch_version.is_empty()).then_some(switch_version.as_str()),
            )
            .await
        })
        .await
        .map_err(anyhow::Error::from)?;

        match switched {
            Ok(()) => {}
            Err(crate::server_software::SoftwareError::Other(err)) => {
                return Err(ToolError::Failed(err));
            }
            Err(crate::server_software::SoftwareError::UnknownVersion) => {
                return Err(ToolError::invalid(format!(
                    "That Minecraft version is not offered for {}; check minecraft_info for installable versions.",
                    to.split(' ').next().unwrap_or("this software")
                )));
            }
            Err(err) => return Err(ToolError::invalid(support::sentence(&err.message()))),
        }

        context
            .log(
                "software",
                json!({"from": from, "to": to, "variant": variant, "version": version}),
            )
            .await;

        Ok(ToolResult::ok(format!(
            "Switching from {from} to {to}: the server is reinstalling now (a few minutes). Other tools will say the server is installing until it finishes."
        ))
        .with_display(format!("Reinstalling as {to}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sources_and_kinds() {
        assert_eq!(source(&json!({})).unwrap(), Source::Modrinth);
        assert_eq!(
            source(&json!({"source": "CurseForge"})).unwrap(),
            Source::CurseForge
        );
        assert_eq!(
            source(&json!({"source": "spigot"})).unwrap(),
            Source::SpigotMc
        );
        assert!(source(&json!({"source": "hangar"})).is_err());

        assert_eq!(kind(&json!({})).unwrap(), None);
        assert_eq!(
            kind(&json!({"kind": "modpack"})).unwrap(),
            Some(Kind::Modpack)
        );
        assert!(kind(&json!({"kind": "datapack"})).is_err());
    }

    #[test]
    fn jar_names_are_plain() {
        assert_eq!(
            jar_name(Some(&json!(" EssentialsX-2.21.0.jar "))).unwrap(),
            "EssentialsX-2.21.0.jar"
        );
        assert!(jar_name(Some(&json!("Foo.JAR.disabled"))).is_ok());
        for bad in [
            "../x.jar",
            "plugins/x.jar",
            "a\\b.jar",
            "x.zip",
            "",
            "..jar",
        ] {
            assert!(jar_name(Some(&json!(bad))).is_err(), "{bad}");
        }
        assert!(jar_name(None).is_err());

        assert_eq!(
            jar_names(&json!({"files": ["a.jar", "a.jar", "b.jar"]})).unwrap(),
            vec!["a.jar", "b.jar"]
        );
        assert!(jar_names(&json!({"files": ["a.jar", "../b.jar"]})).is_err());
    }

    #[test]
    fn versions_are_listed_briefly() {
        let version = Version {
            id: "abc".into(),
            project_id: "p".into(),
            name: "Release".into(),
            number: "2.1.0".into(),
            channel: "release".into(),
            date: "2026-10-01T12:00:00Z".into(),
            game_versions: ["1.20", "1.20.1", "1.21", "1.21.1", "1.21.4"]
                .into_iter()
                .map(Into::into)
                .collect(),
            loaders: Vec::new(),
            file: None,
            dependencies: Vec::new(),
        };

        assert_eq!(
            version_line(&version),
            "- version id abc: 2.1.0 Release [release] for 1.20.1, 1.21, 1.21.1, 1.21.4 (2026-10-01)"
        );
    }

    #[test]
    fn summaries_use_ids() {
        assert_eq!(
            InstallContentTool.summarize(&json!({"project": "P7dR8mSH", "version": "v1"})),
            "Install modrinth project P7dR8mSH (version v1)"
        );
        assert_eq!(
            SwitchSoftwareTool.summarize(&json!({"version": "1.21.4"})),
            "Switch to 1.21.4 (reinstall)"
        );
        assert_eq!(
            RemoveContentTool.summarize(&json!({"files": ["a.jar", "b.jar", "c.jar", "d.jar"]})),
            "Uninstall a.jar, b.jar, c.jar and 1 more"
        );
        assert_eq!(
            InstallModpackTool.summarize(
                &json!({"project": "1KVo5zza", "version": "x", "source": "modrinth", "clean": true})
            ),
            "Install modrinth modpack 1KVo5zza (version x, reinstall, removing current mods)"
        );
    }
}
