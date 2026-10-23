use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod put {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel,
            server::{GetServer, GetServerActivityLogger, TrashSettings, UpdateServerOptions},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(dive)]
        #[schema(inline)]
        custom: Option<TrashSettings>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("settings.trash")?;

        if !state
            .settings
            .get()
            .await?
            .server
            .allow_changing_trash_settings
        {
            return ApiResponse::error("changing trash settings is not allowed")
                .with_status(StatusCode::FORBIDDEN)
                .ok();
        }

        server
            .update(
                &state,
                UpdateServerOptions {
                    trash: Some(data.custom),
                    ..Default::default()
                },
            )
            .await?;

        activity_logger
            .log(
                "server:settings.trash",
                serde_json::json!({
                    "custom": data.custom,
                }),
            )
            .await;

        server.0.batch_sync(&state).await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(put::route))
        .with_state(state.clone())
}
