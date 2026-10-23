//! plugged.host: Modrinth's search, project and version endpoints, in the shape every source is
//! shown in (docs/features/minecraft.md). Modrinth needs no key.

use super::{Kind, Project, SEARCH_TTL, SearchPage, Source, TTL, Version, VersionFile};
use std::collections::HashMap;

const BASE: &str = "https://api.modrinth.com/v2";
pub const PAGE_SIZE: u64 = 20;
/// Modrinth refuses offsets past this.
const MAX_RESULTS: u64 = 10_000;

/// One page of projects of this kind for these loaders, newest Minecraft version filter
/// optional. Without a search text the most downloaded come first and client-only mods are left
/// out (they do nothing on a server); a typed search keeps them, as authors mislabel them.
pub async fn search(
    state: &crate::State,
    kind: Kind,
    loaders: &[&str],
    query: &str,
    game_version: Option<&str>,
    page: u64,
) -> Result<SearchPage, anyhow::Error> {
    let query = query.trim();
    let page = page.max(1);
    let offset = (page - 1) * PAGE_SIZE;
    if offset >= MAX_RESULTS {
        return Ok(SearchPage {
            page,
            per_page: PAGE_SIZE,
            ..Default::default()
        });
    }

    let mut facets = vec![
        vec![format!("project_type:{}", kind.as_str())],
        loaders.iter().map(|l| format!("categories:{l}")).collect(),
    ];
    if let Some(version) = game_version.filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{version}")]);
    }
    if kind == Kind::Mod && query.is_empty() {
        facets.push(vec![
            "server_side:required".into(),
            "server_side:optional".into(),
        ]);
    }

    let parameters = vec![
        ("query", query.to_string()),
        ("facets", serde_json::to_string(&facets)?),
        ("limit", PAGE_SIZE.to_string()),
        ("offset", offset.to_string()),
        (
            "index",
            if query.is_empty() {
                "downloads"
            } else {
                "relevance"
            }
            .to_string(),
        ),
    ];

    let key = super::cache_key("modrinth", "search", &format!("{parameters:?}"));
    state
        .cache
        .cached(&key, SEARCH_TTL, || async {
            let payload =
                super::get_json(state, &format!("{BASE}/search"), &parameters, &[]).await?;

            Ok::<_, anyhow::Error>(SearchPage {
                hits: payload["hits"]
                    .as_array()
                    .map(|hits| hits.iter().map(|hit| project(hit, kind)).collect())
                    .unwrap_or_default(),
                total: payload["total_hits"].as_u64().unwrap_or(0).min(MAX_RESULTS),
                page,
                per_page: PAGE_SIZE,
            })
        })
        .await
}

/// A project's versions for these loaders (and Minecraft version), newest first.
pub async fn versions(
    state: &crate::State,
    project: &str,
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<Vec<Version>, anyhow::Error> {
    if project.is_empty() || loaders.is_empty() {
        return Ok(Vec::new());
    }

    let mut parameters = vec![("loaders", serde_json::to_string(loaders)?)];
    if let Some(version) = game_version.filter(|v| !v.is_empty()) {
        parameters.push(("game_versions", serde_json::to_string(&[version])?));
    }

    let key = super::cache_key("modrinth", "versions", &format!("{project}|{parameters:?}"));
    state
        .cache
        .cached(&key, TTL, || async {
            let payload = super::get_json(
                state,
                &format!("{BASE}/project/{}/version", urlencoding::encode(project)),
                &parameters,
                &[],
            )
            .await?;

            let mut versions: Vec<Version> = payload
                .as_array()
                .map(|items| items.iter().map(version).collect())
                .unwrap_or_default();
            versions.sort_by(|a, b| b.date.cmp(&a.date));

            Ok::<_, anyhow::Error>(versions)
        })
        .await
}

/// A project by id or slug, `None` when Modrinth does not know it.
pub async fn project_by_id(
    state: &crate::State,
    id: &str,
    kind: Kind,
) -> Result<Option<Project>, anyhow::Error> {
    if id.is_empty() {
        return Ok(None);
    }

    let key = super::cache_key("modrinth", "project", id);
    let payload: serde_json::Value = state
        .cache
        .cached(&key, TTL, || async {
            match super::get_json(
                state,
                &format!("{BASE}/project/{}", urlencoding::encode(id)),
                &[],
                &[],
            )
            .await
            {
                Ok(payload) => Ok::<_, anyhow::Error>(payload),
                Err(err) if not_found(&err) => Ok(serde_json::Value::Null),
                Err(err) => Err(err),
            }
        })
        .await?;

    Ok(payload.get("id").map(|_| project(&payload, kind)))
}

/// A version by id, whatever the server runs.
pub async fn version_by_id(
    state: &crate::State,
    id: &str,
) -> Result<Option<Version>, anyhow::Error> {
    if id.is_empty() {
        return Ok(None);
    }

    let key = super::cache_key("modrinth", "version", id);
    let payload: serde_json::Value = state
        .cache
        .cached(&key, TTL, || async {
            match super::get_json(
                state,
                &format!("{BASE}/version/{}", urlencoding::encode(id)),
                &[],
                &[],
            )
            .await
            {
                Ok(payload) => Ok::<_, anyhow::Error>(payload),
                Err(err) if not_found(&err) => Ok(serde_json::Value::Null),
                Err(err) => Err(err),
            }
        })
        .await?;

    Ok(payload.get("id").map(|_| version(&payload)))
}

/// The versions these SHA-1 hashes belong to (one request for all).
pub async fn versions_by_hashes(
    state: &crate::State,
    hashes: &[compact_str::CompactString],
) -> Result<HashMap<compact_str::CompactString, Version>, anyhow::Error> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }

    let payload = super::post_json(
        state,
        &format!("{BASE}/version_files"),
        &serde_json::json!({ "hashes": hashes, "algorithm": "sha1" }),
        &[],
    )
    .await?;

    Ok(payload
        .as_object()
        .map(|found| {
            found
                .iter()
                .filter(|(_, v)| v.get("id").is_some())
                .map(|(hash, v)| (hash.as_str().into(), version(v)))
                .collect()
        })
        .unwrap_or_default())
}

/// The newest version each of these jars could update to, on these loaders and this Minecraft
/// version.
pub async fn latest_by_hashes(
    state: &crate::State,
    hashes: &[compact_str::CompactString],
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<HashMap<compact_str::CompactString, Version>, anyhow::Error> {
    if hashes.is_empty() || loaders.is_empty() {
        return Ok(HashMap::new());
    }

    let mut hashes = hashes.to_vec();
    hashes.sort();
    let mut body = serde_json::json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "loaders": loaders,
    });
    if let Some(version) = game_version.filter(|v| !v.is_empty()) {
        body["game_versions"] = serde_json::json!([version]);
    }

    let key = super::cache_key("modrinth", "updates", &body.to_string());
    state
        .cache
        .cached(&key, TTL, || async {
            let payload =
                super::post_json(state, &format!("{BASE}/version_files/update"), &body, &[])
                    .await?;

            Ok::<_, anyhow::Error>(
                payload
                    .as_object()
                    .map(|found| {
                        found
                            .iter()
                            .filter(|(_, v)| v.get("id").is_some())
                            .map(|(hash, v)| (hash.as_str().into(), version(v)))
                            .collect::<HashMap<compact_str::CompactString, Version>>()
                    })
                    .unwrap_or_default(),
            )
        })
        .await
}

/// Several projects by id, in one request.
pub async fn projects_by_ids(
    state: &crate::State,
    ids: &[compact_str::CompactString],
) -> Result<HashMap<compact_str::CompactString, Project>, anyhow::Error> {
    let mut ids = ids.to_vec();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let parameters = vec![("ids", serde_json::to_string(&ids)?)];
    let key = super::cache_key("modrinth", "projects", &parameters[0].1);
    state
        .cache
        .cached(&key, TTL, || async {
            let payload =
                super::get_json(state, &format!("{BASE}/projects"), &parameters, &[]).await?;

            Ok::<_, anyhow::Error>(
                payload
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter(|p| p.get("id").is_some())
                            .map(|p| {
                                let kind = match p["project_type"].as_str() {
                                    Some("modpack") => Kind::Modpack,
                                    Some("plugin") => Kind::Plugin,
                                    _ => Kind::Mod,
                                };
                                (super::text(p, "id"), project(p, kind))
                            })
                            .collect::<HashMap<compact_str::CompactString, Project>>()
                    })
                    .unwrap_or_default(),
            )
        })
        .await
}

fn not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<reqwest::Error>()
        .and_then(|err| err.status())
        .is_some_and(|status| status == reqwest::StatusCode::NOT_FOUND)
}

/// A search hit or a project (`/search` calls the id `project_id`, `/project` calls it `id`).
fn project(hit: &serde_json::Value, kind: Kind) -> Project {
    let id = match super::text(hit, "project_id") {
        id if id.is_empty() => super::text(hit, "id"),
        id => id,
    };
    let slug = match super::text(hit, "slug") {
        slug if slug.is_empty() => id.clone(),
        slug => slug,
    };
    let categories = match super::texts(hit, "display_categories") {
        categories if categories.is_empty() => super::texts(hit, "categories"),
        categories => categories,
    };

    Project {
        source: Source::Modrinth,
        title: match super::text(hit, "title") {
            title if title.is_empty() => slug.clone(),
            title => title,
        },
        description: super::text(hit, "description"),
        author: super::text(hit, "author"),
        downloads: hit["downloads"].as_u64().unwrap_or(0),
        icon: Some(super::text(hit, "icon_url")).filter(|icon| !icon.is_empty()),
        categories,
        url: compact_str::format_compact!("https://modrinth.com/project/{slug}"),
        kind,
        installable: true,
        id,
        slug,
    }
}

fn version(version: &serde_json::Value) -> Version {
    let files = version["files"].as_array().cloned().unwrap_or_default();
    let file = files
        .iter()
        .find(|file| file["primary"].as_bool() == Some(true))
        .or_else(|| files.first())
        .map(|file| VersionFile {
            name: super::text(file, "filename"),
            url: Some(super::text(file, "url")).filter(|url| !url.is_empty()),
            size: file["size"].as_u64().unwrap_or(0),
        });

    Version {
        id: super::text(version, "id"),
        project_id: super::text(version, "project_id"),
        name: match super::text(version, "name") {
            name if name.is_empty() => super::text(version, "version_number"),
            name => name,
        },
        number: super::text(version, "version_number"),
        channel: match super::text(version, "version_type") {
            channel if channel.is_empty() => "release".into(),
            channel => channel,
        },
        date: super::text(version, "date_published"),
        game_versions: super::texts(version, "game_versions"),
        loaders: super::texts(version, "loaders"),
        file,
        dependencies: version["dependencies"]
            .as_array()
            .map(|dependencies| {
                dependencies
                    .iter()
                    .filter(|d| d["dependency_type"].as_str() == Some("required"))
                    .filter_map(|d| d["project_id"].as_str().map(Into::into))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_hit_takes_the_shared_shape() {
        let hit = serde_json::json!({
            "project_id": "AANobbMI",
            "slug": "sodium",
            "title": "Sodium",
            "author": "jellysquid3",
            "downloads": 1234,
            "icon_url": "",
            "display_categories": ["optimization"],
        });
        let project = project(&hit, Kind::Mod);

        assert_eq!(project.id, "AANobbMI");
        assert_eq!(project.url, "https://modrinth.com/project/sodium");
        assert_eq!(project.icon, None);
        assert_eq!(project.categories, vec!["optimization"]);
    }

    #[test]
    fn a_version_installs_its_primary_file() {
        let payload = serde_json::json!({
            "id": "v1",
            "project_id": "p",
            "version_number": "1.0",
            "files": [
                { "filename": "sources.jar", "url": "https://cdn.example/sources.jar", "primary": false },
                { "filename": "mod.jar", "url": "https://cdn.example/mod.jar", "primary": true, "size": 10 },
            ],
            "dependencies": [
                { "project_id": "fabric-api", "dependency_type": "required" },
                { "project_id": "modmenu", "dependency_type": "optional" },
            ],
        });
        let version = version(&payload);

        assert_eq!(version.name, "1.0");
        assert_eq!(version.channel, "release");
        assert_eq!(version.file.unwrap().name, "mod.jar");
        assert_eq!(version.dependencies, vec!["fabric-api"]);
    }
}
