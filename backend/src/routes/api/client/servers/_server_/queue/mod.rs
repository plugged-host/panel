// plugged.host: a server's place in the start queue (shared/src/start_queue.rs,
// docs/features/start-queue.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod transfer;

/// Whoever may start or restart the server sees its place and may leave.
fn may_queue(
    permissions: &shared::models::user::PermissionManager,
) -> Result<(), shared::response::ApiResponse> {
    permissions
        .has_server_permission("control.start")
        .or_else(|_| permissions.has_server_permission("control.restart"))
}

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        queue: shared::start_queue::ApiStartQueue,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
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
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        super::may_queue(&permissions)?;

        let can_move = permissions
            .has_server_permission("settings.transfer")
            .is_ok();

        ApiResponse::new_serialized(Response {
            queue: shared::start_queue::view(&state, &server, &user, can_move).await?,
        })
        .ok()
    }
}

mod delete {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
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
        super::may_queue(&permissions)?;

        if !shared::start_queue::leave(&state, server.uuid).await? {
            return ApiResponse::error("the server is not waiting in the start queue")
                .with_status(StatusCode::CONFLICT)
                .ok();
        }

        activity_logger
            .log("server:queue.leave", serde_json::json!({}))
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(delete::route))
        .nest("/transfer", transfer::router(state))
        .with_state(state.clone())
}
