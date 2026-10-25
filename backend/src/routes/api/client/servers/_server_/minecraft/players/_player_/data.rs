// plugged.host: delete parts of a player's saved data while the server is stopped
// (shared/src/minecraft/player_data.rs, docs/features/minecraft.md#player-details).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod delete {
    use axum::extract::Path;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::player_data::Removal,
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
        #[serde(flatten)]
        removal: Removal,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
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

        // read before the files go
        let name = shared::minecraft::player_data::name_of(&state, &server, player).await;

        if let Err(err) = shared::minecraft::player_data::remove(
            &state,
            &server,
            player,
            data.removal,
            super::super::access(&permissions),
        )
        .await
        {
            return err.response().ok();
        }

        activity_logger
            .log(
                "server:minecraft.players.delete-data",
                serde_json::json!({
                    "uuid": player,
                    "name": name,
                    "removed": data.removal.names(),
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
