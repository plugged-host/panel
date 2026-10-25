// plugged.host: the panel's own monthly price, paid with tokens (shared/src/panel_access.rs).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::GetUser,
        panel_access::ApiPanelAccess,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        panel_access: ApiPanelAccess,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        ApiResponse::new_serialized(Response {
            panel_access: shared::panel_access::status(&state, &user).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        economy::EconomyError,
        models::{user::GetUser, user_activity::GetUserActivityLogger},
        panel_access::ApiPanelAccess,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        panel_access: ApiPanelAccess,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
    ) -> ApiResponseResult {
        let panel_access = match shared::panel_access::buy(&state, &user).await {
            Ok(panel_access) => panel_access,
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
                "account:panel-access",
                serde_json::json!({ "cost": panel_access.tokens, "days": panel_access.days }),
            )
            .await;

        ApiResponse::new_serialized(Response { panel_access }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
