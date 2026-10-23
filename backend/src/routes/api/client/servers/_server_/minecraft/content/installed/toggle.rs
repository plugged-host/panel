// plugged.host: turning an installed plugin or mod on or off (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::jars::JarError,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 255))]
        #[schema(min_length = 1, max_length = 255)]
        file: compact_str::CompactString,
        #[garde(skip)]
        enabled: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// The file's new name.
        file: compact_str::CompactString,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("minecraft.content")?;
        shared::minecraft::require(&server)?;

        let software = shared::minecraft::software(&state, &server).await?;
        let Some(loader) = software.loader else {
            return ApiResponse::error("that file is not installed")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        let file = match shared::minecraft::jars::toggle(
            &state,
            &server,
            loader,
            &data.file,
            data.enabled,
        )
        .await
        {
            Ok(file) => file,
            Err(JarError::Other(err)) => return Err(err.into()),
            Err(err @ JarError::NotInstalled) => {
                return ApiResponse::error(&err.message())
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            }
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(StatusCode::BAD_GATEWAY)
                    .ok();
            }
        };

        activity_logger
            .log(
                "server:minecraft.content.toggle",
                serde_json::json!({
                    "file": file,
                    "enabled": data.enabled,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { file }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
