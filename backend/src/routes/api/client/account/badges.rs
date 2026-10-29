// plugged.host: profile badges, bought with tokens (shared/src/badges.rs, docs/features/promos.md#profile-badges).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// An economy refusal as the API answers it.
fn refusal(err: shared::economy::EconomyError) -> shared::response::ApiResponseResult {
    use axum::http::StatusCode;
    use shared::{economy::EconomyError, response::ApiResponse};

    match err {
        EconomyError::Other(err) => Err(err.into()),
        err => ApiResponse::error(err.message())
            .with_status(StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST))
            .ok(),
    }
}

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        badges::ApiBadges,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        badges: ApiBadges,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.read")?;

        ApiResponse::new_serialized(Response {
            badges: shared::badges::view(&state, &user).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        badges::ApiBadges,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 32))]
        badge: compact_str::CompactString,
        /// The price the user was shown: 409 when it moved; absent: what it costs now.
        #[garde(inner(range(min = 0)))]
        #[serde(default)]
        price: Option<i64>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        badges: ApiBadges,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
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

        let badges = match shared::badges::buy(&state, &user, &data.badge, data.price).await {
            Ok(badges) => badges,
            Err(err) => return super::refusal(err),
        };

        activity_logger
            .log(
                "account:badge.purchase",
                serde_json::json!({ "badge": data.badge, "cost": badges.tokens }),
            )
            .await;

        ApiResponse::new_serialized(Response { badges }).ok()
    }
}

mod put {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        badges::ApiBadges,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// One of the account's badges, or `null` to show none.
        #[garde(inner(length(chars, min = 1, max = 32)))]
        badge: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        badges: ApiBadges,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_user_permission("economy.read")?;

        match shared::badges::show(&state, &user, data.badge.as_deref()).await {
            Ok(badges) => ApiResponse::new_serialized(Response { badges }).ok(),
            Err(err) => super::refusal(err),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
