use super::State;
use utoipa_axum::router::OpenApiRouter;

mod command;
mod custom_command; // plugged.host
mod docker_image;
mod preview; // plugged.host
mod variables;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/variables", variables::router(state))
        .nest("/docker-image", docker_image::router(state))
        .nest("/command", command::router(state))
        .nest("/custom-command", custom_command::router(state)) // plugged.host
        .nest("/preview", preview::router(state)) // plugged.host
        .with_state(state.clone())
}
