//! plugged.host: ads in the panel (docs/features/ads.md).
//!
//! Adsterra banner and native units render in frames the panel serves itself
//! (`/api/ads/unit`), sandboxed without `allow-same-origin`: the ad code runs in an opaque origin
//! and cannot reach the panel, its cookies or its API. The frame tells the page whether it filled
//! (`postMessage`), so empty units fold away. Page-level scripts (Social Bar, Popunder, Monetag)
//! run in the page and are allowed by `script-src` ([`AppSettingsAds::to_csp_script_src`]).
//! The start waiting room is `crate::waiting_room`, sponsored banners `crate::models::ad_banner`.

use crate::{
    models::{ByUuid, node::Node, user::User},
    settings::ads::AppSettingsAds,
};
use compact_str::CompactString;
use serde::Serialize;
use sqlx::Row;
use std::{collections::HashMap, sync::LazyLock};
use utoipa::ToSchema;

/// The Adsterra banner sizes a zone may have.
pub const SIZES: &[(i32, i32)] = &[
    (728, 90),
    (468, 60),
    (320, 50),
    (300, 250),
    (160, 600),
    (160, 300),
];

/// Where a banner zone's `invoke.js` comes from when a zone line names no host.
const DEFAULT_HOST: &str = "www.highperformanceformat.com";

/// One Adsterra banner zone.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    pub key: CompactString,
    pub width: i32,
    pub height: i32,
    pub host: CompactString,
    /// The script the snippet loads: `https://host/<key>/invoke.js` in older Adsterra code,
    /// `https://host/22/<key>` in newer.
    pub src: CompactString,
}

/// A host name: labels of letters, digits and dashes, at least two of them.
pub fn valid_host(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// An element id or a verification code: letters, digits, `-` and `_`.
pub fn valid_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_key(key: &str) -> bool {
    (8..=64).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_alphanumeric())
}

static SCRIPT_URL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?:https?:)?//[A-Za-z0-9.-]+(?:/[A-Za-z0-9._-]+)+"#).unwrap()
});

/// A script URL, or the first one in a pasted snippet, as `https://host/path`. No query, no
/// fragment, no `..`: whatever else was pasted is not a URL this panel writes into a page.
pub fn clean_script_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let candidate = SCRIPT_URL.find(raw)?.as_str();
    // a bare URL must be the whole field; a snippet gives its first script URL
    if !raw.contains('<') && candidate != raw {
        return None;
    }

    let rest = candidate
        .strip_prefix("https:")
        .or_else(|| candidate.strip_prefix("http:"))
        .unwrap_or(candidate)
        .strip_prefix("//")?;
    let (host, path) = rest.split_once('/')?;
    if !valid_host(host)
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }

    Some(format!("https://{}/{path}", host.to_ascii_lowercase()))
}

/// The host of a script URL stored by [`clean_script_url`].
pub fn script_host(url: &str) -> Option<CompactString> {
    let host = url.strip_prefix("https://")?.split('/').next()?;

    valid_host(host).then(|| host.into())
}

/// The valid host names of a list, one per line.
pub fn hosts(text: &str) -> impl Iterator<Item = CompactString> + '_ {
    text.lines()
        .map(str::trim)
        .filter(|host| valid_host(host))
        .map(CompactString::from)
}

static SNIPPET_KEY: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"['"]?key['"]?\s*:\s*['"]([A-Za-z0-9]+)['"]"#).unwrap());
static SNIPPET_WIDTH: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"['"]?width['"]?\s*:\s*(\d+)"#).unwrap());
static SNIPPET_HEIGHT: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"['"]?height['"]?\s*:\s*(\d+)"#).unwrap());
static SNIPPET_SRC: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"\bsrc\s*=\s*["']([^"']+)["']"#).unwrap());

/// A zone; `source` is its host or its script URL (left out: [`DEFAULT_HOST`]).
fn zone(key: &str, width: i32, height: i32, source: Option<&str>) -> Result<Zone, String> {
    if !valid_key(key) {
        return Err(format!("{key} is not an adsterra zone key"));
    }
    if !SIZES.contains(&(width, height)) {
        return Err(format!("{width}x{height} is not an adsterra banner size"));
    }
    let source = source.unwrap_or(DEFAULT_HOST);
    let (host, src) = if source.contains('/') {
        let src =
            clean_script_url(source).ok_or_else(|| format!("{source} is not a script URL"))?;
        let host = script_host(&src).ok_or_else(|| format!("{source} is not a script URL"))?;
        (host, src.into())
    } else if valid_host(source) {
        let host = source.to_ascii_lowercase();
        let src = format!("https://{host}/{key}/invoke.js");
        (host.into(), src.into())
    } else {
        return Err(format!("{source} is not a host name"));
    };

    Ok(Zone {
        key: key.into(),
        width,
        height,
        host,
        src,
    })
}

/// Banner zones from the settings field: Adsterra's snippets as pasted, or one
/// `key:WIDTHxHEIGHT:host` or `key:WIDTHxHEIGHT:script-url` per line (the host may be left out).
/// A key appears once.
pub fn parse_zones(text: &str) -> Result<Vec<Zone>, String> {
    let mut zones: Vec<Zone> = Vec::new();

    if text.contains("atOptions") {
        for block in text.split("atOptions").skip(1) {
            let key = SNIPPET_KEY.captures(block).map(|c| c[1].to_string());
            let width = SNIPPET_WIDTH
                .captures(block)
                .and_then(|c| c[1].parse().ok());
            let height = SNIPPET_HEIGHT
                .captures(block)
                .and_then(|c| c[1].parse().ok());
            let src = SNIPPET_SRC.captures(block).map(|c| c[1].to_string());
            let (Some(key), Some(width), Some(height)) = (key, width, height) else {
                return Err("a pasted snippet has no key, width or height".into());
            };
            zones.push(zone(&key, width, height, src.as_deref())?);
        }
    } else {
        for (number, line) in text.lines().map(str::trim).enumerate() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, ':').map(str::trim);
            let (Some(key), Some(size)) = (parts.next(), parts.next()) else {
                return Err(format!(
                    "line {} is not a zone (key:WIDTHxHEIGHT:host)",
                    number + 1
                ));
            };
            let Some((width, height)) = size
                .split_once('x')
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            else {
                return Err(format!("line {} has no WIDTHxHEIGHT", number + 1));
            };
            zones.push(zone(key, width, height, parts.next())?);
        }
    }

    let mut seen = std::collections::HashSet::new();
    zones.retain(|zone| seen.insert(zone.key.clone()));

    Ok(zones)
}

/// The zones as the settings store them: the host alone when the script is the older
/// `https://host/<key>/invoke.js`, else the script URL.
pub fn zones_to_text(zones: &[Zone]) -> String {
    zones
        .iter()
        .map(|zone| {
            let source = if zone.src == format!("https://{}/{}/invoke.js", zone.host, zone.key) {
                &zone.host
            } else {
                &zone.src
            };
            format!("{}:{}x{}:{source}", zone.key, zone.width, zone.height)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Who sees ads.
#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// No ads: ads are off, or the account has the second membership (while Fewer Ads for
    /// Members is on).
    None,
    /// Only the waiting room before a start, with its own units and no page scripts: the first
    /// membership while Fewer Ads for Members is on. No banners or popups on the pages.
    Room,
    /// Ads with the slower refresh: a membership while Fewer Ads for Members is off.
    Member,
    Everyone,
}

pub async fn audience(state: &crate::State, user: &User) -> Result<Audience, anyhow::Error> {
    let settings = state.settings.get().await?;
    if !settings.ads.enabled {
        return Ok(Audience::None);
    }
    let member_free = settings.ads.member_free;
    let memberships = settings.economy.membership_enabled;
    drop(settings);
    if !memberships {
        return Ok(Audience::Everyone);
    }

    let membership = crate::economy::membership(&state.database, user.uuid).await?;

    Ok(audience_of(
        membership.priority(chrono::Utc::now().naive_utc()),
        member_free,
    ))
}

/// The audience of an account with start queue priority `priority` (2 the second membership, 1
/// the first, 0 none).
fn audience_of(priority: u8, member_free: bool) -> Audience {
    match (priority, member_free) {
        (0, _) => Audience::Everyone,
        (_, false) => Audience::Member,
        (1, true) => Audience::Room,
        (_, true) => Audience::None,
    }
}

/// The origin pages are served from, for `frame-ancestors`.
pub fn origin(app_url: &str) -> Option<String> {
    let url = reqwest::Url::parse(app_url).ok()?;
    let origin = url.origin();

    origin.is_tuple().then(|| origin.ascii_serialization())
}

/// Tells the page whether the unit holds anything with a size (an ad), so empty units fold.
const REPORT_SCRIPT: &str = r#"<script>(function(){var last=null;function filled(){var all=document.body.getElementsByTagName('*');for(var i=0;i<all.length;i++){var el=all[i];if(el.tagName==='SCRIPT'||el.tagName==='STYLE')continue;var r=el.getBoundingClientRect();if(r.width>=20&&r.height>=20)return true;}return false;}function report(){var f=filled();if(f!==last){last=f;parent.postMessage({adUnit:true,filled:f},'*');}}window.addEventListener('load',function(){report();setTimeout(report,5000);});setInterval(report,2000);})();</script>"#;

/// In an opaque origin, `localStorage`, `sessionStorage` and `document.cookie` throw; ad scripts
/// that touch them would die before they fill, so they get in-memory stand-ins there.
const STORAGE_SHIM: &str = r#"<script>(function(){function mem(){var d={};return{getItem:function(k){return Object.prototype.hasOwnProperty.call(d,k)?d[k]:null},setItem:function(k,v){d[k]=String(v)},removeItem:function(k){delete d[k]},clear:function(){d={}},key:function(i){return Object.keys(d)[i]||null},get length(){return Object.keys(d).length}}}['localStorage','sessionStorage'].forEach(function(n){try{window[n].length}catch(e){try{Object.defineProperty(window,n,{value:mem(),configurable:true})}catch(e2){}}});try{document.cookie}catch(e){var jar={};try{Object.defineProperty(document,'cookie',{configurable:true,get:function(){return Object.keys(jar).map(function(k){return k+'='+jar[k]}).join('; ')},set:function(v){var p=String(v).split(';')[0],i=p.indexOf('=');if(i>0)jar[p.slice(0,i).trim()]=p.slice(i+1)}})}catch(e2){}}})();</script>"#;

const UNIT_HEAD: &str = r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><style>html,body{margin:0;padding:0;background:transparent;overflow:hidden}</style>"#;
const UNIT_BODY: &str = "</head><body>";

/// The document of a banner unit: Adsterra's snippet for the zone.
pub fn banner_html(zone: &Zone) -> String {
    format!(
        "{UNIT_HEAD}{STORAGE_SHIM}{UNIT_BODY}<script>atOptions={{'key':'{key}','format':'iframe','height':{height},'width':{width},'params':{{}}}};</script><script src=\"{src}\"></script>{REPORT_SCRIPT}</body></html>",
        key = zone.key,
        width = zone.width,
        height = zone.height,
        src = zone.src,
    )
}

/// The document of the native unit, or `None` while it is not configured.
pub fn native_html(settings: &AppSettingsAds) -> Option<String> {
    let src = clean_script_url(&settings.native_script)?;
    let container = if valid_id(&settings.native_container) {
        settings.native_container.to_string()
    } else {
        // Adsterra's container id is `container-` and the script's id: the directory of an older
        // `…/<id>/invoke.js`, the file of a newer `…/21/<id>`
        let mut segments = src.rsplit('/');
        let last = segments.next()?;
        let id = if last == "invoke.js" {
            segments.next()?
        } else {
            last
        };
        format!("container-{}", Some(id).filter(|id| valid_id(id))?)
    };

    Some(format!(
        "{UNIT_HEAD}{STORAGE_SHIM}{UNIT_BODY}<script async=\"async\" data-cfasync=\"false\" src=\"{src}\"></script><div id=\"{container}\"></div>{REPORT_SCRIPT}</body></html>"
    ))
}

/// The Content-Security-Policy of a unit document: anything inside its sandbox, framed by the
/// panel only. `own_origin` keeps the frame's origin, which only the ad frame address may: there
/// it is not the panel's.
pub fn unit_csp(origin: Option<&str>, own_origin: bool) -> String {
    format!(
        "default-src * data: blob: 'unsafe-inline' 'unsafe-eval'; frame-ancestors {}; sandbox {}",
        origin.unwrap_or("'self'"),
        unit_sandbox(own_origin),
    )
}

/// The sandbox tokens of a unit frame.
pub fn unit_sandbox(own_origin: bool) -> &'static str {
    if own_origin {
        "allow-scripts allow-same-origin allow-popups allow-popups-to-escape-sandbox allow-forms"
    } else {
        "allow-scripts allow-popups allow-popups-to-escape-sandbox allow-forms"
    }
}

/// `https://host[:port]` of an ad frame address, or `None` when it is anything else (another
/// scheme, a path, a query).
pub fn frame_origin(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }

    origin(value)
}

/// Whether a request's `Host` is the ad frame address's (`frame_origin`), and not the panel's.
pub fn is_frame_host(settings: &AppSettingsAds, app_url: &str, host: Option<&str>) -> bool {
    let (Some(host), Some(frame)) = (host, frame_origin(&settings.frame_origin)) else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let host = host.strip_suffix(":443").unwrap_or(&host);
    let matches = |origin: &str| origin.strip_prefix("https://") == Some(host);

    matches(&frame) && origin(app_url).is_none_or(|panel| !matches(&panel))
}

/// What a page needs to show ads, from the public settings (`/api/settings`).
#[derive(ToSchema, Serialize)]
pub struct ApiAdsSettings {
    pub enabled: bool,
    pub show_account: bool,
    pub show_server: bool,
    pub slot_top: bool,
    pub slot_server_tools: bool,
    pub slot_pages: bool,
    pub slot_foot: bool,
    /// The most units the server tools strip and the page slots fill together on one page.
    pub max_units_per_page: i32,
    pub refresh_seconds: i32,
    pub refresh_member_seconds: i32,
    pub zones: Vec<Zone>,
    /// `https://host[:port]` the unit frames load from, with an origin of their own; `None` loads
    /// them from the panel's address, sandboxed to an opaque origin.
    pub frame_origin: Option<String>,
    pub native: bool,
    pub native_height: i32,
    pub social_bar_script: Option<CompactString>,
    pub social_bar_everywhere: bool,
    pub popunder_script: Option<CompactString>,
    pub room_script: Option<CompactString>,
    pub monetag_push: Option<ApiMonetagTag>,
    pub monetag_vignette: Option<ApiMonetagTag>,
    pub monetag_vignette_minutes: i32,
    pub monetag_vignette_members: bool,
    pub room_banner: bool,
    pub room_native: bool,
    pub room_social_bar: bool,
    pub room_popunder: bool,
    pub room_interstitial: bool,
    pub adblock_enabled: bool,
    pub adblock_mode: CompactString,
    pub adblock_grace: i32,
    pub adblock_dismiss_hours: i32,
    pub adblock_probe: bool,
    pub adblock_skip_console: bool,
    pub adblock_message: Option<CompactString>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiMonetagTag {
    pub zone: CompactString,
    pub src: CompactString,
}

fn script(value: &CompactString) -> Option<CompactString> {
    clean_script_url(value).map(Into::into)
}

fn monetag(zone: &CompactString, src: &CompactString) -> Option<ApiMonetagTag> {
    (!zone.is_empty() && zone.bytes().all(|b| b.is_ascii_digit()))
        .then(|| script(src))
        .flatten()
        .map(|src| ApiMonetagTag {
            zone: zone.clone(),
            src,
        })
}

impl ApiAdsSettings {
    pub fn new(settings: &AppSettingsAds) -> Self {
        let on = settings.enabled;

        Self {
            enabled: on,
            show_account: settings.show_account,
            show_server: settings.show_server,
            slot_top: settings.slot_top,
            slot_server_tools: settings.slot_server_tools,
            slot_pages: settings.slot_pages,
            slot_foot: settings.slot_foot,
            max_units_per_page: settings.max_units_per_page,
            refresh_seconds: settings.refresh_seconds,
            refresh_member_seconds: settings.refresh_member_seconds,
            zones: if on {
                parse_zones(&settings.zones).unwrap_or_default()
            } else {
                Vec::new()
            },
            frame_origin: frame_origin(&settings.frame_origin),
            native: on && native_html(settings).is_some(),
            native_height: settings.native_height,
            social_bar_script: on.then(|| script(&settings.social_bar_script)).flatten(),
            social_bar_everywhere: settings.social_bar_everywhere,
            popunder_script: on.then(|| script(&settings.popunder_script)).flatten(),
            room_script: on.then(|| script(&settings.room_script)).flatten(),
            monetag_push: on
                .then(|| monetag(&settings.monetag_push_zone, &settings.monetag_push_src))
                .flatten(),
            monetag_vignette: on
                .then(|| {
                    monetag(
                        &settings.monetag_vignette_zone,
                        &settings.monetag_vignette_src,
                    )
                })
                .flatten(),
            monetag_vignette_minutes: settings.monetag_vignette_minutes,
            monetag_vignette_members: settings.monetag_vignette_members,
            room_banner: settings.room_banner,
            room_native: settings.room_native,
            room_social_bar: settings.room_social_bar,
            room_popunder: settings.room_popunder,
            room_interstitial: settings.room_interstitial,
            adblock_enabled: on && settings.adblock_enabled,
            adblock_mode: settings.adblock_mode.clone(),
            adblock_grace: settings.adblock_grace,
            adblock_dismiss_hours: settings.adblock_dismiss_hours,
            adblock_probe: settings.adblock_probe,
            adblock_skip_console: settings.adblock_skip_console,
            adblock_message: (!settings.adblock_message.is_empty())
                .then(|| settings.adblock_message.clone()),
        }
    }
}

/// `<meta>` tags that verify the panel's site with ad networks, for the page's `<head>`.
pub fn verification_meta(settings: &AppSettingsAds) -> Option<String> {
    (settings.enabled && valid_id(&settings.monetag_verification)).then(|| {
        format!(
            "<meta name=\"monetag\" content=\"{}\">",
            settings.monetag_verification
        )
    })
}

/// Whether this minute is a server's turn for the in-game message: every server once per
/// `interval` minutes, spread over the minutes by its short id.
pub fn in_game_turn(uuid_short: i32, interval: i32, minute: i64) -> bool {
    let interval = interval.max(1) as i64;

    (uuid_short as i64).rem_euclid(interval) == minute.rem_euclid(interval)
}

/// Sends the in-game message to the running Minecraft servers (eggs with the `eula` feature) whose
/// turn it is, of accounts that are not admins and have no membership.
pub async fn send_in_game(state: &crate::State) -> Result<(), anyhow::Error> {
    let settings = state.settings.get().await?;
    if !settings.ads.enabled || !settings.ads.in_game_enabled {
        return Ok(());
    }
    let interval = settings.ads.in_game_interval.max(5);
    let message = if settings.ads.in_game_message.is_empty() {
        format!("This server is hosted on {}.", settings.app.name)
    } else {
        settings.ads.in_game_message.to_string()
    };
    let memberships = settings.economy.membership_enabled;
    drop(settings);

    let minute = chrono::Utc::now().timestamp() / 60;
    let rows = sqlx::query(
        "SELECT servers.uuid, servers.uuid_short, servers.node_uuid
        FROM servers
        JOIN nest_eggs ON nest_eggs.uuid = servers.egg_uuid
        JOIN users ON users.uuid = servers.owner_uuid
        WHERE servers.status IS NULL
            AND NOT servers.suspended
            AND servers.destination_node_uuid IS NULL
            AND (servers.limbo_state IS NULL OR servers.limbo_state = 'stopped')
            AND 'eula' = ANY(nest_eggs.features)
            AND NOT users.admin
            AND NOT ($1 AND EXISTS (
                SELECT 1 FROM user_memberships
                WHERE user_memberships.user_uuid = users.uuid
                    AND (user_memberships.plus_until > NOW()
                        OR user_memberships.plusplus_until > NOW())
            ))
            AND MOD(servers.uuid_short::BIGINT - $2, $3) = 0",
    )
    .bind(memberships)
    .bind(minute.rem_euclid(interval as i64))
    .bind(interval as i64)
    .fetch_all(state.database.read())
    .await?;

    let mut by_node: HashMap<uuid::Uuid, Vec<uuid::Uuid>> = HashMap::new();
    for row in rows {
        let uuid: uuid::Uuid = row.try_get("uuid")?;
        let uuid_short: i32 = row.try_get("uuid_short")?;
        if !in_game_turn(uuid_short, interval, minute) {
            continue;
        }
        by_node
            .entry(row.try_get("node_uuid")?)
            .or_default()
            .push(uuid);
    }

    let body = wings_api::servers_server_commands::post::RequestBody {
        commands: vec![format!("say {message}").into()],
    };
    for (node_uuid, servers) in by_node {
        let Ok(Some(node)) = Node::by_uuid_optional(&state.database, node_uuid).await else {
            continue;
        };
        // only running servers: a node that does not answer gets the next turn
        let Ok(usage) = node.fetch_server_resources(&state.database).await else {
            continue;
        };
        let Ok(client) = node.api_client(&state.database).await else {
            continue;
        };

        for server in servers {
            if !usage
                .get(&server)
                .is_some_and(|usage| matches!(usage.state, wings_api::ServerState::Running))
            {
                continue;
            }
            if let Err(err) = client.post_servers_server_commands(server, &body).await {
                tracing::debug!(%server, "failed to send the in-game ad: {err:?}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_see_fewer_ads() {
        assert_eq!(audience_of(0, true), Audience::Everyone);
        assert_eq!(audience_of(0, false), Audience::Everyone);
        // the first membership keeps the waiting room only, the second sees none
        assert_eq!(audience_of(1, true), Audience::Room);
        assert_eq!(audience_of(2, true), Audience::None);
        assert_eq!(audience_of(1, false), Audience::Member);
        assert_eq!(audience_of(2, false), Audience::Member);
    }

    #[test]
    fn native_container_comes_from_the_script() {
        let mut settings = AppSettingsAds::default();
        for script in [
            "https://ads.example.org/21/a6adf12cf8b7049e224cbb6ed3aead67",
            "//ads.example.org/a6adf12cf8b7049e224cbb6ed3aead67/invoke.js",
        ] {
            settings.native_script = script.into();
            assert!(
                native_html(&settings)
                    .unwrap()
                    .contains("<div id=\"container-a6adf12cf8b7049e224cbb6ed3aead67\">"),
                "{script}"
            );
        }

        settings.native_container = "container-own".into();
        assert!(
            native_html(&settings)
                .unwrap()
                .contains("<div id=\"container-own\">")
        );
    }

    #[test]
    fn script_urls_are_cleaned_or_refused() {
        assert_eq!(
            clean_script_url("//pl123.example.com/ab/cd/ef/invoke.js").as_deref(),
            Some("https://pl123.example.com/ab/cd/ef/invoke.js")
        );
        assert_eq!(
            clean_script_url(
                "<script async=\"async\" data-cfasync=\"false\" src=\"//pl1.example.com/a1b2/invoke.js\"></script>"
            )
            .as_deref(),
            Some("https://pl1.example.com/a1b2/invoke.js")
        );
        assert_eq!(clean_script_url("https://example.com/a.js?x=1"), None);
        assert_eq!(clean_script_url("https://example.com/../a.js"), None);
        assert_eq!(clean_script_url("javascript:alert(1)"), None);
        assert_eq!(clean_script_url("https://localhost/a.js"), None);
        assert_eq!(
            script_host("https://cdn.example.com/a/b.js").as_deref(),
            Some("cdn.example.com")
        );
    }

    #[test]
    fn zones_parse_from_lines_and_snippets() {
        let zones = parse_zones(
            "abcdef0123456789abcdef0123456789:728x90:www.example.com\n\n0123456789abcdef0123456789abcdef:300x250",
        )
        .unwrap();
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[1].host, DEFAULT_HOST);
        assert_eq!(
            parse_zones(&zones_to_text(&zones)).unwrap(),
            zones,
            "the stored form parses back"
        );

        let snippet = "<script type=\"text/javascript\">\n\tatOptions = {\n\t\t'key' : 'abcdef0123456789abcdef0123456789',\n\t\t'format' : 'iframe',\n\t\t'height' : 60,\n\t\t'width' : 468,\n\t\t'params' : {}\n\t};\n</script>\n<script type=\"text/javascript\" src=\"//www.example.net/abcdef0123456789abcdef0123456789/invoke.js\"></script>";
        let zones = parse_zones(&format!("{snippet}\n{snippet}")).unwrap();
        assert_eq!(zones.len(), 1, "a key appears once");
        assert_eq!((zones[0].width, zones[0].height), (468, 60));
        assert_eq!(zones[0].host, "www.example.net");

        let snippet = "<script>\n  atOptions = {\n    'key' : 'c41025814de9a014f37ce3e96eb57e69',\n    'format' : 'iframe',\n    'height' : 90,\n    'width' : 728,\n    'params' : {}\n  };\n</script>\n<script src=\"https://ads.example.org/22/c41025814de9a014f37ce3e96eb57e69\"></script>";
        let zones = parse_zones(snippet).unwrap();
        assert_eq!(zones[0].host, "ads.example.org");
        assert_eq!(
            zones[0].src,
            "https://ads.example.org/22/c41025814de9a014f37ce3e96eb57e69"
        );
        assert_eq!(
            zones_to_text(&zones),
            "c41025814de9a014f37ce3e96eb57e69:728x90:https://ads.example.org/22/c41025814de9a014f37ce3e96eb57e69"
        );
        assert_eq!(
            parse_zones(&zones_to_text(&zones)).unwrap(),
            zones,
            "a script URL parses back"
        );

        assert!(parse_zones("abcdef0123456789:100x100").is_err());
        assert!(parse_zones("abcdef0123456789:728x90:https://example.com/a.js?b").is_err());
        assert!(parse_zones("short:728x90").is_err());
        assert!(parse_zones("abcdef0123456789:728x90:not a host").is_err());
    }

    #[test]
    fn every_server_gets_one_turn_per_interval() {
        for uuid_short in [-7, 0, 1, 59, 123_456] {
            let turns = (0..60)
                .filter(|minute| in_game_turn(uuid_short, 60, *minute))
                .count();
            assert_eq!(turns, 1);
        }
    }

    #[test]
    fn unit_documents_carry_the_zone_only() {
        let zone = zone("abcdef0123456789", 320, 50, Some("www.example.com")).unwrap();
        let html = banner_html(&zone);
        assert!(html.contains("'key':'abcdef0123456789'"));
        assert!(html.contains("src=\"https://www.example.com/abcdef0123456789/invoke.js\""));
        assert_eq!(
            origin("https://panel.example.com/x").as_deref(),
            Some("https://panel.example.com")
        );
        assert!(
            unit_csp(Some("https://panel.example.com"), false)
                .contains("frame-ancestors https://panel.example.com;")
        );
        assert!(!unit_csp(None, false).contains("allow-same-origin"));
        assert!(unit_csp(None, true).contains("allow-same-origin"));
    }

    #[test]
    fn frame_origin_is_an_origin_only() {
        assert_eq!(
            frame_origin("https://ads.example.net").as_deref(),
            Some("https://ads.example.net")
        );
        assert_eq!(
            frame_origin("https://ads.example.net:8443/").as_deref(),
            Some("https://ads.example.net:8443")
        );
        assert_eq!(
            frame_origin("https://ads.example.net:443").as_deref(),
            Some("https://ads.example.net")
        );
        assert_eq!(frame_origin("http://ads.example.net"), None);
        assert_eq!(frame_origin("https://ads.example.net/unit"), None);
        assert_eq!(frame_origin("https://ads.example.net/?a=1"), None);
        assert_eq!(frame_origin("https://user@ads.example.net"), None);
    }

    #[test]
    fn only_the_frame_host_keeps_its_origin() {
        let settings = AppSettingsAds {
            frame_origin: "https://ads.example.net".into(),
            ..Default::default()
        };
        let panel = "https://panel.example.com";
        assert!(is_frame_host(&settings, panel, Some("ads.example.net")));
        assert!(is_frame_host(&settings, panel, Some("ADS.example.net:443")));
        assert!(!is_frame_host(&settings, panel, Some("panel.example.com")));
        assert!(!is_frame_host(&settings, panel, None));
        // the panel's own address never counts, even when entered as the frame address
        assert!(!is_frame_host(
            &settings,
            "https://ads.example.net",
            Some("ads.example.net")
        ));
        assert!(!is_frame_host(
            &AppSettingsAds::default(),
            panel,
            Some("ads.example.net")
        ));
    }
}
