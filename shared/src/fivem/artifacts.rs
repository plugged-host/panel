//! plugged.host: what Cfx offers: the Linux FXServer builds, which are recommended and latest,
//! until when each is supported, and the one FiveM for GTAV Enhanced build
//! (docs/features/fivem.md).
//!
//! Two sources, the same two the image's `fivem-runtime` reads at every start: the changelog API
//! (recommended, latest and `support_policy`: build → the date its support ends) and the download
//! page, whose `__NEXT_DATA__` lists every Linux build Cfx still serves with its download URL, and
//! the Enhanced build. Both are cached for ten minutes, and the last good answer is kept for a day
//! on top, so Cfx having a bad minute shows yesterday's list instead of an empty page.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const CHANGELOG_API: &str = "https://changelogs-live.fivem.net/api/changelog/versions/linux/server";
const DOWNLOADS_PAGE: &str =
    "https://docs.fivem.net/docs/server-download/?platform=legacy&os=linux";
const TTL: u64 = 600;
const STALE_TTL: u64 = 24 * 60 * 60;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// The channels `FIVEM_VERSION` can follow; anything else is a build number.
pub const CHANNELS: &[&str] = &["recommended", "latest"];

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Build {
    pub build: u32,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Changelog {
    pub recommended: Option<u32>,
    pub latest: Option<u32>,
    /// build → the day Cfx stops supporting it.
    pub support: std::collections::BTreeMap<u32, chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Downloads {
    /// Newest first.
    pub builds: Vec<Build>,
    pub enhanced: Option<Build>,
}

/// Whether a URL is on one of Cfx's download hosts; `fivem-runtime` checks the same before it
/// downloads anything.
pub fn from_cfx(url: &str) -> bool {
    url.starts_with("https://runtime.fivem.net/")
        || url.starts_with("https://downloads.cfx-services.net/")
}

fn build_number(subtitle: &str) -> Option<u32> {
    let digits: String = subtitle
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();

    digits.chars().rev().collect::<String>().parse().ok()
}

fn number(value: Option<&serde_json::Value>) -> Option<u32> {
    match value? {
        serde_json::Value::Number(number) => number.as_u64().and_then(|n| u32::try_from(n).ok()),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// The changelog API's answer, read.
pub fn parse_changelog(json: &serde_json::Value) -> Option<Changelog> {
    let recommended = number(json.get("recommended"))?;
    let support = json
        .get("support_policy")
        .and_then(serde_json::Value::as_object)
        .map(|policy| {
            policy
                .iter()
                .filter_map(|(build, date)| {
                    let date = date.as_str()?;
                    // 9999-... means "no end"
                    if date.starts_with("9999") {
                        return None;
                    }
                    Some((
                        build.parse().ok()?,
                        chrono::NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Changelog {
        recommended: Some(recommended),
        latest: number(json.get("latest")),
        support,
    })
}

/// The download page's builds, read from its `__NEXT_DATA__`.
pub fn parse_downloads(html: &str) -> Option<Downloads> {
    const START: &str = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html.find(START)? + START.len();
    let end = start + html[start..].find("</script>")?;
    let data: serde_json::Value = serde_json::from_str(&html[start..end]).ok()?;
    let props = data.pointer("/props/pageProps")?;
    let legacy = props.get("legacy")?.as_object()?;

    let entry = |entry: &serde_json::Value| {
        let build = build_number(entry.get("subtitle")?.as_str()?)?;
        let url = entry.get("downloadURL")?.as_str()?;

        from_cfx(url).then(|| Build {
            build,
            url: url.to_string(),
        })
    };

    let mut builds: std::collections::BTreeMap<u32, Build> = Default::default();
    for channel in legacy.values() {
        for build in channel
            .get("linux")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(entry)
        {
            builds.insert(build.build, build);
        }
    }

    Some(Downloads {
        builds: builds.into_values().rev().collect(),
        enhanced: props.pointer("/enhanced/linux/0").and_then(entry),
    })
}

async fn remember<T, F, Fut>(state: &crate::State, key: &str, fetch: F) -> Option<T>
where
    T: Serialize + serde::de::DeserializeOwned + Send + Sync,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let fresh = format!("fivem::cfx::{key}");
    let last = format!("fivem::cfx::{key}::last");
    if let Ok(Some(value)) = state.cache.get::<T>(&fresh).await {
        return Some(value);
    }

    match fetch().await {
        Some(value) => {
            state.cache.set(&fresh, TTL, &value).await.ok();
            state.cache.set(&last, STALE_TTL, &value).await.ok();
            Some(value)
        }
        None => state.cache.get::<T>(&last).await.ok().flatten(),
    }
}

async fn get(state: &crate::State, url: &str) -> Option<reqwest::Response> {
    state
        .client
        .get(url)
        .timeout(TIMEOUT)
        .header("User-Agent", crate::minecraft::user_agent(state).await)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()
}

pub async fn changelog(state: &crate::State) -> Changelog {
    remember(state, "changelog", || async {
        parse_changelog(&get(state, CHANGELOG_API).await?.json().await.ok()?)
    })
    .await
    .unwrap_or_default()
}

pub async fn downloads(state: &crate::State) -> Downloads {
    remember(state, "downloads", || async {
        parse_downloads(&get(state, DOWNLOADS_PAGE).await?.text().await.ok()?)
    })
    .await
    .unwrap_or_default()
}

/// The build a channel or a version stands for now.
pub fn resolve(changelog: &Changelog, version: &str) -> Option<u32> {
    match version {
        "recommended" => changelog.recommended,
        "latest" => changelog.latest,
        version => version.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_and_support_dates() {
        let json = serde_json::json!({
            "recommended": "17000", "latest": 17346,
            "support_policy": {"17000": "2026-12-01T00:00:00Z", "17346": "9999-01-01"}
        });
        let changelog = parse_changelog(&json).unwrap();
        assert_eq!(
            (changelog.recommended, changelog.latest),
            (Some(17000), Some(17346))
        );
        assert_eq!(
            changelog.support.get(&17000),
            chrono::NaiveDate::from_ymd_opt(2026, 12, 1).as_ref()
        );
        assert!(!changelog.support.contains_key(&17346));
        assert_eq!(resolve(&changelog, "latest"), Some(17346));
        assert_eq!(resolve(&changelog, "16000"), Some(16000));
        assert_eq!(resolve(&changelog, "nope"), None);
        assert!(parse_changelog(&serde_json::json!({})).is_none());
    }

    #[test]
    fn downloads_from_the_page() {
        let data = serde_json::json!({"props": {"pageProps": {
            "legacy": {
                "recommended": {"linux": [{"subtitle": "Build 17000", "downloadURL": "https://runtime.fivem.net/artifacts/a/17000/fx.tar.xz"}]},
                "all": {"linux": [
                    {"subtitle": "Build 16000", "downloadURL": "https://runtime.fivem.net/artifacts/a/16000/fx.tar.xz"},
                    {"subtitle": "Build 15000", "downloadURL": "https://elsewhere.example.com/fx.tar.xz"}
                ]}
            },
            "enhanced": {"linux": [{"subtitle": "Build 1", "downloadURL": "https://downloads.cfx-services.net/e/1.tar.xz"}]}
        }}});
        let html = format!(
            "<html><script id=\"__NEXT_DATA__\" type=\"application/json\">{data}</script></html>"
        );
        let downloads = parse_downloads(&html).unwrap();
        assert_eq!(
            downloads.builds.iter().map(|b| b.build).collect::<Vec<_>>(),
            vec![17000, 16000]
        );
        assert_eq!(downloads.enhanced.map(|b| b.build), Some(1));
        assert!(parse_downloads("<html></html>").is_none());
    }
}
