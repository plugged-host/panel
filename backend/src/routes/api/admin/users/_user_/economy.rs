// plugged.host: a user's credits, tokens and memberships for admins (docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::routes::api::admin::users::_user_::GetParamUser;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        economy::ApiAdminEconomy,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        economy: ApiAdminEconomy,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "user" = uuid::Uuid,
            description = "The user ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetParamUser,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("users.economy")?;

        ApiResponse::new_serialized(Response {
            economy: shared::economy::admin_view(&state, &user).await?,
        })
        .ok()
    }
}

mod put {
    use crate::routes::api::admin::users::_user_::GetParamUser;
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        economy::{AdminChange, ApiAdminEconomy, EconomyError},
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Credits to add (negative takes away). Without noprofit.space only.
        #[garde(inner(range(min = -100_000_000, max = 100_000_000)))]
        credits: Option<i64>,
        #[garde(inner(range(min = -100_000_000, max = 100_000_000)))]
        tokens: Option<i64>,
        /// Shown in the user's history.
        #[garde(inner(length(chars, max = 200)))]
        reason: Option<String>,
        #[garde(skip)]
        plus: Option<bool>,
        /// `null` ends it now.
        #[garde(skip)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        plusplus_until: Option<Option<chrono::DateTime<chrono::Utc>>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        economy: ApiAdminEconomy,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "user" = uuid::Uuid,
            description = "The user ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetParamUser,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("users.economy")?;
        permissions.can_modify_user(&user)?;

        let logged = serde_json::json!({
            "uuid": user.uuid,
            "credits": data.credits,
            "tokens": data.tokens,
            "reason": data.reason,
            "plus": data.plus,
            "plusplus_until": data.plusplus_until,
        });

        match shared::economy::admin_change(
            &state,
            &user,
            AdminChange {
                credits: data.credits,
                tokens: data.tokens,
                reason: data.reason,
                plus: data.plus,
                plusplus_until: data.plusplus_until,
            },
        )
        .await
        {
            Ok(()) => {}
            Err(EconomyError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        }

        activity_logger.log("user:economy", logged).await;

        ApiResponse::new_serialized(Response {
            economy: shared::economy::admin_view(&state, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
