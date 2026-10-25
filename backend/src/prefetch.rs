//! plugged.host: the API answers a page asks for first, sent inside its HTML, so the browser does
//! not wait a round trip per request stage before it can draw the page (docs/features/speed.md).
//!
//! The answers come from the panel's own routes, called in-process with the visitor's cookie and
//! address headers: authorization, permissions, rate limits and the answers' shape are exactly
//! those of the browser asking itself. Only answers the database gives fast are inlined (never one
//! that asks Wings). The frontend takes each one once, for the same URL
//! (`frontend/src/api/prefetched.ts`), and asks for anything else itself.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header, request::Parts},
};
use std::{fmt::Write, net::SocketAddr, time::Duration};

/// An answer bigger than this is left for the page to ask for.
const MAX_ANSWER: usize = 256 * 1024;
/// An answer slower than this is left out rather than holding the HTML back.
const ANSWER_TIMEOUT: Duration = Duration::from_millis(400);
/// The table page size the server list starts with (`DEFAULT_TABLE_PER_PAGE` in the frontend).
const DEFAULT_PER_PAGE: i64 = 25;

/// Passed on to the in-process requests: who is asking (the session cookie) and from where (the
/// address headers `find_ip` reads behind trusted proxies), so they count as the visitor's own.
const FORWARDED_HEADERS: &[header::HeaderName] = &[
    header::COOKIE,
    header::USER_AGENT,
    header::ACCEPT_LANGUAGE,
    header::HOST,
    header::FORWARDED,
];
const FORWARDED_CUSTOM_HEADERS: &[&str] = &[
    "x-forwarded-for",
    "x-forwarded-proto",
    "x-forwarded-host",
    "x-real-ip",
    "cf-connecting-ip",
    "true-client-ip",
];

const PUBLIC: &[&str] = &["/api/settings", "/api/languages", "/api/icons"];
const ACCOUNT: &[&str] = &[
    "/api/client/account",
    "/api/client/account/balance",
    "/api/client/account/settings",
];

pub struct Prefetched {
    /// `<script type="application/json" id="prefetched">{"<url>": <answer>, …}</script>`
    pub script: String,
    /// Whether the account's own answers are in it: the page must then never be stored.
    pub personal: bool,
    /// The public answers inlined, whose `<link rel="preload">` is no longer needed.
    pub public: Vec<&'static str>,
}

/// The answers for a page request (`GET` of the frontend's index). `None` when there is nothing
/// to inline.
pub async fn for_page(state: &shared::State, parts: &Parts) -> Option<Prefetched> {
    if parts.method != Method::GET {
        return None;
    }

    let path = parts.uri.path();
    let session_cookie = state.settings.get().await.ok()?.app.session_cookie.clone();
    let signed_in = !path.starts_with("/auth")
        && parts
            .headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .any(|cookie| {
                cookie
                    .trim()
                    .strip_prefix(session_cookie.as_str())
                    .is_some_and(|rest| rest.starts_with('='))
            });

    let mut urls: Vec<String> = PUBLIC.iter().map(|url| url.to_string()).collect();
    if signed_in {
        urls.extend(ACCOUNT.iter().map(|url| url.to_string()));
        if let Some(server) = server_of(path) {
            urls.push(format!("/api/client/servers/{server}"));
        }
    }

    let mut answers = fetch_all(state, parts, &urls).await;

    if signed_in {
        // the account's own answers only while its session is good; else the page signs in
        if !answers.iter().any(|(url, _)| url == "/api/client/account") {
            answers.retain(|(url, _)| PUBLIC.contains(&url.as_str()));
        } else if path == "/" && parts.uri.query().is_none() {
            let settings = answers
                .iter()
                .find(|(url, _)| url == "/api/client/account/settings")
                .and_then(|(_, body)| serde_json::from_slice::<serde_json::Value>(body).ok());

            if let Some(url) = settings.as_ref().and_then(server_list_url) {
                answers.extend(fetch_all(state, parts, &[url]).await);
            }
        }
    }

    if answers.is_empty() {
        return None;
    }

    let personal = answers
        .iter()
        .any(|(url, _)| !PUBLIC.contains(&url.as_str()));
    let public = PUBLIC
        .iter()
        .copied()
        .filter(|url| answers.iter().any(|(answered, _)| answered == url))
        .collect();

    let mut json = String::with_capacity(answers.iter().map(|(_, body)| body.len() + 64).sum());
    json.push('{');
    for (i, (url, body)) in answers.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let _ = write!(
            json,
            "{}:{}",
            serde_json::to_string(url).ok()?,
            std::str::from_utf8(body).ok()?
        );
    }
    json.push('}');

    Some(Prefetched {
        script: format!(
            "<script type=\"application/json\" id=\"prefetched\">{}</script>",
            escape_for_script(&json)
        ),
        personal,
        public,
    })
}

/// `html` without the `<link rel="preload" href="…">` of the answers already in it, so the
/// browser does not ask for them a second time.
pub fn without_preloads(html: &str, inlined: &[&str]) -> String {
    let mut html = html.to_string();

    for url in inlined {
        let needle = format!("<link rel=\"preload\" href=\"{url}\"");
        if let Some(start) = html.find(&needle)
            && let Some(len) = html[start..].find('>')
        {
            html.replace_range(start..=start + len, "");
        }
    }

    html
}

/// The server a `/server/{id}/…` page shows: its uuid or short uuid, as the page asks for it.
fn server_of(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/server/")?.split('/').next()?;

    (!id.is_empty()
        && id.len() <= 36
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    .then_some(id)
}

/// The server list's first page as the dashboard asks for it with the account's settings, in the
/// frontend's normalized form (parameters sorted, empty ones left out). `None` when the dashboard
/// starts on the grouped view instead.
fn server_list_url(settings: &serde_json::Value) -> Option<String> {
    let settings = settings.get("settings")?;

    if settings
        .get("dashboard::start_on_grouped_servers")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return None;
    }

    let other = settings
        .get("dashboard::server_list_show_others")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let per_page = settings
        .get("table::per_page")
        .and_then(|value| value.as_i64())
        .filter(|per_page| (1..=100).contains(per_page))
        .unwrap_or(DEFAULT_PER_PAGE);

    Some(format!(
        "/api/client/servers?other={other}&page=1&per_page={per_page}"
    ))
}

/// The answers to `urls`, asked in parallel; only quick, complete, successful JSON ones.
async fn fetch_all(
    state: &shared::State,
    parts: &Parts,
    urls: &[String],
) -> Vec<(String, Vec<u8>)> {
    let requests = urls.iter().map(|url| async move {
        let mut request = Request::get(url.as_str()).body(Body::empty()).ok()?;
        let headers = request.headers_mut();
        for name in FORWARDED_HEADERS {
            for value in parts.headers.get_all(name) {
                headers.append(name.clone(), value.clone());
            }
        }
        for name in FORWARDED_CUSTOM_HEADERS {
            for value in parts.headers.get_all(*name) {
                headers.append(*name, value.clone());
            }
        }
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        if let Some(connect_info) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            request.extensions_mut().insert(*connect_info);
        }

        let response = tokio::time::timeout(ANSWER_TIMEOUT, state.send_router_oneshot(request))
            .await
            .ok()?
            .ok()?;

        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"));
        if response.status() != StatusCode::OK
            || !is_json
            || response.headers().contains_key(header::CONTENT_ENCODING)
        {
            return None;
        }

        let body = tokio::time::timeout(
            ANSWER_TIMEOUT,
            axum::body::to_bytes(response.into_body(), MAX_ANSWER),
        )
        .await
        .ok()?
        .ok()?;

        Some((url.clone(), body.to_vec()))
    });

    futures_util::future::join_all(requests)
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// JSON safe inside `<script>`: no `<` can open `</script>` or `<!--`. `<` is the same
/// character to `JSON.parse`.
fn escape_for_script(json: &str) -> String {
    json.replace('<', "\\u003c")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_pages_name_their_server() {
        assert_eq!(server_of("/server/65db861d"), Some("65db861d"));
        assert_eq!(
            server_of("/server/65db861d-40cf-4789-b0cf-3d44c7be6fe5/files"),
            Some("65db861d-40cf-4789-b0cf-3d44c7be6fe5")
        );
        assert_eq!(server_of("/server/"), None);
        assert_eq!(server_of("/server/../admin"), None);
        assert_eq!(server_of("/servers"), None);
    }

    #[test]
    fn the_server_list_follows_the_accounts_settings() {
        let url = |settings: serde_json::Value| {
            server_list_url(&serde_json::json!({ "settings": settings }))
        };

        assert_eq!(
            url(serde_json::json!({})).as_deref(),
            Some("/api/client/servers?other=false&page=1&per_page=25")
        );
        assert_eq!(
            url(serde_json::json!({"table::per_page": 50, "dashboard::server_list_show_others": true}))
                .as_deref(),
            Some("/api/client/servers?other=true&page=1&per_page=50")
        );
        assert_eq!(
            url(serde_json::json!({"table::per_page": 500})).as_deref(),
            Some("/api/client/servers?other=false&page=1&per_page=25")
        );
        assert_eq!(
            url(serde_json::json!({"dashboard::start_on_grouped_servers": true})),
            None
        );
    }

    #[test]
    fn inlined_answers_lose_their_preload() {
        let html = r#"<head><link rel="preload" href="/api/icons" as="fetch" crossorigin/>
<link rel="preload" href="/api/settings" as="fetch" crossorigin/></head>"#;

        assert_eq!(
            without_preloads(html, &["/api/settings"]),
            "<head><link rel=\"preload\" href=\"/api/icons\" as=\"fetch\" crossorigin/>\n</head>"
        );
    }

    #[test]
    fn nothing_closes_the_script() {
        let escaped = escape_for_script(r#"{"name":"</script><!--x"}"#);
        assert!(!escaped.contains('<'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&escaped).unwrap()["name"],
            "</script><!--x"
        );
    }
}
