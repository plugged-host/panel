//! plugged.host: the public status files under `/data` (shared/src/status.rs,
//! docs/features/status.md), in the shapes the status page and its Cloudflare Worker read.
//! Not found while the status page is off.

use super::State;
use shared::response::{ApiResponse, ApiResponseResult};
use utoipa_axum::{router::OpenApiRouter, routes};

async fn respond(
    state: &State,
    content_type: &'static str,
    body: impl std::future::Future<Output = Result<Vec<u8>, anyhow::Error>>,
) -> ApiResponseResult {
    if !state.settings.get().await?.status.enabled {
        return ApiResponse::error("not found")
            .with_status(axum::http::StatusCode::NOT_FOUND)
            .ok();
    }

    ApiResponse::new(axum::body::Body::from(body.await?))
        .with_header("Content-Type", content_type)
        .with_header("Access-Control-Allow-Origin", "*")
        .with_header("Cache-Control", "public, max-age=30")
        .ok()
}

async fn json(
    state: &State,
    value: impl std::future::Future<Output = Result<serde_json::Value, anyhow::Error>>,
) -> ApiResponseResult {
    respond(state, "application/json", async {
        Ok(serde_json::to_vec(&value.await?)?)
    })
    .await
}

mod current {
    use shared::{GetState, response::ApiResponseResult};

    #[utoipa::path(get, path = "/current.json", responses(
        (status = OK, body = serde_json::Value),
        (status = NOT_FOUND),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        super::json(&state, shared::status::current(&state)).await
    }
}

mod history {
    use shared::{GetState, response::ApiResponseResult};

    #[utoipa::path(get, path = "/history.json", responses(
        (status = OK, body = serde_json::Value),
        (status = NOT_FOUND),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        super::json(&state, shared::status::history(&state)).await
    }
}

mod incidents {
    use shared::{GetState, response::ApiResponseResult};

    #[utoipa::path(get, path = "/incidents.json", responses(
        (status = OK, body = serde_json::Value),
        (status = NOT_FOUND),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        super::json(&state, shared::status::incidents_file(&state)).await
    }
}

mod targets {
    use shared::{GetState, response::ApiResponseResult};

    #[utoipa::path(get, path = "/targets.json", responses(
        (status = OK, body = serde_json::Value),
        (status = NOT_FOUND),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        super::json(&state, shared::status::targets(&state)).await
    }
}

mod feed {
    use shared::{GetState, response::ApiResponseResult};

    #[utoipa::path(get, path = "/feed.xml", responses(
        (status = OK, body = String),
        (status = NOT_FOUND),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        super::respond(&state, "application/rss+xml; charset=utf-8", async {
            Ok(shared::status::feed(&state).await?.into_bytes())
        })
        .await
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(current::route))
        .routes(routes!(history::route))
        .routes(routes!(incidents::route))
        .routes(routes!(targets::route))
        .routes(routes!(feed::route))
        .with_state(state.clone())
}
