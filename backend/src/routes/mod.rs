use utoipa_axum::router::OpenApiRouter;

pub mod api;
mod data; // plugged.host
pub mod docs; // plugged.host: the API documentation pages
mod referral; // plugged.host: referral links (docs/features/referrals.md)
pub mod up; // plugged.host: the health check
pub use shared::{GetState, State};

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/api", api::router(state))
        .nest("/data", data::router(state)) // plugged.host: the public status files
        .nest("/r", referral::router(state)) // plugged.host: referral links
        .with_state(state.clone())
}
