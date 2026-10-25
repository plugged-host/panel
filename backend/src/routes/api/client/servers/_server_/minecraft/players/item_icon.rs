// plugged.host: the icons of the items in a player's inventory, through the panel and cached
// (shared/src/minecraft/player_images.rs, docs/features/minecraft.md#player-details).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{body::Body, extract::Query, http::StatusCode};
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::player_images,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        id: String,
        version: Option<String>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, description = "The icon", content_type = "image/png"),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        ("id" = String, Query, description = "The item id, like minecraft:diamond_sword"),
        ("version" = Option<String>, Query, description = "The Minecraft version, latest when left out"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        let version = params
            .version
            .as_deref()
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .unwrap_or("latest")
            .to_ascii_lowercase();
        if params.id.len() > 128 || !player_images::valid_version(&version) {
            return ApiResponse::error("invalid item or version")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        match player_images::item_icon(&state, params.id.trim(), &version).await {
            Ok(Some(icon)) => ApiResponse::new(Body::from(icon))
                .with_header("Content-Type", "image/png")
                .with_header("Cache-Control", "public, max-age=604800, immutable")
                .with_header("X-Content-Type-Options", "nosniff")
                .ok(),
            // a mod's item, or one no source has a picture of: the page shows its name instead
            Ok(None) => ApiResponse::error("there is no icon for this item")
                .with_status(StatusCode::NOT_FOUND)
                .ok(),
            Err(err) => {
                tracing::warn!("fetching an item icon: {err:#}");
                ApiResponse::error("the icon could not be fetched")
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
