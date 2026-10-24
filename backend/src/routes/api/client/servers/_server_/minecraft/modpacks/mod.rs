// plugged.host: installing a modpack onto a Minecraft server (shared/src/minecraft/modpacks.rs,
// docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod install;
mod search;
mod versions;

mod get {
    use shared::{
        ApiError, GetState,
        minecraft::modpacks::Modpacks,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Modpacks),
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

        match shared::minecraft::modpacks::options(&state, &server).await {
            Ok(modpacks) => ApiResponse::new_serialized(modpacks).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/search", search::router(state))
        .nest("/versions", versions::router(state))
        .nest("/install", install::router(state))
        .with_state(state.clone())
}
