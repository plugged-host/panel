// plugged.host: the server.cfg txAdmin runs, as a form (shared/src/fivem/config.rs,
// docs/features/fivem.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use shared::{
        ApiError, GetState,
        fivem::config::Config,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Config),
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
        permissions.has_server_permission("fivem.config")?;
        shared::fivem::require(&server)?;

        match shared::fivem::config::load(&state, &server).await {
            Ok(config) => ApiResponse::new_serialized(config).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

mod put {
    use axum::http::StatusCode;
    use shared::{
        ApiError, GetState,
        fivem::config::{Config, Save},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = Config),
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
    ), request_body = inline(Save))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Save>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("fivem.config")?;
        shared::fivem::require(&server)?;

        let (config, changed) = match shared::fivem::config::save(&state, &server, &data).await {
            Ok(saved) => saved,
            Err(err) => return err.response().ok(),
        };

        // the names of what changed, never the values: keys and passwords are among them
        if !changed.is_empty() {
            activity_logger
                .log(
                    "server:fivem.config.update",
                    serde_json::json!({
                        "file": config.path,
                        "changed": changed,
                    }),
                )
                .await;
        }

        ApiResponse::new_serialized(config).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, put::route))
        .with_state(state.clone())
}
