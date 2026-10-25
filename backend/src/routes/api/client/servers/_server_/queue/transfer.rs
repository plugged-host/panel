// plugged.host: the start queue page's move to a quieter node (shared/src/start_queue.rs,
// docs/features/start-queue.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_transfers::TransferError,
        start_queue::MoveError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        request: shared::server_transfers::ApiTransferRequest,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
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
        activity_logger: GetServerActivityLogger,
    ) -> ApiResponseResult {
        super::super::may_queue(&permissions)?;
        permissions.has_server_permission("settings.transfer")?;

        let (node, request) =
            match shared::start_queue::take_move(&state, &server, &user, true).await {
                Ok(moved) => moved,
                Err(MoveError::Transfer(TransferError::Other(err))) => return Err(err.into()),
                Err(err) => {
                    return ApiResponse::error(&err.message())
                        .with_status(
                            StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                        )
                        .ok();
                }
            };

        activity_logger
            .log(
                "server:transfer.request",
                serde_json::json!({
                    "location_uuid": node.location.uuid,
                    "node_uuid": node.uuid,
                    "from_queue": true,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { request })
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
