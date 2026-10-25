// plugged.host: a player's game mode: by console while they play, in their file while the server
// is stopped (shared/src/minecraft/player_data.rs, docs/features/minecraft.md#player-details).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod put {
    use axum::extract::Path;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::{player_data::GameMode, players::Outcome},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        mode: GameMode,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = Outcome),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = UNPROCESSABLE_ENTITY, body = ApiError),
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
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        Path((_server, player)): Path<(String, uuid::Uuid)>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        let outcome = match shared::minecraft::player_data::set_game_mode(
            &state,
            &server,
            player,
            data.mode,
            super::super::access(&permissions),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => return err.response().ok(),
        };

        let name = shared::minecraft::player_data::name_of(&state, &server, player).await;
        activity_logger
            .log(
                "server:minecraft.players.gamemode",
                serde_json::json!({
                    "uuid": player,
                    "name": name,
                    "mode": data.mode.as_str(),
                    "live": outcome.live,
                }),
            )
            .await;

        ApiResponse::new_serialized(outcome).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(put::route))
        .with_state(state.clone())
}
