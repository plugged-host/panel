// plugged.host: plugins and mods (shared/src/minecraft/content.rs, docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod install;
mod installed;
mod search;
mod versions;

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
        content: shared::minecraft::content::ContentOptions,
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

        ApiResponse::new_serialized(Response {
            content: shared::minecraft::content::options(&state, &server).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/search", search::router(state))
        .nest("/versions", versions::router(state))
        .nest("/install", install::router(state))
        .nest("/installed", installed::router(state))
        .with_state(state.clone())
}
