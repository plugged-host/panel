use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        earn::{ApiRedeem, EarnError, Redemption},
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use std::collections::HashMap;
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The claim's nonce, from the return URL (AdFoc.us: `<nonce>.<proof>`).
        #[garde(length(min = 1, max = 128))]
        claim: String,
        /// The return URL's other query parameters.
        #[garde(length(max = 20))]
        #[serde(default)]
        params: HashMap<String, String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        redemption: ApiRedeem,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
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

        permissions.has_user_permission("economy.earn")?;

        let redemption = match shared::earn::redeem(&state, &user, &data.claim, &data.params).await
        {
            Ok(redemption) => redemption,
            Err(EarnError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        };

        if redemption.status == Redemption::Granted {
            activity_logger
                .log(
                    "account:tokens.earn",
                    serde_json::json!({
                        "provider": redemption.provider,
                        "amount": redemption.amount,
                    }),
                )
                .await;
        }

        ApiResponse::new_serialized(Response { redemption }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
