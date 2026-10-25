//! plugged.host: a failover's dry run: where each server of the node would go and from which backup, without moving anything (docs/features/failover.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, user::GetPermissionManager},
        node_failover::AdminApiFailoverPlan,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        /// Only this server, as a drill would. Without it, all the node's servers.
        #[serde(default)]
        drill_server_uuid: Option<uuid::Uuid>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        plan: Vec<AdminApiFailoverPlan>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "drill_server_uuid" = Option<uuid::Uuid>, Query,
            description = "Only this server of the node",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        let plan = match shared::node_failover::plan(&state, &node, params.drill_server_uuid).await
        {
            Ok(plan) => plan,
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status())
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    )
                    .ok();
            }
        };

        ApiResponse::new_serialized(Response { plan }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
