// plugged.host: cancelling a player's move (shared/src/server_transfers.rs).
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
        server_transfers::TransferError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
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
        permissions.has_server_permission("settings.transfer")?;

        let admin = permissions.has_admin_permission("servers.transfer").is_ok();
        match shared::server_transfers::cancel(&state, &server, admin).await {
            Ok(()) => {}
            Err(TransferError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        }

        activity_logger
            .log("server:transfer.cancel", serde_json::json!({}))
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
