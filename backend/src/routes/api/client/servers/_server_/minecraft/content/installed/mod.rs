// plugged.host: the installed plugins or mods (shared/src/minecraft/jars.rs,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod identify;
mod toggle;
mod update;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        installed: Vec<shared::minecraft::jars::InstalledJar>,
        /// Jars not identified yet: asking again identifies the next few.
        pending: usize,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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
        permissions.has_server_permission("minecraft.content")?;
        shared::minecraft::require(&server)?;

        let software = shared::minecraft::software(&state, &server).await?;
        let Some(loader) = software.loader else {
            return ApiResponse::new_serialized(Response {
                installed: Vec::new(),
                pending: 0,
            })
            .ok();
        };
        let game_version = software.game_version(&state).await;

        let (installed, pending) =
            shared::minecraft::jars::list(&state, &server, loader, game_version.as_deref()).await?;

        ApiResponse::new_serialized(Response { installed, pending }).ok()
    }
}

mod delete {
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
        /// Delete the plugin's own folder too.
        #[garde(skip)]
        #[serde(default)]
        with_data: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        deleted: Vec<compact_str::CompactString>,
    }

    #[utoipa::path(delete, path = "/", responses(
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

        let deleted = match shared::minecraft::jars::delete(
            &state,
            &server,
            loader,
            &data.file,
            data.with_data,
        )
        .await
        {
            Ok(deleted) => deleted,
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
                "server:minecraft.content.delete",
                serde_json::json!({
                    "files": deleted,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { deleted }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(delete::route))
        .nest("/identify", identify::router(state))
        .nest("/toggle", toggle::router(state))
        .nest("/update", update::router(state))
        .with_state(state.clone())
}
