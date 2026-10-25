//! plugged.host: trying a failed server of a node's failover again (docs/features/failover.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, node::GetNode, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The failed server's row in the node's failovers.
        #[garde(skip)]
        server_failover_uuid: uuid::Uuid,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        server: uuid::Uuid,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.failover")?;

        let server =
            match shared::node_failover::resume(&state, node.uuid, data.server_failover_uuid).await
            {
                Ok(server) => server,
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
                "node:failover.resume",
                serde_json::json!({
                    "uuid": node.uuid,
                    "name": node.name,
                    "server_uuid": server,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { server }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
