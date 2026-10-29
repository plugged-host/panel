// plugged.host: the waiting room's day pass, bought with tokens (shared/src/waiting_room.rs, docs/features/ads.md#day-pass).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        economy::EconomyError,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
        waiting_room::ApiDayPass,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The price the user was shown: 409 when it moved; absent: what it costs now.
        #[garde(inner(range(min = 0)))]
        #[serde(default)]
        price: Option<i64>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        day_pass: ApiDayPass,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
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

        let day_pass = match shared::waiting_room::buy_day_pass(&state, &user, data.price).await {
            Ok(day_pass) => day_pass,
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
                "account:day-pass",
                serde_json::json!({ "cost": day_pass.tokens, "hours": day_pass.hours }),
            )
            .await;

        ApiResponse::new_serialized(Response { day_pass }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
