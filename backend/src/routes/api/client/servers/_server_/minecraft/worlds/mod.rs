// plugged.host: a Minecraft server's worlds (shared/src/minecraft/worlds.rs,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod active;
mod import;
mod save;
mod upload;

mod get {
    use shared::{
        ApiError, GetState,
        minecraft::worlds::Worlds,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Worlds),
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
        permissions.has_server_permission("minecraft.worlds")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::worlds::load(&state, &server).await {
            Ok(worlds) => ApiResponse::new_serialized(worlds).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::worlds::{Generation, Replaced},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        name: String,
        #[garde(length(chars, max = 64))]
        #[schema(max_length = 64)]
        #[serde(default)]
        seed: String,
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        level_type: String,
        #[garde(skip)]
        structures: bool,
        #[garde(length(chars, max = 2000))]
        #[schema(max_length = 2000)]
        #[serde(default)]
        generator: String,
        #[garde(length(max = 32))]
        #[schema(max_items = 32)]
        #[serde(default)]
        datapacks: Vec<String>,
        /// Keep the loaded world under a new name instead of deleting it.
        #[garde(skip)]
        keep: bool,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = Replaced),
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
        shared::minecraft::require(&server)?;

        let generation = Generation {
            name: data.name.trim().to_string(),
            seed: data.seed,
            level_type: data.level_type,
            structures: data.structures,
            generator: data.generator,
            datapacks: data.datapacks,
        };

        match shared::minecraft::worlds::create(&state, &server, &generation, data.keep).await {
            Ok(replaced) => {
                activity_logger
                    .log(
                        "server:minecraft.worlds.create",
                        serde_json::json!({
                            "world": generation.name,
                            "level_type": generation.level_type,
                            "kept": replaced.kept,
                            "removed": replaced.removed,
                        }),
                    )
                    .await;

                ApiResponse::new_serialized(replaced).ok()
            }
            Err(err) => err.response().ok(),
        }
    }
}

mod delete {
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
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        world: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        folders: Vec<String>,
    }

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
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
        shared::minecraft::require(&server)?;

        match shared::minecraft::worlds::delete(&state, &server, &data.world).await {
            Ok(folders) => {
                activity_logger
                    .log(
                        "server:minecraft.worlds.delete",
                        serde_json::json!({
                            "world": data.world,
                            "folders": folders,
                        }),
                    )
                    .await;

                ApiResponse::new_serialized(Response { folders }).ok()
            }
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .routes(routes!(delete::route))
        .nest("/active", active::router(state))
        .nest("/save", save::router(state))
        .nest("/upload", upload::router(state))
        .nest("/import", import::router(state))
        .with_state(state.clone())
}
