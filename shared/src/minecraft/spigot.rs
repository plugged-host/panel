//! plugged.host: SpigotMC's free resources through Spiget, for plugin servers
//! (docs/features/minecraft.md). Spiget has no version filter and only the newest build is
//! downloadable: the resource's own file on Spiget's CDN, or a GitHub release asset.

use super::{Kind, Project, SEARCH_TTL, SearchPage, Source, TTL, Version, VersionFile};
use std::sync::LazyLock;

const BASE: &str = "https://api.spiget.org/v2";
const CDN: &str = "https://cdn.spiget.org/file/spiget-resources/";
const SITE: &str = "https://www.spigotmc.org/";
pub const PAGE_SIZE: u64 = 20;
const FIELDS: &str = "id,name,tag,external,premium,file,testedVersions,downloads,icon.url,author,version,updateDate,category";
/// The categories that hold Bukkit plugins (Bungee and proxies left out).
const CATEGORIES: &[(u64, &str)] = &[
    (4, "Spigot"),
    (14, "Chat"),
    (15, "Tools and Utilities"),
    (16, "Misc"),
    (17, "Fun"),
    (18, "World Management"),
    (21, "Universal"),
    (22, "Mechanics"),
    (23, "Economy"),
    (24, "Game Mode"),
    (26, "Libraries / APIs"),
    (28, "No Rating"),
];

static GITHUB_ASSET: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)^https://github\.com/[\w.-]+/[\w.-]+/releases/download/[^/?#\s]+/[^/?#\s]+\.jar$",
    )
    .unwrap()
});

/// Whether SpigotMC is offered: the setting, and only for plugin servers.
pub async fn enabled(state: &crate::State) -> bool {
    state
        .settings
        .get()
        .await
        .map(|settings| settings.games.minecraft_spigot)
        .unwrap_or(false)
}

fn category(resource: &serde_json::Value) -> Option<&'static str> {
    let id = resource["category"]["id"].as_u64()?;
    CATEGORIES.iter().find(|(c, _)| *c == id).map(|(_, n)| *n)
}

fn listable(resource: &serde_json::Value) -> bool {
    resource["premium"].as_bool() != Some(true) && category(resource).is_some()
}

/// Where the resource's newest build downloads from, `None` when the panel cannot fetch it.
fn download_url(resource: &serde_json::Value) -> Option<String> {
    if resource["premium"].as_bool() == Some(true) {
        return None;
    }
    let id = resource["id"].as_u64()?;

    if resource["external"].as_bool() != Some(true) {
        return resource["file"]["type"]
            .as_str()
            .is_some_and(|t| t.eq_ignore_ascii_case(".jar"))
            .then(|| format!("{CDN}{id}.jar"));
    }

    resource["file"]["externalUrl"]
        .as_str()
        .filter(|url| GITHUB_ASSET.is_match(url))
        .map(str::to_string)
}

/// One page of free plugins, most downloaded first, or the ones whose name matches.
pub async fn search(
    state: &crate::State,
    query: &str,
    page: u64,
) -> Result<SearchPage, anyhow::Error> {
    let query: String = query
        .trim()
        .chars()
        .map(|c| if "/\\?#%".contains(c) { ' ' } else { c })
        .take(80)
        .collect();
    let query = query.trim();
    let page = page.max(1);

    let mut parameters = vec![
        ("size", PAGE_SIZE.to_string()),
        ("page", page.to_string()),
        ("sort", "-downloads".to_string()),
        ("fields", FIELDS.to_string()),
    ];
    let url = if query.is_empty() {
        format!("{BASE}/resources/free")
    } else {
        parameters.push(("field", "name".to_string()));
        format!("{BASE}/search/resources/{}", urlencoding::encode(query))
    };

    let key = super::cache_key("spigot", "search", &format!("{url}|{parameters:?}"));
    let (resources, total): (Vec<serde_json::Value>, u64) = state
        .cache
        .cached(&key, SEARCH_TTL, || async {
            let response = state
                .client
                .get(&url)
                .query(&parameters)
                .timeout(super::TIMEOUT)
                .header("User-Agent", super::user_agent(state).await)
                .header("Accept", "application/json")
                .send()
                .await?;
            // Spiget answers a search without hits with 404.
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok((Vec::new(), 0));
            }
            let response = response.error_for_status()?;
            let total = response
                .headers()
                .get("X-Total")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let resources: Vec<serde_json::Value> = response.json().await?;

            Ok::<_, anyhow::Error>((resources, total))
        })
        .await?;

    let resources: Vec<_> = resources.into_iter().filter(listable).collect();
    let authors = authors(
        state,
        resources
            .iter()
            .filter_map(|r| r["author"]["id"].as_u64())
            .collect(),
    )
    .await;

    Ok(SearchPage {
        hits: resources
            .iter()
            .map(|resource| project(resource, &authors))
            .collect(),
        total,
        page,
        per_page: PAGE_SIZE,
    })
}

/// The plugin whose name starts with this name (for identifying a jar by its plugin.yml).
pub async fn find_by_name(state: &crate::State, name: &str) -> Option<Project> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let wanted = super::jars::comparable(name);
    search(state, name, 1)
        .await
        .ok()?
        .hits
        .into_iter()
        .find(|hit| {
            let title = super::jars::comparable(&hit.title);
            title == wanted
                || title
                    .strip_prefix(wanted.as_str())
                    .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric()))
        })
}

async fn resource(
    state: &crate::State,
    id: u64,
) -> Result<Option<serde_json::Value>, anyhow::Error> {
    if id == 0 {
        return Ok(None);
    }

    let key = super::cache_key("spigot", "resource", &id.to_string());
    let resource: serde_json::Value = state
        .cache
        .cached(&key, TTL, || async {
            super::get_json(
                state,
                &format!("{BASE}/resources/{id}"),
                &[("fields", FIELDS.to_string())],
                &[],
            )
            .await
        })
        .await?;

    Ok(listable(&resource).then_some(resource))
}

/// A plugin by its resource id.
pub async fn project_by_id(
    state: &crate::State,
    id: u64,
) -> Result<Option<Project>, anyhow::Error> {
    let Some(resource) = resource(state, id).await? else {
        return Ok(None);
    };
    let authors = authors(
        state,
        resource["author"]["id"].as_u64().into_iter().collect(),
    )
    .await;

    Ok(Some(project(&resource, &authors)))
}

/// The newest build, the only one Spiget serves; `loose` when the server's version is not among
/// the versions the author tested.
pub async fn versions(
    state: &crate::State,
    id: u64,
    game_version: Option<&str>,
) -> Result<(Vec<Version>, bool), anyhow::Error> {
    let Some(resource) = resource(state, id).await? else {
        return Ok((Vec::new(), false));
    };
    let Some(url) = download_url(&resource) else {
        return Ok((Vec::new(), false));
    };

    let key = super::cache_key("spigot", "latest", &id.to_string());
    let latest: serde_json::Value = state
        .cache
        .cached(&key, SEARCH_TTL, || async {
            super::get_json(
                state,
                &format!("{BASE}/resources/{id}/versions/latest"),
                &[],
                &[],
            )
            .await
        })
        .await
        .unwrap_or_default();

    let tested = super::texts(&resource, "testedVersions");
    let number = super::text(&latest, "name");
    let version_id = match super::text(&latest, "id") {
        id if id.is_empty() => super::text(&resource["version"], "id"),
        id => id,
    };
    if version_id.is_empty() {
        return Ok((Vec::new(), false));
    }

    let loose = game_version.is_some_and(|v| !tested.is_empty() && !tested_on(&tested, v));
    let version = Version {
        id: version_id,
        project_id: id.to_string().into(),
        name: if number.is_empty() {
            super::text(&resource, "name")
        } else {
            number.clone()
        },
        file: Some(VersionFile {
            name: file_name(&resource, &number, &url).into(),
            url: Some(url.into()),
            size: 0,
        }),
        number,
        channel: "release".into(),
        date: latest["releaseDate"]
            .as_i64()
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|t| t.to_rfc3339().into())
            .unwrap_or_default(),
        game_versions: tested.into_iter().rev().collect(),
        loaders: Vec::new(),
        dependencies: Vec::new(),
    };

    Ok((vec![version], loose))
}

/// Whether a version is among the tested ones; `1.21.4` is covered by `1.21`.
fn tested_on(tested: &[compact_str::CompactString], version: &str) -> bool {
    let line: Vec<&str> = version.split('.').take(2).collect();
    let line = line.join(".");

    tested.iter().any(|t| t == version || *t == line)
}

/// A file name for the build: Spiget's CDN serves `<id>.jar`, so it is named after the plugin.
fn file_name(resource: &serde_json::Value, number: &str, url: &str) -> String {
    let base = if url.starts_with(CDN) {
        let name = super::text(resource, "name");
        if number.is_empty() {
            name.to_string()
        } else {
            format!("{name}-{number}")
        }
    } else {
        url.rsplit('/')
            .next()
            .map(|name| {
                urlencoding::decode(name)
                    .map(|n| n.into_owned())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
            .trim_end_matches(".jar")
            .to_string()
    };

    let mut clean = String::with_capacity(base.len());
    for c in base.chars() {
        let c = if c.is_ascii_alphanumeric() || "._+-".contains(c) {
            c
        } else {
            '-'
        };
        if !(c == '-' && clean.ends_with('-')) && !(c == '.' && clean.ends_with('.')) {
            clean.push(c);
        }
    }
    let clean: String = clean.trim_matches(['.', '-']).chars().take(100).collect();

    if clean.is_empty() {
        format!("spigot-{}.jar", resource["id"].as_u64().unwrap_or(0))
    } else {
        format!("{clean}.jar")
    }
}

/// Author names, one request per author not seen in a day.
async fn authors(
    state: &crate::State,
    mut ids: Vec<u64>,
) -> std::collections::HashMap<u64, String> {
    ids.sort_unstable();
    ids.dedup();

    let lookups = ids.into_iter().filter(|id| *id > 0).map(|id| async move {
        let key = format!("minecraft::spigot::author::{id}");
        let name = state
            .cache
            .cached(&key, 86_400, || async {
                let author = super::get_json(
                    state,
                    &format!("{BASE}/authors/{id}"),
                    &[("fields", "id,name".to_string())],
                    &[],
                )
                .await?;

                Ok::<_, anyhow::Error>(super::text(&author, "name").to_string())
            })
            .await
            .ok()?;

        Some((id, name))
    });

    futures_util::future::join_all(lookups)
        .await
        .into_iter()
        .flatten()
        .collect()
}

fn decode_entities(value: &str) -> compact_str::CompactString {
    value
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .into()
}

fn project(
    resource: &serde_json::Value,
    authors: &std::collections::HashMap<u64, String>,
) -> Project {
    let id = resource["id"].as_u64().unwrap_or(0);
    let name = decode_entities(&super::text(resource, "name"));
    let icon = super::text(&resource["icon"], "url");

    Project {
        source: Source::SpigotMc,
        id: id.to_string().into(),
        slug: name
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-")
            .into(),
        description: decode_entities(&super::text(resource, "tag")),
        author: resource["author"]["id"]
            .as_u64()
            .and_then(|id| authors.get(&id))
            .map(|name| name.as_str().into())
            .unwrap_or_default(),
        downloads: resource["downloads"].as_u64().unwrap_or(0),
        icon: (!icon.is_empty() && !icon.contains("://"))
            .then(|| compact_str::format_compact!("{SITE}{}", icon.trim_start_matches('/'))),
        categories: vec![category(resource).unwrap_or("Spigot").into()],
        url: compact_str::format_compact!("{SITE}resources/{id}/"),
        kind: Kind::Plugin,
        installable: download_url(resource).is_some(),
        title: name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_jars_on_the_cdn_or_github_download() {
        let hosted = serde_json::json!({ "id": 1, "file": { "type": ".jar" } });
        assert_eq!(download_url(&hosted).unwrap(), format!("{CDN}1.jar"));

        let premium = serde_json::json!({ "id": 1, "premium": true, "file": { "type": ".jar" } });
        assert_eq!(download_url(&premium), None);

        let github = serde_json::json!({ "id": 2, "external": true, "file": { "externalUrl": "https://github.com/o/r/releases/download/v1/Plugin-1.0.jar" } });
        assert!(download_url(&github).is_some());

        let elsewhere = serde_json::json!({ "id": 3, "external": true, "file": { "externalUrl": "https://example.com/Plugin.jar" } });
        assert_eq!(download_url(&elsewhere), None);
    }

    #[test]
    fn builds_are_named_after_the_plugin() {
        let resource = serde_json::json!({ "id": 9, "name": "Essentials X (Chat)" });
        assert_eq!(
            file_name(&resource, "2.21.0", &format!("{CDN}9.jar")),
            "Essentials-X-Chat-2.21.0.jar"
        );
        assert_eq!(
            file_name(
                &resource,
                "",
                "https://github.com/o/r/releases/download/v1/My%20Plugin.jar"
            ),
            "My-Plugin.jar"
        );
    }

    #[test]
    fn a_minor_line_covers_its_patches() {
        let tested = vec!["1.20".into(), "1.21".into()];
        assert!(tested_on(&tested, "1.21.4"));
        assert!(!tested_on(&tested, "1.19.4"));
    }
}
