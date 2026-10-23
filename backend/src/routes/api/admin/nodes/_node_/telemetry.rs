//! plugged.host: `GET /api/admin/nodes/{node}/telemetry`, the node's newest sample and its history
//! (shared/src/node_telemetry.rs).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::extract::Query;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, user::GetPermissionManager},
        node_telemetry::{AdminApiNodeTelemetry, AdminApiNodeTelemetryPoint},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        #[serde(default = "default_hours")]
        hours: i32,
    }

    fn default_hours() -> i32 {
        24
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        latest: Option<AdminApiNodeTelemetry>,
        /// Minutes each point of `history` covers.
        bucket_minutes: i32,
        history: Vec<AdminApiNodeTelemetryPoint>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "hours" = i32, Query,
            description = "How many hours of history, 1 to 168",
            example = "24",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        let hours = params
            .hours
            .clamp(1, shared::node_telemetry::RETENTION_DAYS * 24);
        let (latest, history) = tokio::try_join!(
            shared::node_telemetry::latest(&state.database, node.uuid),
            shared::node_telemetry::history(&state.database, node.uuid, hours),
        )?;

        ApiResponse::new_serialized(Response {
            latest,
            bucket_minutes: shared::node_telemetry::bucket_minutes(hours),
            history,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
