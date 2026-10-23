use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod delete {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            DeletableModel,
            server::{GetServer, GetServerActivityLogger},
            server_device::ServerDevice,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "device" = uuid::Uuid,
            description = "The device ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        Path((_server, device)): Path<(String, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("devices.detach")?;

        let server_device =
            match ServerDevice::by_server_uuid_device_uuid(&state.database, server.uuid, device)
                .await?
            {
                Some(device) => device,
                None => {
                    return ApiResponse::error("device not found")
                        .with_status(StatusCode::NOT_FOUND)
                        .ok();
                }
            };

        if !server_device
            .device
            .fetch(&state.database)
            .await?
            .user_attachable
        {
            return ApiResponse::new_serialized(ApiError::new_value(&["device not found"]))
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        server_device.delete(&state, ()).await?;

        activity_logger
            .log(
                "server:devices.detach",
                serde_json::json!({
                    "device_uuid": server_device.device.uuid,
                }),
            )
            .await;

        server.0.batch_sync(&state).await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
