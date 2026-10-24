// plugged.host: the whitelist, operators and bans of a Minecraft server
// (shared/src/minecraft/players.rs, docs/features/minecraft.md).
use super::State;
use shared::{
    minecraft::players::Change,
    models::{server::Server, user::PermissionManager},
    response::{ApiResponse, ApiResponseResult},
};
use utoipa_axum::{router::OpenApiRouter, routes};

mod bans;
mod enforce;
mod operators;
mod whitelist;

mod get {
    use shared::{
        ApiError, GetState,
        minecraft::players::Roster,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Roster),
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
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::players::roster(&state, &server).await {
            Ok(roster) => ApiResponse::new_serialized(roster).ok(),
            Err(err) => shared::minecraft::players::PlayerError::Wings(err)
                .response()
                .ok(),
        }
    }
}

/// One player's change, by console while the server runs and the caller may use it.
async fn apply(
    state: &shared::State,
    permissions: &PermissionManager,
    server: &Server,
    activity_logger: &shared::models::server::ServerActivityLogger,
    name: &str,
    change: Change,
) -> ApiResponseResult {
    permissions.has_server_permission("minecraft.players")?;
    shared::minecraft::require(server)?;

    let console = permissions.has_server_permission("control.console").is_ok();
    let event = change.event();
    let reason = match &change {
        Change::Ban(reason) => reason.clone(),
        _ => None,
    };

    match shared::minecraft::players::change(state, server, name, change, console).await {
        Ok(outcome) => {
            activity_logger
                .log(
                    event,
                    serde_json::json!({
                        "name": name.trim(),
                        "reason": reason,
                        "live": outcome.live,
                    }),
                )
                .await;

            ApiResponse::new_serialized(outcome).ok()
        }
        Err(err) => err.response().ok(),
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/whitelist", whitelist::router(state))
        .nest("/operators", operators::router(state))
        .nest("/bans", bans::router(state))
        .nest("/enforce", enforce::router(state))
        .with_state(state.clone())
}
