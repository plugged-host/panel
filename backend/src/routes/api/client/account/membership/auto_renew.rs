// plugged.host: a membership renewing by itself, switched on or off per membership
// (docs/features/economy.md#auto-renew).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        economy::{ApiEconomy, EconomyError, MembershipKind},
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        membership: MembershipKind,
        #[garde(skip)]
        enabled: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        economy: ApiEconomy,
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

        // it spends the balance later, like a purchase
        permissions.has_user_permission("economy.purchase")?;

        let economy =
            match shared::economy::set_auto_renew(&state, &user, data.membership, data.enabled)
                .await
            {
                Ok(economy) => economy,
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
                "account:membership.auto-renew",
                serde_json::json!({ "membership": data.membership, "enabled": data.enabled }),
            )
            .await;

        ApiResponse::new_serialized(Response { economy }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
