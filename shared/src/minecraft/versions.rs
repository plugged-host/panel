//! plugged.host: the Minecraft versions each loader has builds for, newest first, from the
//! loaders' own APIs (docs/features/minecraft.md). Only releases: no snapshots, pre-releases or
//! release candidates.

use super::{Loader, TTL};

/// The versions the loader has builds for, newest first.
pub async fn list(
    state: &crate::State,
    loader: Loader,
) -> Result<Vec<compact_str::CompactString>, anyhow::Error> {
    let key = format!(
        "minecraft::versions::{}",
        serde_json::to_string(&loader)?.trim_matches('"')
    );

    state
        .cache
        .cached(&key, TTL, || async {
            let versions = match loader {
                Loader::Paper => {
                    let mut versions = paper(state).await?;
                    // Purpur follows Paper; a failing Purpur API leaves Paper's list.
                    versions.extend(purpur(state).await.unwrap_or_default());
                    versions
                }
                Loader::Fabric | Loader::Quilt => fabric(state).await?,
                Loader::Forge => forge(state).await?,
                Loader::NeoForge => neoforge(state).await?,
            };

            Ok::<_, anyhow::Error>(normalise(versions))
        })
        .await
}

async fn paper(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    let payload =
        super::get_json(state, "https://fill.papermc.io/v3/projects/paper", &[], &[]).await?;

    // v3 groups the versions by their minor line
    let mut versions = Vec::new();
    collect_strings(&payload["versions"], &mut versions);

    Ok(versions)
}

async fn purpur(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    let payload = super::get_json(state, "https://api.purpurmc.org/v2/purpur", &[], &[]).await?;

    let mut versions = Vec::new();
    collect_strings(&payload["versions"], &mut versions);

    Ok(versions)
}

async fn fabric(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    let payload = super::get_json(
        state,
        "https://meta.fabricmc.net/v2/versions/game",
        &[],
        &[],
    )
    .await?;

    Ok(payload
        .as_array()
        .map(|games| {
            games
                .iter()
                .filter(|game| game["stable"].as_bool() == Some(true))
                .filter_map(|game| game["version"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

async fn forge(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    let payload = super::get_json(
        state,
        "https://files.minecraftforge.net/maven/net/minecraftforge/forge/promotions_slim.json",
        &[],
        &[],
    )
    .await?;

    Ok(payload["promos"]
        .as_object()
        .map(|promos| {
            promos
                .keys()
                .filter_map(|key| key.split('-').next().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

async fn neoforge(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    let metadata = state
        .client
        .get("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml")
        .timeout(super::TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let releases = mojang_releases(state).await.unwrap_or_default();

    Ok(neoforge_versions(&metadata, &releases))
}

async fn mojang_releases(state: &crate::State) -> Result<Vec<String>, anyhow::Error> {
    state
        .cache
        .cached("minecraft::versions::mojang", TTL, || async {
            let payload = super::get_json(
                state,
                "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
                &[],
                &[],
            )
            .await?;

            Ok::<_, anyhow::Error>(
                payload["versions"]
                    .as_array()
                    .map(|versions| {
                        versions
                            .iter()
                            .filter(|v| v["type"].as_str() == Some("release"))
                            .filter_map(|v| v["id"].as_str().map(str::to_string))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default(),
            )
        })
        .await
}

/// NeoForge's artifacts (`21.1.77`, `26.1.0.3-beta`) map to Minecraft versions (`1.21.1`, `26.1`):
/// the first candidate Mojang released wins.
fn neoforge_versions(metadata: &str, releases: &[String]) -> Vec<String> {
    let mut versions = Vec::new();

    for artifact in metadata.split("<version>").skip(1) {
        let Some(artifact) = artifact.split("</version>").next() else {
            continue;
        };
        let base = artifact.trim().split('-').next().unwrap_or_default();
        let parts: Vec<&str> = base.split('.').collect();
        let numeric = |part: Option<&&str>| {
            part.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        };
        // April Fools' builds ("0.25w14craftmine.3-beta") have no numeric minor
        if !numeric(parts.first()) || !numeric(parts.get(1)) {
            continue;
        }

        let mut candidates = Vec::new();
        if numeric(parts.get(2)) {
            candidates.push(format!("{}.{}.{}", parts[0], parts[1], parts[2]));
        }
        candidates.push(format!("{}.{}", parts[0], parts[1]));
        candidates.push(format!("1.{}.{}", parts[0], parts[1]));
        candidates.push(format!("1.{}", parts[0]));

        if let Some(candidate) = candidates.into_iter().find(|candidate| {
            if releases.is_empty() {
                candidate.starts_with("1.")
            } else {
                releases.contains(candidate)
            }
        }) {
            versions.push(candidate);
        }
    }

    // NeoForge's 1.20.1 build lives under another artifact; the egg's installer knows it
    versions.push("1.20.1".into());

    versions
}

fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_strings(v, out)),
        serde_json::Value::Object(map) => map.values().for_each(|v| collect_strings(v, out)),
        _ => {}
    }
}

/// Releases only (`1.21.4`, `26.1`), deduplicated, newest first: the year-numbered versions
/// (26.x) come after the 1.x line.
fn normalise(versions: Vec<String>) -> Vec<compact_str::CompactString> {
    let mut clean: Vec<String> = versions
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| {
            let parts: Vec<&str> = v.split('.').collect();
            (1..=3).contains(&parts.len())
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    clean.sort_by(|a, b| compare(b, a));
    clean.dedup();

    clean.into_iter().map(Into::into).collect()
}

fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let legacy = |v: &str| {
        if v.starts_with("1.") || v == "1" {
            0
        } else {
            1
        }
    };
    let numbers = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };

    legacy(a)
        .cmp(&legacy(b))
        .then_with(|| numbers(a).cmp(&numbers(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_are_sorted_newest_first() {
        let versions = normalise(vec![
            "1.20.1".into(),
            "1.21.4".into(),
            "26.1".into(),
            "1.21.11-pre1".into(),
            "24w14a".into(),
            "1.21.4".into(),
            "1.9".into(),
        ]);

        assert_eq!(versions, vec!["26.1", "1.21.4", "1.20.1", "1.9"]);
    }

    #[test]
    fn neoforge_artifacts_map_to_minecraft_releases() {
        let metadata = "<versions><version>20.4.80</version><version>21.1.77</version>\
            <version>0.25w14craftmine.3-beta</version><version>26.1.0.3-beta</version></versions>";
        let releases = vec![
            "1.20.4".to_string(),
            "1.21.1".to_string(),
            "26.1".to_string(),
        ];

        assert_eq!(
            neoforge_versions(metadata, &releases),
            vec!["1.20.4", "1.21.1", "26.1", "1.20.1"]
        );
    }
}
