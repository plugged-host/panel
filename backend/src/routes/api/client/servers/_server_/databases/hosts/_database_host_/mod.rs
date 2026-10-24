use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod sizes;

mod get {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject, database_host::DatabaseHost, server::GetServer,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        database_host: shared::models::database_host::ApiDatabaseHost,
    }

    #[utoipa::path(get, path = "/", responses(
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
            "database_host" = uuid::Uuid,
            description = "The database host ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Path((_server, database_host)): Path<(String, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("databases.read")?;

        let node = server.node.fetch_cached(&state.database).await?;

        let Some(database_host) =
            DatabaseHost::by_node_server_uuid(&state.database, &node, server.uuid, database_host)
                .await?
        else {
            return ApiResponse::error("database host not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        ApiResponse::new_serialized(Response {
            database_host: database_host.into_api_object(&state, ()).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/sizes", sizes::router(state))
        .with_state(state.clone())
}
