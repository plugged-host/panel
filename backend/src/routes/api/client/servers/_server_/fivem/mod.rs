// plugged.host: the FiveM pages of a FiveM server (shared/src/fivem/, docs/features/fivem.md).
use super::State;
use utoipa_axum::router::OpenApiRouter;

mod artifacts;
mod config;
mod players;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/artifacts", artifacts::router(state))
        .nest("/config", config::router(state))
        .nest("/players", players::router(state))
        .with_state(state.clone())
}
