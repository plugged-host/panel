// plugged.host: server.properties and the Bukkit-family YAML as forms (shared/src/minecraft/config.rs,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        minecraft::config::ConfigFile,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        files: Vec<ConfigFile>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
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
        permissions.has_server_permission("minecraft.settings")?;
        shared::minecraft::require(&server)?;

        let software = shared::minecraft::software(&state, &server).await?;

        match shared::minecraft::config::load(&state, &server, software.loader).await {
            Ok(files) => ApiResponse::new_serialized(Response { files }).ok(),
            Err(err) => shared::minecraft::config::ConfigError::Wings(err)
                .response()
                .ok(),
        }
    }
}

mod put {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::config::{Edit, MAX_EDITS},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The file's id from the list (`properties`, `bukkit`, `spigot`, `paper`, `purpur`).
        #[garde(length(chars, min = 1, max = 32))]
        #[schema(min_length = 1, max_length = 32)]
        file: compact_str::CompactString,
        #[garde(length(max = MAX_EDITS))]
        #[schema(max_items = 1000)]
        edits: Vec<Edit>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// How many values changed.
        changed: usize,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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

        permissions.has_server_permission("minecraft.settings")?;
        shared::minecraft::require(&server)?;

        let software = shared::minecraft::software(&state, &server).await?;

        let (path, changed) = match shared::minecraft::config::save(
            &state,
            &server,
            software.loader,
            &data.file,
            &data.edits,
        )
        .await
        {
            Ok(saved) => saved,
            Err(err) => return err.response().ok(),
        };

        if changed > 0 {
            activity_logger
                .log(
                    "server:minecraft.settings.update",
                    serde_json::json!({
                        "file": path,
                        "changed": changed,
                    }),
                )
                .await;
        }

        ApiResponse::new_serialized(Response { changed }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
