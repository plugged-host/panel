// plugged.host: heal, feed or kill an online player through the console
// (shared/src/minecraft/player_data.rs, docs/features/minecraft.md#player-details).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::extract::Path;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::{player_data::PlayerAction, players::Outcome},
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
        action: PlayerAction,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = Outcome),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
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
        permissions.has_server_permission("control.console")?;
        shared::minecraft::require(&server)?;

        if let Err(err) =
            shared::minecraft::player_data::act(&state, &server, player, data.action).await
        {
            return err.response().ok();
        }

        let name = shared::minecraft::player_data::name_of(&state, &server, player).await;
        activity_logger
            .log(
                data.action.event(),
                serde_json::json!({
                    "uuid": player,
                    "name": name,
                    "live": true,
                }),
            )
            .await;

        ApiResponse::new_serialized(Outcome { live: true }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
