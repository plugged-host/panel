// plugged.host: one player of a Minecraft server in detail (shared/src/minecraft/player_data.rs,
// docs/features/minecraft.md#player-details).
use super::State;
use shared::{minecraft::player_data::Access, models::user::PermissionManager};
use utoipa_axum::{router::OpenApiRouter, routes};

mod action;
mod data;
mod game_mode;
mod head;
mod teleport;

/// What the caller may change: live through the console, offline in the files.
fn access(permissions: &PermissionManager) -> Access {
    Access {
        console: permissions.has_server_permission("control.console").is_ok(),
        edit: permissions.has_server_permission("files.update").is_ok(),
        delete: permissions.has_server_permission("files.delete").is_ok(),
    }
}

mod get {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        minecraft::player_data::PlayerDetails,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = PlayerDetails),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "player" = uuid::Uuid,
            description = "The player's UUID",
            example = "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Path((_server, player)): Path<(String, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::player_data::details(&state, &server, player).await {
            Ok(details) => ApiResponse::new_serialized(details).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/head", head::router(state))
        .nest("/action", action::router(state))
        .nest("/game-mode", game_mode::router(state))
        .nest("/teleport", teleport::router(state))
        .nest("/data", data::router(state))
        .with_state(state.clone())
}
