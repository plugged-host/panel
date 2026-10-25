// plugged.host: deleting a server's subdomain in Admin › Servers (docs/features/subdomains.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod delete {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, server::GetServer, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        subdomains::{Subdomain, records::Service},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Tokens that went back to the owner.
        refunded: i32,
    }

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "subdomain" = uuid::Uuid,
            description = "The subdomain ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetAdminActivityLogger,
        Path((_server, subdomain)): Path<(String, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("servers.subdomains")?;

        let Some(subdomain) =
            Subdomain::by_server_uuid_uuid(&state.database, server.uuid, subdomain).await?
        else {
            return ApiResponse::error("subdomain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        let refunded = shared::subdomains::delete(&state, &subdomain, Service::of_egg(&server.egg))
            .await?
            .unwrap_or_default();

        activity_logger
            .log(
                "server:subdomain.delete",
                serde_json::json!({
                    "uuid": server.uuid,
                    "subdomain": subdomain.fqdn(),
                    "refunded": refunded,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { refunded }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
