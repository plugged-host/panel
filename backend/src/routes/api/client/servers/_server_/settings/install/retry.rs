// plugged.host: starts an install that is stuck over (shared/src/stuck_installs.rs,
// docs/features/server-settings.md#stuck-installs).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        stuck_installs::RetryError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.install")?;

        match shared::stuck_installs::retry(&state, &server).await {
            Ok(()) => {}
            Err(RetryError::NotStuck) => {
                return ApiResponse::error("the install is not stuck")
                    .with_status(StatusCode::CONFLICT)
                    .ok();
            }
            Err(RetryError::Failed(err)) => {
                tracing::warn!(server = %server.uuid, "failed to start a stuck install over: {err:?}");

                let (err, status) = shared::response::extract_readable_message(&err)
                    .unwrap_or_else(|| (err.to_string(), StatusCode::EXPECTATION_FAILED));

                return ApiResponse::error(format!("failed to install the server again: {err}"))
                    .with_status(status)
                    .ok();
            }
        }

        activity_logger
            .log("server:settings.install-retry", serde_json::json!({}))
            .await;

        ApiResponse::new_serialized(Response {})
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
