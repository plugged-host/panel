//! plugged.host: an admin deletes a server's custom domains on its Subdomains tab
//! (docs/features/subdomains.md#custom-domains).

use super::super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod delete {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, server::GetServer, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(delete, path = "/{custom}", responses(
        (status = OK, body = Object),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        ("server" = uuid::Uuid, Path, description = "The server ID"),
        ("custom" = uuid::Uuid, Path, description = "The custom domain ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetAdminActivityLogger,
        Path((_server_uuid, custom_uuid)): Path<(uuid::Uuid, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("servers.subdomains")?;

        if !shared::subdomains::custom::delete(&state, server.uuid, custom_uuid).await? {
            return ApiResponse::error("custom domain not found")
                .with_status(axum::http::StatusCode::NOT_FOUND)
                .ok();
        }
        activity_logger
            .log(
                "server:custom-domain.admin-delete",
                serde_json::json!({ "uuid": custom_uuid, "server_uuid": server.uuid }),
            )
            .await;

        ApiResponse::new_serialized(serde_json::json!({})).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
