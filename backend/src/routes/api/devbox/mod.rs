// plugged.host: what a running devbox (`ph`) asks the panel, and the browser edge's two calls
// (shared/src/devboxes, docs/features/devboxes.md).
use super::State;
use axum::http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use shared::{
    devboxes::{Context, Devbox},
    response::ApiResponse,
};
use utoipa_axum::{router::OpenApiRouter, routes};

mod game;
mod web;

fn unauthorized() -> ApiResponse {
    ApiResponse::error("unauthorized").with_status(StatusCode::UNAUTHORIZED)
}

/// The devbox a request comes from, by the bearer token of its current start (never one that is
/// stopping: its `ph` then gets a 401 and ends every session), and the settings.
async fn devbox_of(
    state: &shared::State,
    headers: &HeaderMap,
    action: &str,
    per_minute: u64,
) -> Result<(Context, Devbox), ApiResponse> {
    let context = Context::load(state).await?;
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .trim();
    if !context.ready() {
        return Err(unauthorized());
    }
    let Some(devbox) = Devbox::by_token(&state.database, token).await? else {
        return Err(unauthorized());
    };

    // many devboxes share a node's address: limits go per devbox
    state
        .cache
        .ratelimit(
            format!("devbox/{action}"),
            per_minute,
            60,
            devbox.uuid.to_string(),
        )
        .await?;

    Ok((context, devbox))
}

mod boot {
    pub mod post {
        use axum::http::HeaderMap;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            devboxes::agent::ApiAccess,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(skip)]
            version: Option<String>,
            /// `ok`, `mount_missing`, …
            #[garde(skip)]
            mount: Option<String>,
            #[garde(skip)]
            detail: Option<String>,
            /// The SSH host key's fingerprint.
            #[garde(skip)]
            host_key: Option<String>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = ApiAccess),
            (status = UNAUTHORIZED, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            let (context, devbox) = super::super::devbox_of(&state, &headers, "boot", 10).await?;

            shared::devboxes::agent::boot(
                &state,
                &devbox,
                data.version.as_deref(),
                data.mount.as_deref(),
                data.detail.as_deref(),
                data.host_key.as_deref(),
            )
            .await?;

            ApiResponse::new_serialized(
                shared::devboxes::agent::access(&state, &context, &devbox).await?,
            )
            .ok()
        }
    }
}

mod heartbeat {
    pub mod post {
        use axum::http::HeaderMap;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            devboxes::agent::ApiAccess,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// Open SSH and browser sessions.
            #[garde(skip)]
            #[serde(default)]
            connections: i64,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = ApiAccess),
            (status = UNAUTHORIZED, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            let (context, devbox) =
                super::super::devbox_of(&state, &headers, "heartbeat", 6).await?;

            shared::devboxes::agent::heartbeat(&state, &devbox, data.connections).await?;

            ApiResponse::new_serialized(
                shared::devboxes::agent::access(&state, &context, &devbox).await?,
            )
            .ok()
        }
    }
}

mod auth {
    pub mod post {
        use axum::http::HeaderMap;
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(skip)]
            #[serde(default)]
            username: String,
            /// The connect code typed as the password.
            #[garde(skip)]
            #[serde(default)]
            code: String,
            /// Where the connection comes from, for the activity log.
            #[garde(skip)]
            #[serde(default)]
            ip: String,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            ok: bool,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            // VS Code opens a few connections per window; a bot guessing codes (about 79 bits)
            // gets nowhere at this rate
            let (context, devbox) = super::super::devbox_of(&state, &headers, "auth", 20).await?;

            let ok = shared::devboxes::agent::authenticate(
                &state,
                &context,
                &devbox,
                data.username.trim(),
                &data.code,
                &data.ip,
            )
            .await?;

            ApiResponse::new_serialized(Response { ok }).ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/boot",
            OpenApiRouter::new()
                .routes(routes!(boot::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/heartbeat",
            OpenApiRouter::new()
                .routes(routes!(heartbeat::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/auth",
            OpenApiRouter::new()
                .routes(routes!(auth::post::route))
                .with_state(state.clone()),
        )
        .nest("/game", game::router(state))
        .nest("/web", web::router(state))
        .with_state(state.clone())
}
