// plugged.host: a player's head, cut from their skin by the panel
// (shared/src/minecraft/player_images.rs, docs/features/minecraft.md#player-details).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{
        body::Body,
        extract::{Path, Query},
        http::StatusCode,
    };
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        /// The name, for an offline-mode UUID whose account is found by name.
        name: Option<String>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, description = "The head, 64×64", content_type = "image/png"),
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
        ("name" = Option<String>, Query, description = "The player's name"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Path((_server, player)): Path<(String, uuid::Uuid)>,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::player_images::head(&state, player, params.name.as_deref()).await {
            Ok(head) => ApiResponse::new(Body::from(head))
                .with_header("Content-Type", "image/png")
                .with_header("Cache-Control", "private, max-age=86400")
                .with_header("X-Content-Type-Options", "nosniff")
                .ok(),
            Err(err) => {
                tracing::warn!("making a player's head: {err:#}");
                ApiResponse::error("the head could not be made")
                    .with_status(StatusCode::BAD_GATEWAY)
                    .ok()
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
