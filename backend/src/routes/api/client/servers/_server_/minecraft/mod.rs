// plugged.host: the Minecraft pages of a Minecraft server (shared/src/minecraft/,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::router::OpenApiRouter;

mod content;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/content", content::router(state))
        .with_state(state.clone())
}
