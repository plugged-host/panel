//! plugged.host: a node's failover: its servers restored on other nodes of its location from their
//! newest backup (shared/src/node_failover.rs, docs/features/failover.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod cancel;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, user::GetPermissionManager},
        node_failover::AdminApiNodeFailover,
        response::{ApiResponse, ApiResponseResult},
        settings::recovery::FailoverMode,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Admin › Settings › Hosting › Recovery: whether failovers can be started at all.
        mode: FailoverMode,
        failovers: Vec<AdminApiNodeFailover>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
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
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        let mode = state.settings.get().await?.recovery.failover_mode;

        ApiResponse::new_serialized(Response {
            mode,
            failovers: shared::node_failover::recent(&state.database, node.uuid).await?,
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
        models::{
            admin_activity::GetAdminActivityLogger,
            node::GetNode,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// A drill: only this server, which is stopped on the node first. Without it, all the
        /// node's servers.
        #[garde(skip)]
        #[serde(default)]
        drill_server_uuid: Option<uuid::Uuid>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        failover: uuid::Uuid,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
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
        user: GetUser,
        node: GetNode,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.failover")?;

        let failover = match shared::node_failover::start(
            &state,
            &node,
            Some(user.uuid),
            data.drill_server_uuid,
        )
        .await
        {
            Ok(failover) => failover,
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
                "node:failover",
                serde_json::json!({
                    "uuid": node.uuid,
                    "name": node.name,
                    "drill_server_uuid": data.drill_server_uuid,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { failover }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .nest("/cancel", cancel::router(state))
        .with_state(state.clone())
}
