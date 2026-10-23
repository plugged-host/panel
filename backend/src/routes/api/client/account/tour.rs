//! plugged.host: the signed-in user's welcome tour (`shared::welcome_tour`,
//! docs/features/welcome-tour.md).

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
        tour: shared::welcome_tour::ApiTour,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        ApiResponse::new_serialized(Response {
            tour: shared::welcome_tour::get(&state, user.uuid).await?,
        })
        .ok()
    }
}

mod post {
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
        welcome_tour::TourAction,
    };
    use utoipa::ToSchema;

    fn first_step() -> i32 {
        1
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        action: TourAction,
        /// The step reached, for `advance` and `advance_server`.
        #[garde(range(min = 1, max = 50))]
        #[serde(default = "first_step")]
        step: i32,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        tour: shared::welcome_tour::ApiTour,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        user: GetUser,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        ApiResponse::new_serialized(Response {
            tour: shared::welcome_tour::act(&state, user.uuid, data.action, data.step).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
