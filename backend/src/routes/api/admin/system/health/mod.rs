use super::State;
use utoipa_axum::router::OpenApiRouter;

mod general;
mod nodes;
mod tasks; // plugged.host
mod telemetry; // plugged.host

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/general", general::router(state))
        .nest("/nodes", nodes::router(state))
        .nest("/tasks", tasks::router(state)) // plugged.host
        .nest("/telemetry", telemetry::router(state)) // plugged.host
        .with_state(state.clone())
}
