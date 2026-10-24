// plugged.host: making an uploaded world archive the loaded world (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The archive's file name in the upload folder.
        #[garde(length(chars, min = 1, max = 255))]
        #[schema(min_length = 1, max_length = 255)]
        archive: String,
        /// Keep the loaded world under a new name instead of deleting it.
        #[garde(skip)]
        keep: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
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

        permissions.has_server_permission("minecraft.worlds")?;
        permissions.has_server_permission("files.create")?;
        shared::minecraft::require(&server)?;

        if let Err(err) =
            shared::minecraft::worlds::start_import(&state, &server, &data.archive).await
        {
            return err.response().ok();
        }

        // Unpacking a big world outlasts a request: the page polls the import's state.
        let server = server.0;
        tokio::spawn(async move {
            match shared::minecraft::worlds::import(&state, &server, &data.archive, data.keep).await
            {
                Ok(replaced) => {
                    activity_logger
                        .log(
                            "server:minecraft.worlds.import",
                            serde_json::json!({
                                "archive": data.archive,
                                "kept": replaced.kept,
                                "removed": replaced.removed,
                            }),
                        )
                        .await;
                }
                Err(err) => {
                    tracing::info!(server = %server.uuid, "world import failed: {}", err.message());
                }
            }
        });

        ApiResponse::new_serialized(Response {})
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
