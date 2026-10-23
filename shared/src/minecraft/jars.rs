//! plugged.host: the plugins or mods installed on a server, identified once and remembered
//! (docs/features/minecraft.md).
//!
//! A jar is identified in this order: its SHA-1 (Wings hashes it) at Modrinth, its CurseForge
//! fingerprint (Wings again) at CurseForge, then its own descriptor's name (`plugin.yml`,
//! `fabric.mod.json`, `mods.toml`, read inside the jar through Wings) matched exactly against
//! Modrinth and SpigotMC. The answer is kept 30 days per server, file name and size; the list
//! identifies a few jars per request until none is left.

use super::{Kind, Loader, Source};
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::io::AsyncReadExt;
use utoipa::ToSchema;

/// Jars identified per request.
pub const BATCH: usize = 8;
const IDENTITY_TTL: u64 = 30 * 24 * 60 * 60;
/// Descriptors bigger than this are not read.
const DESCRIPTOR_LIMIT: u64 = 256 * 1024;
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// What a jar's own descriptor says.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct JarMeta {
    pub kind: Option<Kind>,
    pub id: Option<compact_str::CompactString>,
    pub name: Option<compact_str::CompactString>,
    pub version: Option<compact_str::CompactString>,
    pub authors: Vec<compact_str::CompactString>,
    pub description: Option<compact_str::CompactString>,
    pub website: Option<compact_str::CompactString>,
    /// Plugins or mods it needs (required only).
    pub depends: Vec<compact_str::CompactString>,
}

/// How sure the identification is.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// The exact file, by its SHA-1 at Modrinth.
    Hash,
    /// The exact file, by its CurseForge fingerprint.
    Fingerprint,
    /// A project whose name is exactly the jar's.
    Name,
    /// Only the jar's own descriptor.
    Metadata,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Identity {
    sha1: Option<compact_str::CompactString>,
    meta: JarMeta,
    source: Option<Source>,
    project_id: Option<compact_str::CompactString>,
    title: Option<compact_str::CompactString>,
    icon: Option<compact_str::CompactString>,
    url: Option<compact_str::CompactString>,
    version_id: Option<compact_str::CompactString>,
    version_number: Option<compact_str::CompactString>,
    confidence: Confidence,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct InstalledUpdate {
    /// The Modrinth version to update to.
    pub id: compact_str::CompactString,
    pub number: compact_str::CompactString,
}

/// One jar of the list.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct InstalledJar {
    pub file: compact_str::CompactString,
    pub disabled: bool,
    pub size: u64,
    pub modified: chrono::DateTime<chrono::Utc>,
    /// Not identified yet.
    pub pending: bool,
    pub name: compact_str::CompactString,
    pub version: Option<compact_str::CompactString>,
    pub authors: Vec<compact_str::CompactString>,
    pub description: Option<compact_str::CompactString>,
    pub kind: Option<Kind>,
    pub source: Option<Source>,
    pub project_id: Option<compact_str::CompactString>,
    pub url: Option<compact_str::CompactString>,
    pub icon: Option<compact_str::CompactString>,
    pub confidence: Confidence,
    /// What its descriptor says it needs.
    pub depends: Vec<compact_str::CompactString>,
    /// What it needs that is not installed and enabled (plugins only).
    pub missing: Vec<compact_str::CompactString>,
    /// The plugin's own folder next to it, deleted with it on request.
    pub data_folder: Option<compact_str::CompactString>,
    pub update: Option<InstalledUpdate>,
}

#[derive(Clone, Debug)]
struct JarEntry {
    name: compact_str::CompactString,
    disabled: bool,
    size: u64,
    modified: chrono::DateTime<chrono::Utc>,
}

/// The jar's name without `.disabled`.
pub fn base_name(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".disabled") {
        &name[..name.len() - ".disabled".len()]
    } else {
        name
    }
}

/// Letters and digits only, lower case: how names are compared.
pub fn comparable(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A jar's file name as words: `EssentialsX-2.21.0.jar` → `essentials x`.
pub fn normalize_name(name: &str) -> String {
    static SUFFIX: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)[-_+ ](v?\d+[\w.+-]*|mc\d.*|b\d+|fabric|forge|neoforge|spigot|paper|bukkit|purpur)$",
        )
        .unwrap()
    });
    static CAMEL: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"([a-z])([A-Z])").unwrap());

    let lower = name.to_ascii_lowercase();
    let mut name = name.to_string();
    for suffix in [".jar.disabled", ".jar"] {
        if lower.ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    for _ in 0..2 {
        name = SUFFIX.replace(&name, "").into_owned();
    }
    let name = CAMEL.replace_all(&name, "$1 $2");

    name.to_ascii_lowercase()
        .replace(['-', '_', '+', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn identity_key(server: &Server, jar: &JarEntry) -> String {
    use sha2::Digest;

    let digest = sha2::Sha256::digest(format!("{}|{}", base_name(&jar.name), jar.size));
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }

    format!("minecraft::jar::{}::{hex}", server.uuid)
}

async fn client(
    state: &crate::State,
    server: &Server,
) -> Result<wings_api::client::WingsClient, anyhow::Error> {
    Ok(server
        .node
        .fetch_cached(&state.database)
        .await?
        .api_client(&state.database)
        .await?
        .ignoring(server.subuser_ignored_files.clone().unwrap_or_default()))
}

/// The jars and folders of the content folder.
async fn listing(
    client: &wings_api::client::WingsClient,
    server: &Server,
    directory: &str,
) -> (Vec<JarEntry>, Vec<compact_str::CompactString>) {
    let Ok(list) = client
        .get_servers_server_files_list(
            server.uuid,
            &wings_api::servers_server_files_list::get::Query {
                directory: Some(format!("/{directory}").into()),
                per_page: Some(1000),
                ..Default::default()
            },
        )
        .await
    else {
        return (Vec::new(), Vec::new());
    };

    let mut jars = Vec::new();
    let mut folders = Vec::new();
    for entry in list.entries {
        let lower = entry.name.to_ascii_lowercase();

        if entry.directory {
            folders.push(entry.name);
        } else if entry.file && (lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
            jars.push(JarEntry {
                disabled: lower.ends_with(".disabled"),
                size: entry.size,
                modified: entry.modified.with_timezone(&chrono::Utc),
                name: entry.name,
            });
        }
    }
    jars.sort_by_key(|jar| jar.name.to_ascii_lowercase());

    (jars, folders)
}

/// The installed jars, identifying up to [`BATCH`] of those not identified yet; the second value
/// is how many are left.
pub async fn list(
    state: &crate::State,
    server: &Server,
    loader: Loader,
    game_version: Option<&str>,
) -> Result<(Vec<InstalledJar>, usize), anyhow::Error> {
    let client = client(state, server).await?;
    let directory = loader.directory();
    let (jars, folders) = listing(&client, server, directory).await;

    let mut identities: Vec<Option<Identity>> = Vec::with_capacity(jars.len());
    for jar in &jars {
        identities.push(
            state
                .cache
                .get::<Identity>(&identity_key(server, jar))
                .await
                .ok()
                .flatten(),
        );
    }

    let pending: Vec<usize> = identities
        .iter()
        .enumerate()
        .filter(|(_, identity)| identity.is_none())
        .map(|(i, _)| i)
        .collect();
    let batch: Vec<usize> = pending.iter().copied().take(BATCH).collect();
    if !batch.is_empty() {
        let found = identify(
            state,
            &client,
            server,
            loader,
            directory,
            &batch.iter().map(|i| jars[*i].clone()).collect::<Vec<_>>(),
        )
        .await;

        for (i, identity) in batch.iter().zip(found) {
            let _ = state
                .cache
                .set(&identity_key(server, &jars[*i]), IDENTITY_TTL, &identity)
                .await;
            identities[*i] = Some(identity);
        }
    }
    let left = pending.len() - batch.len();

    let mut rows: Vec<InstalledJar> = jars
        .iter()
        .zip(&identities)
        .map(|(jar, identity)| row(jar, identity.as_ref(), &folders))
        .collect();

    if left == 0 {
        mark_missing(&mut rows);
    }
    mark_updates(state, &mut rows, &identities, loader, game_version).await;

    Ok((rows, left))
}

fn row(
    jar: &JarEntry,
    identity: Option<&Identity>,
    folders: &[compact_str::CompactString],
) -> InstalledJar {
    let meta = identity.map(|i| i.meta.clone()).unwrap_or_default();
    let name = meta
        .name
        .clone()
        .or_else(|| identity.and_then(|i| i.title.clone()))
        .unwrap_or_else(|| {
            normalize_name(&jar.name)
                .split(' ')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
                .into()
        });
    let data_folder = (meta.kind == Some(Kind::Plugin))
        .then(|| {
            let plugin = meta.name.as_deref()?;
            folders
                .iter()
                .find(|folder| folder.eq_ignore_ascii_case(plugin))
                .cloned()
        })
        .flatten();
    let url = identity
        .and_then(|i| i.url.clone())
        .or_else(|| meta.website.clone())
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));

    InstalledJar {
        file: jar.name.clone(),
        disabled: jar.disabled,
        size: jar.size,
        modified: jar.modified,
        pending: identity.is_none(),
        name,
        version: meta
            .version
            .clone()
            .or_else(|| identity.and_then(|i| i.version_number.clone())),
        authors: meta.authors.clone(),
        description: meta.description.clone(),
        kind: meta.kind,
        source: identity.and_then(|i| i.source),
        project_id: identity.and_then(|i| i.project_id.clone()),
        url,
        icon: identity.and_then(|i| i.icon.clone()),
        confidence: identity.map_or(Confidence::Unknown, |i| i.confidence),
        depends: meta.depends.clone(),
        missing: Vec::new(),
        data_folder,
        update: None,
    }
}

/// Plugins whose required plugins are not installed and enabled.
fn mark_missing(rows: &mut [InstalledJar]) {
    let present: HashSet<compact_str::CompactString> = rows
        .iter()
        .filter(|row| !row.disabled && row.kind == Some(Kind::Plugin))
        .map(|row| row.name.to_lowercase())
        .collect();

    for row in rows.iter_mut() {
        if row.kind == Some(Kind::Plugin) && !row.disabled {
            row.missing = row
                .depends
                .iter()
                .filter(|dependency| !present.contains(&dependency.to_lowercase()))
                .cloned()
                .collect();
        }
    }
}

async fn mark_updates(
    state: &crate::State,
    rows: &mut [InstalledJar],
    identities: &[Option<Identity>],
    loader: Loader,
    game_version: Option<&str>,
) {
    let hashes: Vec<compact_str::CompactString> = identities
        .iter()
        .flatten()
        .filter(|i| i.source == Some(Source::Modrinth) && i.confidence == Confidence::Hash)
        .filter_map(|i| i.sha1.clone())
        .collect();
    if hashes.is_empty() {
        return;
    }

    let Ok(latest) =
        super::modrinth::latest_by_hashes(state, &hashes, loader.modrinth_loaders(), game_version)
            .await
    else {
        return;
    };

    for (row, identity) in rows.iter_mut().zip(identities) {
        let Some(identity) = identity else { continue };
        let (Some(sha1), Some(current)) = (&identity.sha1, &identity.version_id) else {
            continue;
        };
        if let Some(version) = latest.get(sha1)
            && version.id != *current
        {
            row.update = Some(InstalledUpdate {
                id: version.id.clone(),
                number: if version.number.is_empty() {
                    version.name.clone()
                } else {
                    version.number.clone()
                },
            });
        }
    }
}

async fn identify(
    state: &crate::State,
    client: &wings_api::client::WingsClient,
    server: &Server,
    loader: Loader,
    directory: &str,
    jars: &[JarEntry],
) -> Vec<Identity> {
    let paths: Vec<compact_str::CompactString> = jars
        .iter()
        .map(|jar| compact_str::format_compact!("{directory}/{}", jar.name))
        .collect();

    let sha1 = fingerprints(client, server, &paths, wings_api::Algorithm::Sha1).await;
    let metas =
        futures_util::future::join_all(jars.iter().zip(&paths).map(|(jar, path)| async move {
            if jar.disabled {
                JarMeta::default()
            } else {
                read_meta(client, server, path, loader).await
            }
        }))
        .await;

    let mut identities: Vec<Identity> = paths
        .iter()
        .zip(metas)
        .map(|(path, meta)| Identity {
            sha1: sha1.get(path).cloned(),
            meta,
            source: None,
            project_id: None,
            title: None,
            icon: None,
            url: None,
            version_id: None,
            version_number: None,
            confidence: Confidence::Unknown,
        })
        .collect();

    // 1. Modrinth by SHA-1
    let hashes: Vec<compact_str::CompactString> =
        identities.iter().filter_map(|i| i.sha1.clone()).collect();
    let versions = super::modrinth::versions_by_hashes(state, &hashes)
        .await
        .unwrap_or_default();
    let project_ids: Vec<compact_str::CompactString> =
        versions.values().map(|v| v.project_id.clone()).collect();
    let projects = super::modrinth::projects_by_ids(state, &project_ids)
        .await
        .unwrap_or_default();

    for identity in &mut identities {
        let Some(version) = identity.sha1.as_ref().and_then(|h| versions.get(h)) else {
            continue;
        };
        let Some(project) = projects.get(&version.project_id) else {
            continue;
        };

        identity.source = Some(Source::Modrinth);
        identity.project_id = Some(project.id.clone());
        identity.title = Some(project.title.clone());
        identity.icon = project.icon.clone();
        identity.url = Some(project.url.clone());
        identity.version_id = Some(version.id.clone());
        identity.version_number = Some(version.number.clone());
        identity.confidence = Confidence::Hash;
    }

    // 2. CurseForge by fingerprint, for what Modrinth did not know
    if let Some(key) = super::curseforge::key(state).await {
        let unknown: Vec<compact_str::CompactString> = paths
            .iter()
            .zip(&identities)
            .filter(|(_, identity)| identity.source.is_none())
            .map(|(path, _)| path.clone())
            .collect();

        if !unknown.is_empty() {
            let prints =
                fingerprints(client, server, &unknown, wings_api::Algorithm::Curseforge).await;
            let numbers: Vec<u32> = prints.values().filter_map(|p| p.parse().ok()).collect();
            let matches = super::curseforge::fingerprints(state, &key, &numbers)
                .await
                .unwrap_or_default();

            for found in matches {
                let Some(path) = prints
                    .iter()
                    .find(|(_, print)| print.parse::<u32>().ok() == Some(found.fingerprint))
                    .map(|(path, _)| path)
                else {
                    continue;
                };
                let Some(i) = paths.iter().position(|p| p == path) else {
                    continue;
                };
                let Ok(Some(project)) =
                    super::curseforge::project_by_id(state, &key, found.mod_id).await
                else {
                    continue;
                };

                let identity = &mut identities[i];
                identity.source = Some(Source::CurseForge);
                identity.project_id = Some(project.id.clone());
                identity.title = Some(project.title.clone());
                identity.icon = project.icon.clone();
                identity.url = Some(project.url.clone()).filter(|url| !url.is_empty());
                identity.version_number = Some(found.display_name);
                identity.confidence = Confidence::Fingerprint;
            }
        }
    }

    // 3. by name, strictly; 4. the descriptor alone
    for (jar, identity) in jars.iter().zip(&mut identities) {
        if identity.source.is_some() {
            continue;
        }

        let name = identity.meta.name.clone();
        let query = name
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| normalize_name(&jar.name));
        let wanted = comparable(&query);
        identity.confidence = if name.is_some() {
            Confidence::Metadata
        } else {
            Confidence::Unknown
        };
        if wanted.is_empty() {
            continue;
        }

        let kind = if loader == Loader::Paper {
            Kind::Plugin
        } else {
            Kind::Mod
        };
        let hits = super::modrinth::search(state, kind, loader.modrinth_loaders(), &query, None, 1)
            .await
            .map(|page| page.hits)
            .unwrap_or_default();
        let mut found = hits
            .into_iter()
            .find(|hit| comparable(&hit.title) == wanted);

        if found.is_none()
            && name.is_some()
            && identity.meta.kind == Some(Kind::Plugin)
            && super::spigot::enabled(state).await
        {
            found = super::spigot::find_by_name(state, &query).await;
        }

        if let Some(project) = found {
            identity.source = Some(project.source);
            identity.project_id = Some(project.id.clone());
            identity.title = Some(project.title.clone());
            identity.icon = project.icon.clone();
            identity.url = Some(project.url.clone());
            identity.confidence = Confidence::Name;
        }
    }

    identities
}

/// Fingerprints of these files, by path; files Wings could not read are left out.
async fn fingerprints(
    client: &wings_api::client::WingsClient,
    server: &Server,
    paths: &[compact_str::CompactString],
    algorithm: wings_api::Algorithm,
) -> HashMap<compact_str::CompactString, compact_str::CompactString> {
    if paths.is_empty() {
        return HashMap::new();
    }

    client
        .get_servers_server_files_fingerprints(
            server.uuid,
            &wings_api::servers_server_files_fingerprints::get::Query {
                algorithm: Some(algorithm),
                files: Some(paths.to_vec()),
                ..Default::default()
            },
        )
        .await
        .map(|response| response.fingerprints.into_iter().collect())
        .unwrap_or_default()
}

/// A file's contents, when it exists and is small enough.
async fn read(
    client: &wings_api::client::WingsClient,
    server: &Server,
    path: &str,
    limit: u64,
) -> Option<Vec<u8>> {
    tokio::time::timeout(READ_TIMEOUT, async {
        let reader = client
            .get_servers_server_files_contents(
                server.uuid,
                &wings_api::servers_server_files_contents::get::Query {
                    file: Some(format!("/{path}").into()),
                    max_size: Some(limit),
                    ..Default::default()
                },
            )
            .await
            .ok()?;

        let mut content = Vec::new();
        reader.take(limit).read_to_end(&mut content).await.ok()?;
        Some(content)
    })
    .await
    .ok()
    .flatten()
}

/// The jar's descriptor, read inside the jar (Wings opens jars like zips). The descriptors the
/// loader uses are tried first.
async fn read_meta(
    client: &wings_api::client::WingsClient,
    server: &Server,
    jar: &str,
    loader: Loader,
) -> JarMeta {
    let order: &[&str] = match loader {
        Loader::Paper => &["paper-plugin.yml", "plugin.yml"],
        Loader::Fabric | Loader::Quilt => &["fabric.mod.json", "quilt.mod.json"],
        Loader::Forge | Loader::NeoForge => &["META-INF/neoforge.mods.toml", "META-INF/mods.toml"],
    };

    for file in order {
        let Some(raw) = read(client, server, &format!("{jar}/{file}"), DESCRIPTOR_LIMIT).await
        else {
            continue;
        };
        let raw = String::from_utf8_lossy(&raw);

        let meta = if file.ends_with(".yml") {
            bukkit(&raw)
        } else if file.ends_with(".json") {
            fabric(&raw)
        } else {
            let manifest = read(
                client,
                server,
                &format!("{jar}/META-INF/MANIFEST.MF"),
                64 * 1024,
            )
            .await
            .map(|raw| String::from_utf8_lossy(&raw).into_owned());
            forge(&raw, manifest.as_deref().and_then(manifest_version))
        };

        if let Some(meta) = meta {
            return meta;
        }
    }

    JarMeta::default()
}

fn clip(value: &str, chars: usize) -> compact_str::CompactString {
    value.trim().chars().take(chars).collect::<String>().into()
}

/// A version as shown, `None` for a build placeholder (`${version}`).
fn version_of(value: Option<&serde_json::Value>) -> Option<compact_str::CompactString> {
    let value = match value? {
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };

    (!value.is_empty() && !value.contains("${")).then(|| clip(&value, 60))
}

fn strings(values: impl IntoIterator<Item = String>) -> Vec<compact_str::CompactString> {
    let mut out: Vec<compact_str::CompactString> = Vec::new();
    for value in values {
        let value = clip(&value, 80);
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    out.truncate(20);

    out
}

fn string_list(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(o) => {
                    o.get("name").and_then(|n| n.as_str()).map(str::to_string)
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `plugin.yml` / `paper-plugin.yml`.
pub fn bukkit(raw: &str) -> Option<JarMeta> {
    let data: serde_json::Value =
        serde_saphyr::from_str_with_options(raw, crate::utils::yaml_deserialize_options()).ok()?;
    let name = data.get("name")?.as_str()?;

    let mut authors = string_list(&data["authors"]);
    authors.extend(string_list(&data["author"]));
    let mut depends = string_list(&data["depend"]);
    // paper-plugin.yml: dependencies.server.<name>.required
    if let Some(server) = data["dependencies"]["server"].as_object() {
        for (dependency, details) in server {
            if details["required"].as_bool().unwrap_or(true) {
                depends.push(dependency.clone());
            }
        }
    }

    Some(JarMeta {
        kind: Some(Kind::Plugin),
        id: Some(clip(name, 80)),
        name: Some(clip(name, 80)),
        version: version_of(data.get("version")),
        authors: strings(authors),
        description: data["description"].as_str().map(|d| clip(d, 500)),
        website: data["website"].as_str().map(|w| clip(w, 255)),
        depends: strings(depends),
    })
}

/// `fabric.mod.json` / `quilt.mod.json`.
pub fn fabric(raw: &str) -> Option<JarMeta> {
    let data: serde_json::Value = serde_json::from_str(raw).ok()?;
    // quilt.mod.json keeps everything under quilt_loader
    let loader = if data["quilt_loader"].is_object() {
        &data["quilt_loader"]
    } else {
        &data
    };
    let info = if loader["metadata"].is_object() {
        &loader["metadata"]
    } else {
        &data
    };
    let id = loader["id"].as_str()?;

    let authors = if info["authors"].is_array() {
        string_list(&info["authors"])
    } else if let Some(contributors) = info["contributors"].as_object() {
        contributors.keys().cloned().collect()
    } else {
        string_list(&info["contributors"])
    };
    let depends: Vec<String> = match &loader["depends"] {
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(o) => {
                    o.get("id").and_then(|i| i.as_str()).map(str::to_string)
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    const BUILT_IN: &[&str] = &[
        "minecraft",
        "java",
        "fabricloader",
        "fabric-loader",
        "quilt_loader",
    ];

    Some(JarMeta {
        kind: Some(Kind::Mod),
        id: Some(clip(id, 80)),
        name: Some(clip(info["name"].as_str().unwrap_or(id), 80)),
        version: version_of(loader.get("version")),
        authors: strings(authors),
        description: info["description"].as_str().map(|d| clip(d, 500)),
        website: info["contact"]["homepage"].as_str().map(|w| clip(w, 255)),
        depends: strings(
            depends
                .into_iter()
                .filter(|d| !BUILT_IN.contains(&d.as_str())),
        ),
    })
}

/// `Implementation-Version` of a jar's manifest.
pub fn manifest_version(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim().eq_ignore_ascii_case("Implementation-Version") && !value.trim().is_empty())
            .then(|| value.trim().to_string())
    })
}

/// `META-INF/mods.toml` / `neoforge.mods.toml`: the first `[[mods]]` entry.
pub fn forge(raw: &str, manifest_version: Option<String>) -> Option<JarMeta> {
    let data: toml::Table = toml::from_str(raw).ok()?;
    let entry = data.get("mods")?.as_array()?.first()?.as_table()?;
    let text = |key: &str| entry.get(key).and_then(|v| v.as_str()).map(str::trim);
    let id = text("modId")?;

    let version = text("version")
        .filter(|v| !v.contains("${"))
        .map(str::to_string)
        .or(manifest_version);
    let mut depends = Vec::new();
    if let Some(dependencies) = data.get("dependencies").and_then(|d| d.as_table()) {
        for list in dependencies.values() {
            for dependency in list.as_array().into_iter().flatten() {
                let Some(dependency) = dependency.as_table() else {
                    continue;
                };
                let required = dependency.get("mandatory").and_then(|m| m.as_bool()) == Some(true)
                    || dependency.get("type").and_then(|t| t.as_str()) == Some("required");
                if required
                    && let Some(mod_id) = dependency.get("modId").and_then(|m| m.as_str())
                    && !["minecraft", "forge", "neoforge", "java"].contains(&mod_id)
                {
                    depends.push(mod_id.to_string());
                }
            }
        }
    }

    Some(JarMeta {
        kind: Some(Kind::Mod),
        id: Some(clip(id, 80)),
        name: Some(clip(text("displayName").unwrap_or(id), 80)),
        version: version.filter(|v| !v.is_empty()).map(|v| clip(&v, 60)),
        authors: strings(
            text("authors")
                .unwrap_or_default()
                .split(',')
                .map(str::to_string),
        ),
        description: text("description").map(|d| clip(d, 500)),
        website: text("displayURL").map(|w| clip(w, 255)),
        depends: strings(depends),
    })
}

async fn row_by_file(
    state: &crate::State,
    server: &Server,
    loader: Loader,
    file: &str,
) -> Result<Option<(InstalledJar, Option<Identity>)>, anyhow::Error> {
    let client = client(state, server).await?;
    let (jars, folders) = listing(&client, server, loader.directory()).await;
    let Some(jar) = jars.into_iter().find(|jar| jar.name == file) else {
        return Ok(None);
    };
    let identity = state
        .cache
        .get::<Identity>(&identity_key(server, &jar))
        .await
        .ok()
        .flatten();

    Ok(Some((row(&jar, identity.as_ref(), &folders), identity)))
}

#[derive(Debug)]
pub enum JarError {
    NotInstalled,
    NoUpdate,
    Wings(String),
    Other(anyhow::Error),
}

impl JarError {
    pub fn message(&self) -> String {
        match self {
            Self::NotInstalled => "that file is not installed".into(),
            Self::NoUpdate => "there is no update for that file".into(),
            Self::Wings(message) => message.clone(),
            Self::Other(err) => err.to_string(),
        }
    }
}

impl From<anyhow::Error> for JarError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

/// Deletes a jar, with its plugin's folder when asked. Answers what was deleted.
pub async fn delete(
    state: &crate::State,
    server: &Server,
    loader: Loader,
    file: &str,
    with_data: bool,
) -> Result<Vec<compact_str::CompactString>, JarError> {
    let Some((row, _)) = row_by_file(state, server, loader, file).await? else {
        return Err(JarError::NotInstalled);
    };

    let mut targets = vec![row.file.clone()];
    if with_data && let Some(folder) = row.data_folder {
        targets.push(folder);
    }

    client(state, server)
        .await?
        .post_servers_server_files_delete(
            server.uuid,
            &wings_api::servers_server_files_delete::post::RequestBody {
                root: format!("/{}", loader.directory()).into(),
                files: targets.clone(),
            },
        )
        .await
        .map_err(|err| JarError::Wings(format!("wings did not delete the file: {err:?}")))?;

    Ok(targets)
}

/// Turns a jar on or off (`.disabled`). Answers its new name.
pub async fn toggle(
    state: &crate::State,
    server: &Server,
    loader: Loader,
    file: &str,
    enabled: bool,
) -> Result<compact_str::CompactString, JarError> {
    let Some((row, _)) = row_by_file(state, server, loader, file).await? else {
        return Err(JarError::NotInstalled);
    };

    let base = base_name(&row.file);
    let target: compact_str::CompactString = if enabled {
        base.into()
    } else {
        compact_str::format_compact!("{base}.disabled")
    };

    if target != row.file {
        client(state, server)
            .await?
            .put_servers_server_files_rename(
                server.uuid,
                &wings_api::servers_server_files_rename::put::RequestBody {
                    root: format!("/{}", loader.directory()).into(),
                    files: vec![
                        wings_api::servers_server_files_rename::put::RequestBodyFiles {
                            from: row.file.clone(),
                            to: target.clone(),
                        },
                    ],
                },
            )
            .await
            .map_err(|err| JarError::Wings(format!("wings did not rename the file: {err:?}")))?;
    }

    Ok(target)
}

/// Replaces a jar with the newest Modrinth build for the server (a disabled jar stays disabled).
/// Answers the new file's name.
pub async fn update(
    state: &crate::State,
    server: &Server,
    loader: Loader,
    game_version: Option<&str>,
    file: &str,
) -> Result<compact_str::CompactString, JarError> {
    let Some((_, Some(identity))) = row_by_file(state, server, loader, file).await? else {
        return Err(JarError::NotInstalled);
    };
    let Some(sha1) = identity
        .sha1
        .filter(|_| identity.confidence == Confidence::Hash)
    else {
        return Err(JarError::NoUpdate);
    };

    let latest = super::modrinth::latest_by_hashes(
        state,
        std::slice::from_ref(&sha1),
        loader.modrinth_loaders(),
        game_version,
    )
    .await?;
    let Some(version) = latest
        .get(&sha1)
        .filter(|v| Some(&v.id) != identity.version_id.as_ref())
    else {
        return Err(JarError::NoUpdate);
    };
    let Some(download) = version
        .file
        .clone()
        .filter(|f| super::plain_file_name(&f.name))
    else {
        return Err(JarError::NoUpdate);
    };
    let Some(url) = download.url else {
        return Err(JarError::NoUpdate);
    };

    let disabled = file.to_ascii_lowercase().ends_with(".disabled");
    let target: compact_str::CompactString = if disabled {
        compact_str::format_compact!("{}.disabled", download.name)
    } else {
        download.name.clone()
    };

    super::content::pull(state, server, &url, &target, loader.directory())
        .await
        .map_err(|err| JarError::Wings(err.message()))?;

    if target != file {
        client(state, server)
            .await?
            .post_servers_server_files_delete(
                server.uuid,
                &wings_api::servers_server_files_delete::post::RequestBody {
                    root: format!("/{}", loader.directory()).into(),
                    files: vec![file.into()],
                },
            )
            .await
            .map_err(|err| {
                JarError::Wings(format!("wings did not delete the old file: {err:?}"))
            })?;
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(name: &str, depends: &[&str], disabled: bool) -> InstalledJar {
        InstalledJar {
            file: format!("{name}.jar").into(),
            disabled,
            size: 1,
            modified: chrono::Utc::now(),
            pending: false,
            name: name.into(),
            version: None,
            authors: Vec::new(),
            description: None,
            kind: Some(Kind::Plugin),
            source: None,
            project_id: None,
            url: None,
            icon: None,
            confidence: Confidence::Metadata,
            depends: depends.iter().map(|d| (*d).into()).collect(),
            missing: Vec::new(),
            data_folder: None,
            update: None,
        }
    }

    #[test]
    fn missing_plugins_are_named() {
        let mut rows = vec![
            plugin("Shop", &["Vault", "essentials", "WorldGuard"], false),
            plugin("Vault", &[], false),
            plugin("Essentials", &[], false),
            plugin("WorldGuard", &[], true),
        ];
        mark_missing(&mut rows);

        assert_eq!(rows[0].missing, vec!["WorldGuard"]);
        assert!(rows[3].missing.is_empty());
    }

    #[test]
    fn jar_names_become_words() {
        assert_eq!(normalize_name("EssentialsX-2.21.0.jar"), "essentials x");
        assert_eq!(normalize_name("sodium-fabric-0.6.0+mc1.21.1.jar"), "sodium");
        assert_eq!(normalize_name("WorldEdit.jar.disabled"), "world edit");
        assert_eq!(base_name("WorldEdit.jar.disabled"), "WorldEdit.jar");
        assert_eq!(comparable("World-Edit 7"), "worldedit7");
    }

    #[test]
    fn a_plugin_yml_is_read() {
        let meta = bukkit(
            "name: Essentials\nversion: 2.21.0\nauthor: md_5\nauthors: [snowleo]\ndepend: [Vault]\nwebsite: https://essentialsx.net\n",
        )
        .unwrap();

        assert_eq!(meta.kind, Some(Kind::Plugin));
        assert_eq!(meta.name.as_deref(), Some("Essentials"));
        assert_eq!(meta.version.as_deref(), Some("2.21.0"));
        assert_eq!(meta.authors, vec!["snowleo", "md_5"]);
        assert_eq!(meta.depends, vec!["Vault"]);
    }

    #[test]
    fn a_paper_plugin_yml_lists_required_server_dependencies() {
        let meta = bukkit(
            "name: Thing\nversion: '${version}'\ndependencies:\n  server:\n    LuckPerms:\n      required: true\n    Vault:\n      required: false\n",
        )
        .unwrap();

        assert_eq!(meta.version, None);
        assert_eq!(meta.depends, vec!["LuckPerms"]);
    }

    #[test]
    fn fabric_and_quilt_descriptors_are_read() {
        let fabric_meta = fabric(
            r#"{"id":"lithium","name":"Lithium","version":"0.15.0","authors":["jellysquid3",{"name":"2No2Name"}],"depends":{"fabricloader":"*","fabric-api":"*"}}"#,
        )
        .unwrap();
        assert_eq!(fabric_meta.name.as_deref(), Some("Lithium"));
        assert_eq!(fabric_meta.authors, vec!["jellysquid3", "2No2Name"]);
        assert_eq!(fabric_meta.depends, vec!["fabric-api"]);

        let quilt_meta = fabric(
            r#"{"quilt_loader":{"id":"qsl","version":"1.0","metadata":{"name":"QSL","contributors":{"Quilt":"Owner"}},"depends":[{"id":"minecraft"},"quilt_base"]}}"#,
        )
        .unwrap();
        assert_eq!(quilt_meta.name.as_deref(), Some("QSL"));
        assert_eq!(quilt_meta.authors, vec!["Quilt"]);
        assert_eq!(quilt_meta.depends, vec!["quilt_base"]);
    }

    #[test]
    fn a_mods_toml_is_read_with_the_manifest_version() {
        let meta = forge(
            r#"
modLoader = "javafml"
[[mods]]
modId = "jei"
version = "${file.jarVersion}"
displayName = "Just Enough Items"
authors = "mezz, Ranger"
[[dependencies.jei]]
modId = "forge"
mandatory = true
[[dependencies.jei]]
modId = "architectury"
type = "required"
[[dependencies.jei]]
modId = "optional_thing"
type = "optional"
"#,
            manifest_version("Manifest-Version: 1.0\nImplementation-Version: 19.0.0.1\n"),
        )
        .unwrap();

        assert_eq!(meta.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(meta.version.as_deref(), Some("19.0.0.1"));
        assert_eq!(meta.authors, vec!["mezz", "Ranger"]);
        assert_eq!(meta.depends, vec!["architectury"]);
    }
}
