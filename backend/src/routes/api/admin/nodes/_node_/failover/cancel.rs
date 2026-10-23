//! plugged.host: stopping a node's failover: queued servers stay where they are, the ones under
//! way finish (docs/features/failover.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, node::GetNode, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        cancelled: u64,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
        activity_logger: GetAdminActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.failover")?;

        let cancelled = match shared::node_failover::cancel(&state, node.uuid).await {
            Ok(cancelled) => cancelled,
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status())
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    )
                    .ok();
            }
        };

        activity_logger
            .log(
                "node:failover.cancel",
                serde_json::json!({
                    "uuid": node.uuid,
                    "name": node.name,
                    "cancelled": cancelled,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { cancelled }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
