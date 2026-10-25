//! plugged.host: CurseForge's Core API, in the shape every source is shown in
//! (docs/features/minecraft.md). It needs an API key (Admin › Settings › Hosting › Minecraft);
//! without one CurseForge is simply not offered.

use super::{Kind, Loader, Project, SEARCH_TTL, SearchPage, Source, TTL, Version, VersionFile};

const BASE: &str = "https://api.curseforge.com/v1";
/// Minecraft's game id.
const GAME_ID: u32 = 432;
const CLASS_MODS: u32 = 6;
const CLASS_PLUGINS: u32 = 5;
const CLASS_MODPACKS: u32 = 4471;
pub const PAGE_SIZE: u64 = 20;
/// CurseForge refuses an index past this.
const MAX_RESULTS: u64 = 10_000;

/// The API key, when one is set.
pub async fn key(state: &crate::State) -> Option<String> {
    let settings = state.settings.get().await.ok()?;

    settings
        .games
        .minecraft_curseforge_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

fn class(kind: Kind) -> u32 {
    match kind {
        Kind::Plugin => CLASS_PLUGINS,
        Kind::Mod => CLASS_MODS,
        Kind::Modpack => CLASS_MODPACKS,
    }
}

/// CurseForge's mod loader type, for filtering files.
fn loader_type(loader: Loader) -> Option<u32> {
    match loader {
        Loader::Forge => Some(1),
        Loader::Fabric => Some(4),
        Loader::Quilt => Some(5),
        Loader::NeoForge => Some(6),
        Loader::Paper => None,
    }
}

async fn get(
    state: &crate::State,
    key: &str,
    path: &str,
    query: &[(&str, String)],
) -> Result<serde_json::Value, anyhow::Error> {
    super::get_json(
        state,
        &format!("{BASE}{path}"),
        query,
        &[("x-api-key", key.to_string())],
    )
    .await
}

/// One page of projects of this kind; mods are filtered to the loader.
pub async fn search(
    state: &crate::State,
    key: &str,
    kind: Kind,
    loader: Option<Loader>,
    query: &str,
    game_version: Option<&str>,
    page: u64,
) -> Result<SearchPage, anyhow::Error> {
    let query = query.trim();
    let page = page.max(1);
    let index = (page - 1) * PAGE_SIZE;
    if index >= MAX_RESULTS {
        return Ok(SearchPage {
            page,
            per_page: PAGE_SIZE,
            ..Default::default()
        });
    }

    let mut parameters = vec![
        ("gameId", GAME_ID.to_string()),
        ("classId", class(kind).to_string()),
        ("pageSize", PAGE_SIZE.to_string()),
        ("index", index.to_string()),
        // 2 = popularity for a browse, 1 = featured for a search
        (
            "sortField",
            if query.is_empty() { "2" } else { "1" }.to_string(),
        ),
        ("sortOrder", "desc".to_string()),
    ];
    if !query.is_empty() {
        parameters.push(("searchFilter", query.to_string()));
    }
    if let Some(version) = game_version.filter(|v| !v.is_empty()) {
        parameters.push(("gameVersion", version.to_string()));
    }
    if kind == Kind::Mod
        && let Some(loader_type) = loader.and_then(loader_type)
    {
        parameters.push(("modLoaderType", loader_type.to_string()));
    }

    let cache_key = super::cache_key("curseforge", "search", &format!("{parameters:?}"));
    state
        .cache
        .cached(&cache_key, SEARCH_TTL, || async {
            let payload = get(state, key, "/mods/search", &parameters).await?;
            let total = payload["pagination"]["totalCount"]
                .as_u64()
                .unwrap_or(0)
                .min(MAX_RESULTS);

            Ok::<_, anyhow::Error>(SearchPage {
                hits: payload["data"]
                    .as_array()
                    .map(|mods| mods.iter().map(|m| project(m, kind)).collect())
                    .unwrap_or_default(),
                total,
                page,
                per_page: PAGE_SIZE,
            })
        })
        .await
}

/// A project by its id.
pub async fn project_by_id(
    state: &crate::State,
    key: &str,
    id: u64,
) -> Result<Option<Project>, anyhow::Error> {
    if id == 0 {
        return Ok(None);
    }

    let cache_key = super::cache_key("curseforge", "mod", &id.to_string());
    let payload: serde_json::Value = state
        .cache
        .cached(&cache_key, TTL, || async {
            get(state, key, &format!("/mods/{id}"), &[]).await
        })
        .await?;

    let data = &payload["data"];
    if !data.is_object() {
        return Ok(None);
    }
    let kind = match data["classId"].as_u64() {
        Some(c) if c == CLASS_PLUGINS as u64 => Kind::Plugin,
        Some(c) if c == CLASS_MODPACKS as u64 => Kind::Modpack,
        _ => Kind::Mod,
    };

    Ok(Some(project(data, kind)))
}

/// A project's files for this Minecraft version and loader, newest first.
pub async fn files(
    state: &crate::State,
    key: &str,
    id: u64,
    game_version: Option<&str>,
    loader: Option<Loader>,
) -> Result<Vec<Version>, anyhow::Error> {
    if id == 0 {
        return Ok(Vec::new());
    }

    let mut parameters = vec![("pageSize", "50".to_string())];
    if let Some(version) = game_version.filter(|v| !v.is_empty()) {
        parameters.push(("gameVersion", version.to_string()));
    }
    if let Some(loader_type) = loader.and_then(loader_type) {
        parameters.push(("modLoaderType", loader_type.to_string()));
    }

    let cache_key = super::cache_key("curseforge", "files", &format!("{id}|{parameters:?}"));
    state
        .cache
        .cached(&cache_key, SEARCH_TTL, || async {
            let payload = get(state, key, &format!("/mods/{id}/files"), &parameters).await?;
            let mut files: Vec<Version> = payload["data"]
                .as_array()
                .map(|files| files.iter().map(file).collect())
                .unwrap_or_default();
            files.sort_by(|a, b| b.date.cmp(&a.date));

            Ok::<_, anyhow::Error>(files)
        })
        .await
}

/// One file of a project.
pub async fn file_by_id(
    state: &crate::State,
    key: &str,
    id: u64,
    file_id: u64,
) -> Result<Option<Version>, anyhow::Error> {
    let cache_key = super::cache_key("curseforge", "file", &format!("{id}|{file_id}"));
    let payload: serde_json::Value = state
        .cache
        .cached(&cache_key, TTL, || async {
            get(state, key, &format!("/mods/{id}/files/{file_id}"), &[]).await
        })
        .await?;

    Ok(payload["data"].is_object().then(|| file(&payload["data"])))
}

/// Where a file downloads from. The file itself often names no URL; this endpoint always does
/// for files the author lets third parties download.
pub async fn download_url(
    state: &crate::State,
    key: &str,
    id: u64,
    file_id: u64,
) -> Result<Option<compact_str::CompactString>, anyhow::Error> {
    let cache_key = super::cache_key("curseforge", "download", &format!("{id}|{file_id}"));
    let payload: serde_json::Value = state
        .cache
        .cached(&cache_key, TTL, || async {
            get(
                state,
                key,
                &format!("/mods/{id}/files/{file_id}/download-url"),
                &[],
            )
            .await
        })
        .await?;

    Ok(payload["data"]
        .as_str()
        .filter(|url| url.starts_with("https://"))
        .map(Into::into))
}

/// A jar CurseForge knows by its fingerprint.
pub struct FingerprintMatch {
    pub fingerprint: u32,
    pub mod_id: u64,
    pub display_name: compact_str::CompactString,
}

/// The jars CurseForge knows among these fingerprints (one request for all).
pub async fn fingerprints(
    state: &crate::State,
    key: &str,
    fingerprints: &[u32],
) -> Result<Vec<FingerprintMatch>, anyhow::Error> {
    if fingerprints.is_empty() {
        return Ok(Vec::new());
    }

    let payload = super::post_json(
        state,
        &format!("{BASE}/fingerprints/{GAME_ID}"),
        &serde_json::json!({ "fingerprints": fingerprints }),
        &[("x-api-key", key.to_string())],
    )
    .await?;

    Ok(payload["data"]["exactMatches"]
        .as_array()
        .map(|matches| {
            matches
                .iter()
                .filter_map(|m| {
                    let file = &m["file"];
                    Some(FingerprintMatch {
                        fingerprint: file["fileFingerprint"].as_u64()? as u32,
                        mod_id: file["modId"].as_u64().or_else(|| m["id"].as_u64())?,
                        display_name: super::text(file, "displayName"),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

fn project(data: &serde_json::Value, kind: Kind) -> Project {
    let id = super::text(data, "id");

    Project {
        source: Source::CurseForge,
        slug: match super::text(data, "slug") {
            slug if slug.is_empty() => id.clone(),
            slug => slug,
        },
        title: super::text(data, "name"),
        description: super::text(data, "summary"),
        author: data["authors"]
            .as_array()
            .and_then(|authors| authors.first())
            .map(|author| super::text(author, "name"))
            .unwrap_or_default(),
        downloads: data["downloadCount"].as_f64().unwrap_or(0.0) as u64,
        icon: Some(super::text(&data["logo"], "thumbnailUrl")).filter(|icon| !icon.is_empty()),
        categories: data["categories"]
            .as_array()
            .map(|categories| {
                categories
                    .iter()
                    .map(|category| super::text(category, "name"))
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        url: super::text(&data["links"], "websiteUrl"),
        kind,
        installable: true,
        client_only: false,
        id,
    }
}

fn file(data: &serde_json::Value) -> Version {
    let name = super::text(data, "fileName");
    let game_versions = super::texts(data, "gameVersions");

    Version {
        id: super::text(data, "id"),
        project_id: super::text(data, "modId"),
        name: match super::text(data, "displayName") {
            display if display.is_empty() => name.clone(),
            display => display,
        },
        number: super::text(data, "displayName"),
        channel: match data["releaseType"].as_u64() {
            Some(2) => "beta",
            Some(3) => "alpha",
            _ => "release",
        }
        .into(),
        date: super::text(data, "fileDate"),
        // CurseForge lists loaders among the game versions
        loaders: game_versions
            .iter()
            .filter(|v| !v.starts_with(|c: char| c.is_ascii_digit()))
            .map(|v| v.to_ascii_lowercase().into())
            .collect(),
        game_versions: game_versions
            .into_iter()
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
            .collect(),
        file: Some(VersionFile {
            url: Some(super::text(data, "downloadUrl")).filter(|url| !url.is_empty()),
            size: data["fileLength"].as_u64().unwrap_or(0),
            name,
        }),
        dependencies: data["dependencies"]
            .as_array()
            .map(|dependencies| {
                dependencies
                    .iter()
                    // 3 = required
                    .filter(|d| d["relationType"].as_u64() == Some(3))
                    .map(|d| super::text(d, "modId"))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_splits_loaders_from_game_versions() {
        let version = file(&serde_json::json!({
            "id": 42,
            "modId": 7,
            "displayName": "JEI 1.0",
            "fileName": "jei-1.0.jar",
            "releaseType": 2,
            "gameVersions": ["1.20.1", "Forge", "NeoForge"],
            "dependencies": [{ "modId": 9, "relationType": 3 }, { "modId": 10, "relationType": 2 }],
        }));

        assert_eq!(version.id, "42");
        assert_eq!(version.channel, "beta");
        assert_eq!(version.game_versions, vec!["1.20.1"]);
        assert_eq!(version.loaders, vec!["forge", "neoforge"]);
        assert_eq!(version.dependencies, vec!["9"]);
        assert_eq!(version.file.unwrap().url, None);
    }
}
