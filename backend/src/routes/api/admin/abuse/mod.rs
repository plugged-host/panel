//! plugged.host: Admin › Abuse (shared/src/abuse.rs, docs/features/abuse.md).

use super::State;
use utoipa_axum::router::OpenApiRouter;

mod findings;
mod scan;

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
        .nest("/findings", findings::router(state))
        .nest("/scan", scan::router(state))
        .with_state(state.clone())
}
