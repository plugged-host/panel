use super::State;
use utoipa_axum::router::OpenApiRouter;

mod auto_kill;
mod auto_start;
mod icon; // plugged.host
mod install;
mod notifications; // plugged.host
mod rename;
mod resize; // plugged.host
mod software; // plugged.host
mod timezone;
mod trash;
mod usage; // plugged.host

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/rename", rename::router(state))
        .nest("/icon", icon::router(state)) // plugged.host
        .nest("/install", install::router(state))
        .nest("/timezone", timezone::router(state))
        .nest("/auto-kill", auto_kill::router(state))
        .nest("/auto-start", auto_start::router(state))
        .nest("/trash", trash::router(state))
        .nest("/resize", resize::router(state)) // plugged.host
        .nest("/software", software::router(state)) // plugged.host
        .nest("/usage", usage::router(state)) // plugged.host
        .nest("/notifications", notifications::router(state)) // plugged.host
        .with_state(state.clone())
}
