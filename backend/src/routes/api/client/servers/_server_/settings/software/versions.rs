// plugged.host: the Minecraft versions a software choice offers (shared/src/server_software.rs).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
        server_software::SoftwareError,
    };
    use utoipa::ToSchema;

    #[derive(Deserialize)]
    pub struct Params {
        #[serde(default)]
        variant: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Newest first.
        versions: Vec<compact_str::CompactString>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        ("variant" = Option<String>, Query, description = "The software choice; the server's own by default"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.software")?;

        match shared::server_software::versions(&state, &server, params.variant.as_deref()).await {
            Ok(versions) => ApiResponse::new_serialized(Response { versions }).ok(),
            Err(SoftwareError::Other(err)) => {
                tracing::warn!("minecraft versions: {err:#}");
                ApiResponse::error("the version list did not load, try again in a moment")
                    .with_status(StatusCode::BAD_GATEWAY)
                    .ok()
            }
            Err(err) => ApiResponse::error(&err.message())
                .with_status(StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST))
                .ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
