//! plugged.host: `/up`, the health check the Pelican panel answers and the status page probes
//! (docs/features/status.md): 200 while the panel can reach its database, else 503. Without it
//! the frontend's fallback answered 200 even with the database down.

use axum::{
    Router,
    extract::State as AxumState,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use std::time::Duration;

/// A database slower than this counts as down.
const TIMEOUT: Duration = Duration::from_secs(3);

pub fn router(state: &shared::State) -> Router {
    Router::new()
        .route("/up", get(up))
        .with_state(state.clone())
}

async fn up(AxumState(state): AxumState<shared::State>) -> impl IntoResponse {
    let database = tokio::time::timeout(
        TIMEOUT,
        sqlx::query("SELECT 1").execute(state.database.read()),
    )
    .await;

    let (status, body) = match database {
        Ok(Ok(_)) => (StatusCode::OK, "ok"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "database unavailable"),
    };

    (
        status,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
}
