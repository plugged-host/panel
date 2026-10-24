// plugged.host: who is on a FiveM server now (shared/src/fivem/players.rs, docs/features/fivem.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use shared::{
        ApiError, GetState,
        fivem::players::Players,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Players),
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
        permissions.has_server_permission("fivem.players")?;
        shared::fivem::require(&server)?;

        ApiResponse::new_serialized(shared::fivem::players::fetch(&state, &server).await).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
