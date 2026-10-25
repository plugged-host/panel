// plugged.host: what the server uses of its backup, database and allocation limits, for the limits
// on its Settings page (docs/features/server-settings.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::GetServer, server_allocation::ServerAllocation, server_backup::ServerBackup,
            server_database::ServerDatabase, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    /// How many the server has of each; `None` where the person may not see that list.
    #[derive(ToSchema, Serialize)]
    struct Usage {
        backups: Option<i64>,
        databases: Option<i64>,
        allocations: Option<i64>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        usage: Usage,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
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
    ) -> ApiResponseResult {
        let usage = Usage {
            backups: match permissions.has_server_permission("backups.read") {
                Ok(()) => {
                    Some(ServerBackup::count_by_server_uuid(&state.database, server.uuid).await?)
                }
                Err(_) => None,
            },
            databases: match permissions.has_server_permission("databases.read") {
                Ok(()) => {
                    Some(ServerDatabase::count_by_server_uuid(&state.database, server.uuid).await?)
                }
                Err(_) => None,
            },
            allocations: match permissions.has_server_permission("allocations.read") {
                Ok(()) => Some(
                    ServerAllocation::count_by_server_uuid(&state.database, server.uuid).await?,
                ),
                Err(_) => None,
            },
        };

        ApiResponse::new_serialized(Response { usage }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
