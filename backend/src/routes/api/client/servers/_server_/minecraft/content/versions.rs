// plugged.host: a project's builds that fit the server (docs/features/minecraft.md).
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
        source: Source,
        project: compact_str::CompactString,
        kind: Option<Kind>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        versions: Vec<shared::minecraft::Version>,
        /// No build is made for the server's Minecraft version; these are all of them.
        loose: bool,
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
        ("source" = Source, Query, description = "The project's source"),
        ("project" = String, Query, description = "The project's id at its source"),
        ("kind" = Option<Kind>, Query, description = "The kind of project"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.content")?;
        shared::minecraft::require(&server)?;

        if params.project.is_empty() || params.project.len() > 64 {
            return ApiResponse::error("project not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        let software = shared::minecraft::software(&state, &server).await?;
        let Some(loader) = software.loader else {
            return shared::minecraft::content::ContentError::NoLoader
                .response()
                .ok();
        };
        let kind = params.kind.unwrap_or(loader.kinds()[0]);
        let game_version = software.game_version(&state).await;

        match shared::minecraft::content::versions(
            &state,
            loader,
            game_version.as_deref(),
            params.source,
            &params.project,
            kind,
        )
        .await
        {
            Ok((versions, loose)) => ApiResponse::new_serialized(Response { versions, loose }).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
