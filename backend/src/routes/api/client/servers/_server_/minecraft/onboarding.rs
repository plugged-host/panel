// plugged.host: the setup a player's new Minecraft server starts with
// (shared/src/minecraft/onboarding.rs, docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use shared::{
        ApiError, GetState,
        minecraft::onboarding::Onboarding,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Onboarding),
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
        permissions.has_server_permission("minecraft.settings")?;
        shared::minecraft::require(&server)?;

        // a server without a setup (one imported from the Pelican panel, or made before setups)
        // has nothing pending: every page waits on this answer, so it is never an error
        match shared::minecraft::onboarding::get(&state, &server).await? {
            Some(onboarding) => ApiResponse::new_serialized(onboarding).ok(),
            None => ApiResponse::new_serialized(Onboarding::none()).ok(),
        }
    }
}

mod post {
    use axum::http::StatusCode;
    use shared::{
        ApiError, GetState,
        minecraft::onboarding::Answers,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(serde_json::Value)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Answers))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Answers>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("minecraft.settings")?;
        permissions.has_server_permission("minecraft.players")?;
        if data.reinstalls() {
            permissions.has_server_permission("settings.software")?;
        }
        if data.modpack.is_some() {
            permissions.has_server_permission("minecraft.content")?;
        }
        if data.bedrock {
            permissions.has_server_permission("minecraft.bedrock")?;
        }
        shared::minecraft::require(&server)?;

        if let Err(err) = shared::minecraft::onboarding::complete(&state, &server, &data).await {
            return err.response().ok();
        }

        // The answers wait for the software picked before (or now), then start the server: the
        // page polls.
        let server_uuid = server.uuid;
        tokio::spawn(async move {
            let progress = shared::minecraft::onboarding::run_until_ended(
                &state,
                server_uuid,
                std::time::Duration::from_secs(3 * 60 * 60),
            )
            .await;

            activity_logger
                .log(
                    "server:minecraft.onboarding.complete",
                    serde_json::json!({
                        "variant": data.variant,
                        "version": data.version,
                        "modpack": data.modpack.as_ref().map(|modpack| &modpack.project),
                        "bedrock": data.bedrock,
                        "online_mode": data.online_mode,
                        "private": data.private,
                        "players": data.players.len(),
                        "operators": data.operators.len(),
                        "step": progress.as_ref().map(|progress| progress.step),
                        "error": progress.and_then(|progress| progress.error),
                    }),
                )
                .await;
        });

        ApiResponse::new_serialized(serde_json::json!({}))
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

mod software {
    use axum::http::StatusCode;
    use shared::{
        ApiError, GetState,
        minecraft::onboarding::SoftwarePick,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    /// Puts the picked software (or modpack) in place at once, while the rest is still being
    /// answered.
    #[utoipa::path(put, path = "/software", responses(
        (status = ACCEPTED, body = inline(serde_json::Value)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(SoftwarePick))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        shared::Payload(data): shared::Payload<SoftwarePick>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("minecraft.settings")?;
        permissions.has_server_permission("settings.software")?;
        if data.modpack.is_some() {
            permissions.has_server_permission("minecraft.content")?;
        }
        shared::minecraft::require(&server)?;

        if let Err(err) = shared::minecraft::onboarding::choose(&state, &server, &data).await {
            return err.response().ok();
        }

        ApiResponse::new_serialized(serde_json::json!({}))
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .routes(routes!(software::route))
        .with_state(state.clone())
}
