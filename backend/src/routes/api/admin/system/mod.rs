use super::State;
use utoipa_axum::router::OpenApiRouter;

mod capacity; // plugged.host
mod debug;
mod email;
mod health;
mod neo; // plugged.host
mod overview;
mod telemetry;
// plugged.host: the update checker removed (docs/upstream.md)
// mod updates;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/overview", overview::router(state))
        .nest("/telemetry", telemetry::router(state))
        // .nest("/updates", updates::router(state))
        .nest("/health", health::router(state))
        .nest("/capacity", capacity::router(state)) // plugged.host
        .nest("/email", email::router(state))
        .nest("/neo", neo::router(state)) // plugged.host
        .nest("/debug", debug::router(state))
        .with_state(state.clone())
}
