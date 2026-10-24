// plugged.host: a modpack's versions, newest first (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::{Source, Version},
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        source: Source,
        project: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        versions: Vec<Version>,
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
        ("source" = Source, Query, description = "The modpack's source"),
        ("project" = String, Query, description = "The modpack's id on its source"),
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
            return ApiResponse::error("unknown modpack")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        match shared::minecraft::modpacks::versions(&state, &server, params.source, &params.project)
            .await
        {
            Ok(versions) => ApiResponse::new_serialized(Response { versions }).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
