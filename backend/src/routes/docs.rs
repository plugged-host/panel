//! plugged.host: the API documentation pages the Pelican panel has (docs/features/api-docs.md).
//! `/docs/api` lists the APIs, `/docs/api/{client,admin}` shows one in Stoplight Elements and
//! `/docs/api/{client,admin}.json` is its OpenAPI document, cut from the one the routes generate
//! (`/openapi.json`), so it is always the running panel's. For signed-in users, as on Pelican.

use axum::{
    Router,
    extract::{Path, State as AxumState},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

/// Stoplight Elements as Pelican's docs load it (Scramble), pinned with subresource integrity.
const ELEMENTS_JS: &str = "https://unpkg.com/@stoplight/elements@8.4.2/web-components.min.js";
const ELEMENTS_JS_SRI: &str =
    "sha384-JQMvMNVvV8QXHXbrUyttKUojmPo2SI/3ztEsfv2seW/Gdqh8uvfVZSc/8rpCSYP4";
const ELEMENTS_CSS: &str = "https://unpkg.com/@stoplight/elements@8.4.2/styles.min.css";
const ELEMENTS_CSS_SRI: &str =
    "sha384-4SEI8azIKLxyKQW9QIuPrsJaVzrXaKHhlf82/u3KR1Cw5T9lYqsgzg+4MWygH2N2";

/// The documented APIs: (page name, title, what it is for, the paths it covers).
const APIS: &[(&str, &str, &str, &str)] = &[
    (
        "client",
        "Client API",
        "What a user can do with their own account and servers: the same requests the panel makes.",
        "/api/client",
    ),
    (
        "admin",
        "Admin API",
        "Managing the panel: users, servers, nodes, eggs, locations and settings, for accounts with the matching admin permissions.",
        "/api/admin",
    ),
];

struct Docs {
    app_name: String,
    /// Each API's OpenAPI document, serialized once.
    specs: HashMap<&'static str, Arc<Vec<u8>>>,
}

type DocsState = Arc<(shared::State, Docs)>;

/// The docs routes for `openapi` (the panel's whole document).
pub fn router(
    state: &shared::State,
    openapi: &utoipa::openapi::OpenApi,
    app_name: String,
) -> Result<Router, anyhow::Error> {
    let whole = serde_json::to_value(openapi)?;
    let mut specs = HashMap::new();
    for (name, title, _, prefix) in APIS {
        specs.insert(
            *name,
            Arc::new(serde_json::to_vec(&only(
                &whole,
                prefix,
                &format!("{app_name} {title}"),
            ))?),
        );
    }

    let docs: DocsState = Arc::new((state.clone(), Docs { app_name, specs }));

    Ok(Router::new()
        .route("/docs/api", get(index))
        .route("/docs/api/", get(index))
        .route("/docs/api/{page}", get(page))
        .with_state(docs))
}

/// The paths under `prefix`, with only the components they use.
fn only(whole: &serde_json::Value, prefix: &str, title: &str) -> serde_json::Value {
    let mut spec = whole.clone();

    if let Some(paths) = spec
        .get_mut("paths")
        .and_then(|paths| paths.as_object_mut())
    {
        paths.retain(|path, _| path == prefix || path.starts_with(&format!("{prefix}/")));
    }
    if let Some(info) = spec.get_mut("info") {
        info["title"] = title.into();
    }

    // the components the paths name, and those the components name, until nothing new turns up
    let mut used = BTreeSet::new();
    let mut pending = Vec::new();
    refs(
        spec.get("paths").unwrap_or(&serde_json::Value::Null),
        &mut pending,
    );
    while let Some(reference) = pending.pop() {
        if !used.insert(reference.clone()) {
            continue;
        }
        if let Some(component) = reference
            .strip_prefix("#/")
            .and_then(|pointer| whole.pointer(&format!("/{pointer}")))
        {
            refs(component, &mut pending);
        }
    }

    if let Some(components) = spec.get_mut("components").and_then(|c| c.as_object_mut()) {
        for (kind, entries) in components.iter_mut() {
            if kind == "securitySchemes" {
                continue;
            }
            if let Some(entries) = entries.as_object_mut() {
                entries.retain(|name, _| used.contains(&format!("#/components/{kind}/{name}")));
            }
        }
    }

    spec
}

fn refs(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("$ref", serde_json::Value::String(reference)) => found.push(reference.clone()),
                    _ => refs(value, found),
                }
            }
        }
        serde_json::Value::Array(values) => values.iter().for_each(|value| refs(value, found)),
        _ => {}
    }
}

/// Whether the request comes from a signed-in, not suspended user.
async fn signed_in(docs: &DocsState, headers: &HeaderMap) -> bool {
    let state = &docs.0;
    let Ok(settings) = state.settings.get().await else {
        return false;
    };
    let session_cookie = settings.app.session_cookie.to_string();
    drop(settings);

    let Some(session) = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|cookie| {
            cookie
                .trim()
                .strip_prefix(session_cookie.as_str())?
                .strip_prefix('=')
        })
    else {
        return false;
    };

    matches!(
        shared::models::user::User::by_session_cached(&state.database, session).await,
        Ok(Some((user, _))) if !user.suspended
    )
}

fn html(body: String, csp: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "private, no-cache"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
            (header::CONTENT_SECURITY_POLICY, csp),
        ],
        body,
    )
        .into_response()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const PAGE_STYLE: &str = "html{color-scheme:dark}body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:#0b0d10;color:#e6e8ee;font:15px/1.5 system-ui,sans-serif}main{max-width:28rem;padding:2rem 1.5rem}h1{font-size:1.25rem;margin:0 0 1.25rem}a{color:#7c9cff;text-decoration:none}a:hover{text-decoration:underline}ul{list-style:none;margin:0;padding:0}li{margin:0 0 1rem}li p{margin:.25rem 0 0;color:#9aa3b2;font-size:.875rem}.note{color:#9aa3b2;font-size:.875rem}";
const PAGE_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";

async fn index(AxumState(docs): AxumState<DocsState>, headers: HeaderMap) -> Response {
    let title = format!("{} API Documentation", docs.1.app_name);
    let content = if signed_in(&docs, &headers).await {
        let items: String = APIS
            .iter()
            .map(|(name, title, about, _)| {
                format!(
                    "<li><a href=\"/docs/api/{name}\">{}</a><p>{}</p></li>",
                    escape(title),
                    escape(about)
                )
            })
            .collect();
        format!(
            "<ul>{items}</ul><p class=\"note\">Requests take an API key from Account › API Keys as <code>Authorization: Bearer &lt;key&gt;</code>. The machine-readable documents: <a href=\"/docs/api/client.json\">client.json</a>, <a href=\"/docs/api/admin.json\">admin.json</a>.</p>"
        )
    } else {
        "<p class=\"note\">You need to be signed in to view the API documentation. <a href=\"/auth/login\">Sign in</a></p>".into()
    };

    html(
        format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{0}</title><style>{PAGE_STYLE}</style></head><body><main><h1>{0}</h1>{content}</main></body></html>",
            escape(&title)
        ),
        PAGE_CSP,
    )
}

async fn page(
    AxumState(docs): AxumState<DocsState>,
    Path(page): Path<String>,
    headers: HeaderMap,
) -> Response {
    // Pelican's name for the admin API, for links made before the move
    match page.as_str() {
        "application" => return Redirect::permanent("/docs/api/admin").into_response(),
        "application.json" => return Redirect::permanent("/docs/api/admin.json").into_response(),
        _ => {}
    }

    let (name, json) = match page.strip_suffix(".json") {
        Some(name) => (name, true),
        None => (page.as_str(), false),
    };
    let Some((name, title, _, _)) = APIS.iter().find(|(api, ..)| *api == name) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if !signed_in(&docs, &headers).await {
        return if json {
            StatusCode::UNAUTHORIZED.into_response()
        } else {
            Redirect::to("/docs/api").into_response()
        };
    }

    if json {
        return (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "private, max-age=300"),
            ],
            docs.1.specs[name].as_ref().clone(),
        )
            .into_response();
    }

    html(
        format!(
            "<!doctype html><html lang=\"en\" data-theme=\"dark\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, shrink-to-fit=no\"><meta name=\"color-scheme\" content=\"dark\"><title>{title}</title><script src=\"{ELEMENTS_JS}\" integrity=\"{ELEMENTS_JS_SRI}\" crossorigin=\"anonymous\"></script><link rel=\"stylesheet\" href=\"{ELEMENTS_CSS}\" integrity=\"{ELEMENTS_CSS_SRI}\" crossorigin=\"anonymous\"><style>html,body{{margin:0;height:100%}}body{{background-color:var(--color-canvas);height:100vh;overflow-y:hidden}}</style></head><body><elements-api apiDescriptionUrl=\"/docs/api/{name}.json\" router=\"hash\" layout=\"responsive\" tryItCredentialsPolicy=\"include\" hideSchemas=\"false\" hideTryIt=\"false\"></elements-api></body></html>",
            title = escape(&format!("{} {title}", docs.1.app_name)),
        ),
        "default-src 'self'; script-src 'self' https://unpkg.com; style-src 'self' 'unsafe-inline' https://unpkg.com; img-src * data: blob:; font-src 'self' data: https:; connect-src 'self'; worker-src blob:; base-uri 'self'; form-action 'self'; frame-ancestors 'self'",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_api_keeps_its_paths_and_the_components_they_use() {
        let whole = serde_json::json!({
            "info": {"title": "x"},
            "paths": {
                "/api/client/servers": {"get": {"responses": {"200": {"$ref": "#/components/schemas/Servers"}}}},
                "/api/clients": {"get": {}},
                "/api/admin/users": {"get": {"responses": {"200": {"$ref": "#/components/schemas/Users"}}}},
            },
            "components": {
                "schemas": {
                    "Servers": {"items": {"$ref": "#/components/schemas/Server"}},
                    "Server": {"type": "object"},
                    "Users": {"type": "object"},
                },
                "securitySchemes": {"cookie": {}},
            },
        });

        let client = only(&whole, "/api/client", "Panel Client API");
        let paths: Vec<_> = client["paths"].as_object().unwrap().keys().collect();
        assert_eq!(paths, ["/api/client/servers"]);
        let schemas: BTreeSet<_> = client["components"]["schemas"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(schemas, BTreeSet::from(["Server".into(), "Servers".into()]));
        assert!(
            client["components"]["securitySchemes"]
                .get("cookie")
                .is_some()
        );
        assert_eq!(client["info"]["title"], "Panel Client API");
    }
}
