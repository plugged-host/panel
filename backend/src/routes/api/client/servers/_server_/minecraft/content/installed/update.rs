// plugged.host: updating an installed plugin or mod to its newest Modrinth build
// (docs/features/minecraft.md).
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
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// The new file's name.
        file: compact_str::CompactString,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
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
        let game_version = software.game_version(&state).await;

        let file = match shared::minecraft::jars::update(
            &state,
            &server,
            loader,
            game_version.as_deref(),
            &data.file,
        )
        .await
        {
            Ok(file) => file,
            Err(JarError::Other(err)) => return Err(err.into()),
            Err(err @ (JarError::NotInstalled | JarError::NoUpdate)) => {
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
                "server:minecraft.content.update",
                serde_json::json!({
                    "from": data.file,
                    "to": file,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { file })
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
