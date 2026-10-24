use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _database_host_;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject, database_host::DatabaseHost, server::GetServer,
            user::GetPermissionManager,
        },
        prelude::AsyncIteratorExt,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        database_hosts: Vec<shared::models::database_host::ApiDatabaseHost>,
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
        permissions.has_server_permission("databases.read")?;

        let node = server.node.fetch_cached(&state.database).await?;
        let database_hosts = DatabaseHost::all_public_by_node(&state.database, &node).await?;

        ApiResponse::new_serialized(Response {
            database_hosts: database_hosts
                .into_iter()
                .map(|database_host| database_host.into_api_object(&state, ()))
                .try_collect_async_vec()
                .await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/{database_host}", _database_host_::router(state))
        .with_state(state.clone())
}
