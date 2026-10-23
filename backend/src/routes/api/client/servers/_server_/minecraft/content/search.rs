// plugged.host: one page of plugins, mods or modpacks from a source (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::{Kind, Source},
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        #[serde(default = "modrinth")]
        source: Source,
        kind: Option<Kind>,
        #[serde(default)]
        query: compact_str::CompactString,
        /// Left out: the server's version; empty: any version.
        game_version: Option<compact_str::CompactString>,
        #[serde(default = "first")]
        page: u64,
    }

    fn modrinth() -> Source {
        Source::Modrinth
    }

    fn first() -> u64 {
        1
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        projects: shared::minecraft::SearchPage,
        kind: Kind,
        /// The version the projects are filtered by.
        game_version: Option<compact_str::CompactString>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
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
        ("kind" = Option<Kind>, Query, description = "The kind of project, the server's first by default"),
        ("query" = Option<String>, Query, description = "The search text"),
        ("game_version" = Option<String>, Query, description = "The Minecraft version; empty for any"),
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

        let software = shared::minecraft::software(&state, &server).await?;
        let Some(loader) = software.loader else {
            return shared::minecraft::content::ContentError::NoLoader
                .response()
                .ok();
        };
        let server_version = software.game_version(&state).await;

        match shared::minecraft::content::search(
            &state,
            loader,
            server_version.as_deref(),
            params.source,
            params.kind,
            &params.query,
            params.game_version.as_deref(),
            params.page.clamp(1, 500),
        )
        .await
        {
            Ok((projects, kind, game_version)) => ApiResponse::new_serialized(Response {
                projects,
                kind,
                game_version,
            })
            .ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
