// plugged.host: the Minecraft pages of a Minecraft server (shared/src/minecraft/,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::router::OpenApiRouter;

mod bedrock;
mod content;
mod log_check;
mod modpacks;
mod onboarding;
mod players;
mod settings;
mod worlds;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/bedrock", bedrock::router(state))
        .nest("/content", content::router(state))
        .nest("/log-check", log_check::router(state))
        .nest("/modpacks", modpacks::router(state))
        .nest("/onboarding", onboarding::router(state))
        .nest("/settings", settings::router(state))
        .nest("/players", players::router(state))
        .nest("/worlds", worlds::router(state))
        .with_state(state.clone())
}
