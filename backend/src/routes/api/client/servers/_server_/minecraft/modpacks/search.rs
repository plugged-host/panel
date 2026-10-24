// plugged.host: one page of modpacks the server can run (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::{SearchPage, Source},
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        #[serde(default = "modrinth")]
        source: Source,
        #[serde(default)]
        query: compact_str::CompactString,
        #[serde(default = "first")]
        page: u64,
    }

    fn modrinth() -> Source {
        Source::Modrinth
    }

    fn first() -> u64 {
        1
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = SearchPage),
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
        ("source" = Source, Query, description = "Where to search"),
        ("query" = Option<String>, Query, description = "The search text"),
        ("page" = Option<u64>, Query, description = "The page, from 1"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.content")?;
        shared::minecraft::require(&server)?;

        if params.query.chars().count() > 100 {
            return ApiResponse::error("the search text is too long")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        match shared::minecraft::modpacks::search(
            &state,
            &server,
            params.source,
            &params.query,
            params.page.clamp(1, 500),
        )
        .await
        {
            Ok(page) => ApiResponse::new_serialized(page).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
