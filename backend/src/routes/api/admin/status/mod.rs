//! plugged.host: Admin › Status (shared/src/status.rs, docs/features/status.md).

use super::State;
use utoipa_axum::router::OpenApiRouter;

mod components;
mod incidents;
mod overrides;

fn bad_request(message: impl AsRef<str>) -> shared::response::ApiResponseResult {
    shared::response::ApiResponse::error(message)
        .with_status(axum::http::StatusCode::BAD_REQUEST)
        .ok()
}

fn not_found(message: impl AsRef<str>) -> shared::response::ApiResponseResult {
    shared::response::ApiResponse::error(message)
        .with_status(axum::http::StatusCode::NOT_FOUND)
        .ok()
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/components", components::router(state))
        .nest("/incidents", incidents::router(state))
        .nest("/overrides", overrides::router(state))
        .with_state(state.clone())
}
