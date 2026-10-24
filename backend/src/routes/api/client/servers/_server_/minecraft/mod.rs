// plugged.host: the Minecraft pages of a Minecraft server (shared/src/minecraft/,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::router::OpenApiRouter;

mod content;
mod players;
mod settings;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/content", content::router(state))
        .nest("/settings", settings::router(state))
        .nest("/players", players::router(state))
        .with_state(state.clone())
}
