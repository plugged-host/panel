//! plugged.host: the signed-in user's standing (`shared::standing`, docs/features/standing.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// The panel's own standing, or noprofit.space's for a user it made; `None` when the one
        /// to ask does not answer.
        standing: Option<shared::standing::Standing>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        ApiResponse::new_serialized(Response {
            standing: shared::standing::of_user(&state, user.uuid).await,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
