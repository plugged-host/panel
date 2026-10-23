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
    struct Response {
        version: String,
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

        let version = async {
            database_host
                .get_connection(&state.database)
                .await?
                .version()
                .await
        };

        match tokio::time::timeout(std::time::Duration::from_secs(5), version).await {
            Ok(Ok(version)) => ApiResponse::new_serialized(Response { version }).ok(),
            Ok(Err(err)) => ApiResponse::error(err.to_string())
                .with_status(StatusCode::EXPECTATION_FAILED)
                .ok(),
            Err(_) => {
                ApiResponse::error("fetching database host version timed out after 5 seconds")
                    .with_status(StatusCode::EXPECTATION_FAILED)
                    .ok()
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
