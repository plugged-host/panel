// plugged.host: VS Code in the browser, the panel's half of the edge (docs/features/devboxes.md):
//
// 1. "Open in browser" on the server's VS Code page makes a one-time ticket and sends the browser
//    to https://<slug>.<web domain>/_devbox/login?ticket=… (shared::devboxes::web::login_url)
// 2. `login` (reached through the edge, so its cookie lands on the devbox's own host) redeems it
//    and sets a host-only `__Host-` cookie for 12 hours.
// 3. `authorize` answers nginx's auth_request for every request: the cookie still belongs to that
//    host, the devbox runs and the person may still use it; it hands nginx the devbox's address.
//
// The edge sends neither the panel's cookies nor a request body here, and these answers set none
// of the panel's cookies.
use super::State;
use axum::http::HeaderMap;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The devbox host the edge asks about.
fn devbox_host(headers: &HeaderMap) -> String {
    headers
        .get("x-devbox-host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

mod login {
    pub mod get {
        use axum::{body::Body, extract::Query, http::HeaderMap, http::StatusCode};
        use serde::Deserialize;
        use shared::{
            GetState,
            devboxes::{Context, Devbox, codes},
            response::{ApiResponse, ApiResponseResult},
        };

        #[derive(Deserialize)]
        pub struct Params {
            #[serde(default)]
            ticket: String,
        }

        /// The page for a link that is used up or expired.
        fn denied(context: &Context) -> ApiResponseResult {
            let panel = html_escape(&context.app_url);
            let name = html_escape(&format!("{} panel", context.app_name).trim().to_string());

            ApiResponse::new(Body::from(format!(
                "<!doctype html><meta charset=\"utf-8\"><title>Devbox</title><p>This link is used up or expired. Open the devbox again from the <a href=\"{panel}\">{name}</a>.</p>"
            )))
            .with_status(StatusCode::UNAUTHORIZED)
            .with_header("Content-Type", "text/html; charset=utf-8")
            .with_header("Cache-Control", "no-store")
            .ok()
        }

        fn html_escape(value: &str) -> String {
            value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        }

        #[utoipa::path(get, path = "/", responses(
            (status = FOUND, description = "Signed in on the devbox's host"),
            (status = UNAUTHORIZED, description = "The ticket is used up or expired", body = String, content_type = "text/html"),
        ), params(
            ("ticket" = String, Query, description = "The one-time ticket from the panel"),
        ))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            Query(params): Query<Params>,
        ) -> ApiResponseResult {
            let context = Context::load(&state).await?;
            let host = super::super::devbox_host(&headers);
            let Some(ticket) = codes::redeem(&state, &params.ticket).await else {
                return denied(&context);
            };
            if ticket.host != host {
                return denied(&context);
            }
            let Some(devbox) = Devbox::by_uuid(&state.database, ticket.devbox).await? else {
                return denied(&context);
            };
            if devbox.web_host(&context.devboxes) != host
                || !shared::devboxes::web::allowed(&state, &context, &devbox, ticket.user).await?
            {
                return denied(&context);
            }

            let value = codes::Session::new(&ticket).seal(&state).await?;
            // a relative Location: the browser stays on the devbox's host
            ApiResponse::new(Body::empty())
                .with_status(StatusCode::FOUND)
                .with_header("Location", "/")
                .with_header("Cache-Control", "no-store")
                .with_header(
                    "Set-Cookie",
                    format!(
                        "{}={value}; Path=/; Max-Age={}; Secure; HttpOnly; SameSite=Lax",
                        codes::COOKIE,
                        codes::SESSION_HOURS * 3600
                    ),
                )
                .ok()
        }
    }
}

mod authorize {
    pub mod get {
        use axum::{body::Body, http::HeaderMap, http::StatusCode};
        use shared::{
            GetState,
            devboxes::{Context, Devbox, codes},
            response::{ApiResponse, ApiResponseResult},
        };

        /// Seconds an answer for one devbox and person is remembered (nginx caches it as long).
        const REMEMBER_SECONDS: u64 = 30;

        fn refused() -> ApiResponseResult {
            ApiResponse::new(Body::empty())
                .with_status(StatusCode::UNAUTHORIZED)
                .with_header("Cache-Control", "no-store")
                .ok()
        }

        #[utoipa::path(get, path = "/", responses(
            (status = NO_CONTENT, description = "Allowed; `X-Devbox-Upstream` is where the devbox listens"),
            (status = UNAUTHORIZED, description = "Not allowed"),
        ))]
        pub async fn route(state: GetState, headers: HeaderMap) -> ApiResponseResult {
            let context = Context::load(&state).await?;
            let host = super::super::devbox_host(&headers);
            let Some(session) = codes::Session::open(
                &state,
                super::super::cookie_value(&headers, codes::COOKIE).unwrap_or_default(),
                &host,
            )
            .await
            else {
                return refused();
            };
            let Some(devbox) = Devbox::by_uuid(&state.database, session.devbox).await? else {
                return refused();
            };
            if devbox.web_host(&context.devboxes) != host {
                return refused();
            }

            let key = format!("devboxes::web_ok::{}::{}", devbox.uuid, session.user);
            let upstream = match state.cache.get::<String>(&key).await.ok().flatten() {
                Some(upstream) => Some(upstream),
                None if shared::devboxes::web::allowed(&state, &context, &devbox, session.user)
                    .await? =>
                {
                    let upstream = shared::devboxes::web::upstream(&state, &devbox).await?;
                    if let Some(upstream) = &upstream {
                        state.cache.set(&key, REMEMBER_SECONDS, upstream).await.ok();
                    }
                    upstream
                }
                None => None,
            };
            let Some(upstream) = upstream else {
                return refused();
            };

            ApiResponse::new(Body::empty())
                .with_status(StatusCode::NO_CONTENT)
                .with_header("X-Devbox-Upstream", upstream)
                .with_header("Cache-Control", "no-store")
                .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/login",
            OpenApiRouter::new()
                .routes(routes!(login::get::route))
                .with_state(state.clone()),
        )
        .nest(
            "/authorize",
            OpenApiRouter::new()
                .routes(routes!(authorize::get::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
