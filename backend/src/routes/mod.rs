use utoipa_axum::router::OpenApiRouter;

pub mod api;
mod data; // plugged.host
pub mod docs; // plugged.host: the API documentation pages
pub mod up; // plugged.host: the health check
pub use shared::{GetState, State};

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/api", api::router(state))
        .nest("/data", data::router(state)) // plugged.host: the public status files
        .with_state(state.clone())
}
