use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            ByUuid, database_host::DatabaseHost, server::GetServer,
            server_database::ServerDatabase, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use std::collections::HashMap;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        sizes: HashMap<uuid::Uuid, i64>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
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

        let databases = ServerDatabase::uuids_by_server_uuid_database_host_uuid(
            &state.database,
            server.uuid,
            database_host,
        )
        .await?;
        if databases.is_empty() {
            return ApiResponse::error("database host not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        let Some(mut database_host) =
            DatabaseHost::by_uuid_optional_cached(&state.database, database_host).await?
        else {
            return ApiResponse::error("database host not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        if database_host.maintenance_enabled {
            return ApiResponse::error(
                "cannot get database sizes while database host is in maintenance mode",
            )
            .with_status(StatusCode::EXPECTATION_FAILED)
            .ok();
        }

        let host_sizes = match database_host.get_cached_database_sizes(&state).await {
            Ok(sizes) => sizes,
            Err(err) => {
                tracing::error!(
                    server = %server.uuid,
                    database_host = %database_host.uuid,
                    "failed to get database sizes: {:?}",
                    err
                );

                return ApiResponse::error("failed to get database sizes")
                    .with_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .ok();
            }
        };

        ApiResponse::new_serialized(Response {
            sizes: databases
                .into_iter()
                .filter_map(|database| host_sizes.get(&database).map(|size| (database, *size)))
                .collect(),
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
