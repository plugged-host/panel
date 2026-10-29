// plugged.host: a membership bought for another account (docs/features/economy.md#gifts).

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
        /// The username or email of the account it is for, exactly.
        #[garde(length(chars, min = 1, max = 255))]
        recipient: compact_str::CompactString,
        #[garde(skip)]
        membership: MembershipKind,
        /// The price the buyer was shown; 409 when it moved.
        #[garde(inner(range(min = 0)))]
        #[serde(default)]
        price: Option<i64>,
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
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
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

        let gift = match shared::economy::gift_membership(
            &state,
            &user,
            &data.recipient,
            data.membership,
            data.price,
        )
        .await
        {
            Ok(gift) => gift,
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
                "account:membership.gift",
                serde_json::json!({
                    "membership": data.membership,
                    "recipient": gift.recipient.username,
                    "days": gift.offer.days,
                    "cost": gift.offer.cost,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            economy: gift.economy,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
