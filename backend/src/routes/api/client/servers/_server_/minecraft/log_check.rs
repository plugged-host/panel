// plugged.host: the console's automatic log check on mclo.gs (shared/src/minecraft/log_check.rs,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        minecraft::log_check::LogCheck,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Whether the panel checks logs at all.
        enabled: bool,
        result: Option<LogCheck>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
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
    ) -> ApiResponseResult {
        // the check publishes the log, so it needs what reading the log needs
        permissions.has_server_permission("control.console")?;
        permissions.has_server_permission("files.read-content")?;
        shared::minecraft::require(&server)?;

        if !shared::minecraft::log_check::enabled(&state).await {
            return ApiResponse::new_serialized(Response {
                enabled: false,
                result: None,
            })
            .ok();
        }

        match shared::minecraft::log_check::check(&state, &server).await {
            Ok(result) => ApiResponse::new_serialized(Response {
                enabled: true,
                result,
            })
            .ok(),
            Err(err) => {
                tracing::warn!(server = %server.uuid, "minecraft log check: {err:#}");

                ApiResponse::error("the log could not be checked, try again in a moment")
                    .with_status(axum::http::StatusCode::BAD_GATEWAY)
                    .ok()
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
