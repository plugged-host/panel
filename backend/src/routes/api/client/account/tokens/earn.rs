use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetIp, GetState,
        earn::{ApiEarnStart, EarnError, Provider},
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        provider: Provider,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        earn: ApiEarnStart,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        ip: GetIp,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.earn")?;

        match shared::earn::start(&state, &user, data.provider, ip.0).await {
            Ok(earn) => ApiResponse::new_serialized(Response { earn }).ok(),
            Err(EarnError::Other(err)) => Err(err.into()),
            Err(err) => ApiResponse::error(err.message())
                .with_status(StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST))
                .ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
