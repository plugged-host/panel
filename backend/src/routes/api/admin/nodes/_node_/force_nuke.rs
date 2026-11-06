//! plugged.host: Admin > Nodes > Force nuke. Only while the node does not answer: it force-deletes
//! every server on it, then deletes the node from the panel. The probe is the authority, so an
//! admin cannot nuke a healthy node by mistake (docs/features/nodes.md).

use std::time::Duration;

use axum::http::StatusCode;
use serde::Serialize;
use shared::{
    ApiError, GetState,
    models::{
        DeletableModel, admin_activity::GetAdminActivityLogger, node::GetNode, server::Server,
        user::GetPermissionManager,
    },
    response::{ApiResponse, ApiResponseResult},
};
use utoipa::ToSchema;

#[derive(ToSchema, Serialize)]
struct Response {
    servers_deleted: i64,
}

#[utoipa::path(post, path = "/force-nuke", responses(
    (status = OK, body = inline(Response)),
    (status = NOT_FOUND, body = ApiError),
    (status = BAD_REQUEST, body = ApiError, description = "The node answered, so it is not nuked"),
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
    permissions.has_admin_permission("nodes.force-nuke")?;

    // The node must not answer. A timeout, a refused connection or any API error counts as
    // offline; a successful answer refuses the nuke.
    if let Ok(client) = node.api_client(&state.database).await
        && matches!(
            tokio::time::timeout(Duration::from_secs(3), client.get_system_overview()).await,
            Ok(Ok(_))
        )
    {
        return ApiResponse::error(
            "node is responding, refusing to force nuke (use the normal delete)",
        )
        .with_status(StatusCode::BAD_REQUEST)
        .ok();
    }

    // Force-delete every server, one batch at a time: page 1 always holds the next servers.
    let mut servers_deleted = 0i64;
    loop {
        let batch =
            Server::by_node_uuid_with_pagination(&state.database, node.uuid, 1, 100, None).await?;
        if batch.data.is_empty() {
            break;
        }

        for server in batch.data {
            server
                .delete(
                    &state,
                    shared::models::server::DeleteServerOptions { force: true },
                )
                .await?;
            servers_deleted += 1;
        }
    }

    node.delete(&state, ()).await?;

    activity_logger
        .log(
            "node:force-nuke",
            serde_json::json!({
                "uuid": node.uuid,
                "name": node.name,
                "servers_deleted": servers_deleted,
            }),
        )
        .await;

    shared::tunnel::poke_nodes(&state.database).await;

    ApiResponse::new_serialized(Response { servers_deleted }).ok()
}
