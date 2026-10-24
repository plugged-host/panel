// plugged.host: Bedrock players on a Java server through Geyser and Floodgate
// (shared/src/minecraft/bedrock.rs, docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use shared::{
        ApiError, GetState,
        minecraft::bedrock::Bedrock,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Bedrock),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.bedrock")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::bedrock::status(&state, &server).await {
            Ok(bedrock) => ApiResponse::new_serialized(bedrock).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

mod post {
    use shared::{
        ApiError, GetState,
        minecraft::bedrock::Installed,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = Installed),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.bedrock")?;
        shared::minecraft::require(&server)?;

        match shared::minecraft::bedrock::install(&state, &server).await {
            Ok(installed) => {
                activity_logger
                    .log(
                        "server:minecraft.bedrock.install",
                        serde_json::json!({
                            "updated": installed.updated,
                            "floodgate": installed.floodgate,
                            "port": installed.port,
                        }),
                    )
                    .await;

                ApiResponse::new_serialized(installed).ok()
            }
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .with_state(state.clone())
}
