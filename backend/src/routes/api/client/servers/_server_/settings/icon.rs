// plugged.host: uploading and deleting a server's icon, shaped like upstream's account avatar
// routes (client/account/avatar.rs) (docs/features/server-settings.md#server-icon).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod put {
    use axum::body::Bytes;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger, icon},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        icon: String,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = String)]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        image: Bytes,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.change-icon")?;

        let data = icon::transcode(image).await?;

        tokio::spawn(async move {
            let path = server.store_icon(&state, data).await?;

            activity_logger
                .log("server:settings.update-icon", serde_json::json!({}))
                .await;

            ApiResponse::new_serialized(Response {
                icon: state.storage.retrieve_urls().await?.get_url(&path),
            })
            .ok()
        })
        .await?
    }
}

mod delete {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
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
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.change-icon")?;

        if server.icon.is_none() {
            return ApiResponse::error("no icon to delete")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        tokio::spawn(async move {
            server.remove_icon(&state).await?;

            activity_logger
                .log("server:settings.delete-icon", serde_json::json!({}))
                .await;

            ApiResponse::new_serialized(Response {}).ok()
        })
        .await?
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(put::route))
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
