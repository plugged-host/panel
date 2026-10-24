use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::routes::api::admin::database_hosts::_database_host_::GetDatabaseHost;
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ResponseServer {
        uuid: uuid::Uuid,
        name: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseDatabase {
        uuid: uuid::Uuid,
        name: compact_str::CompactString,
        #[schema(inline)]
        server: ResponseServer,
        size: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        databases: Vec<ResponseDatabase>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
    ), params(
        (
            "database_host" = uuid::Uuid,
            description = "The database host ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut database_host: GetDatabaseHost,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("database-hosts.read")?;

        if database_host.maintenance_enabled {
            return ApiResponse::error(
                "cannot get database sizes while database host is in maintenance mode",
            )
            .with_status(StatusCode::EXPECTATION_FAILED)
            .ok();
        }

        let sizes = match database_host.get_cached_database_sizes(&state).await {
            Ok(sizes) => sizes,
            Err(err) => {
                tracing::error!(
                    database_host = %database_host.uuid,
                    "failed to get database sizes: {:?}",
                    err
                );

                return ApiResponse::error("failed to get database sizes")
                    .with_status(StatusCode::EXPECTATION_FAILED)
                    .ok();
            }
        };

        let rows = sqlx::query!(
            r#"
            SELECT server_databases.uuid, server_databases.name, servers.uuid AS server_uuid, servers.name AS server_name
            FROM server_databases
            JOIN servers ON servers.uuid = server_databases.server_uuid
            WHERE server_databases.database_host_uuid = $1
            "#,
            database_host.uuid
        )
        .fetch_all(state.database.read())
        .await?;

        let mut databases = rows
            .into_iter()
            .map(|row| ResponseDatabase {
                size: sizes.get(&row.uuid).copied().unwrap_or(0),
                uuid: row.uuid,
                name: row.name.into(),
                server: ResponseServer {
                    uuid: row.server_uuid,
                    name: row.server_name.into(),
                },
            })
            .collect::<Vec<_>>();
        databases.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));

        ApiResponse::new_serialized(Response { databases }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
