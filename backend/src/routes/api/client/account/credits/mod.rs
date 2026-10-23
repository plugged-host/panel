// plugged.host: the Resource Store: credits and the CPU, memory and disk they buy
// (docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        economy::{ApiStore, EconomyError},
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        store: ApiStore,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.read")?;

        match shared::economy::store(&state, &user).await {
            Ok(store) => ApiResponse::new_serialized(Response { store }).ok(),
            Err(EconomyError::Other(err)) => Err(err.into()),
            Err(err) => ApiResponse::error(err.message())
                .with_status(
                    axum::http::StatusCode::from_u16(err.status())
                        .unwrap_or(axum::http::StatusCode::BAD_REQUEST),
                )
                .ok(),
        }
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        economy::{ApiStore, EconomyError},
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        pricing::StoreResource,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        resource: StoreResource,
        #[garde(range(min = 1, max = 50))]
        steps: i64,
        /// The price the player was shown.
        #[garde(range(min = 0))]
        price: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        store: ApiStore,
    }

    #[utoipa::path(post, path = "/store", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = UNPROCESSABLE_ENTITY, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_user_permission("economy.purchase")?;

        let store =
            match shared::economy::purchase(&state, &user, data.resource, data.steps, data.price)
                .await
            {
                Ok(store) => store,
                Err(EconomyError::Other(err)) => return Err(err.into()),
                Err(err) => {
                    return ApiResponse::error(err.message())
                        .with_status(
                            StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                        )
                        .ok();
                }
            };

        activity_logger
            .log(
                "account:store.purchase",
                serde_json::json!({
                    "resource": data.resource,
                    "steps": data.steps,
                    "price": data.price,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { store }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
