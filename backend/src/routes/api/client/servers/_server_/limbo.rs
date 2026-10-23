// plugged.host: a parked server's page and its restore (shared/src/limbo.rs,
// docs/features/limbo.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Whoever may start or restart the server sees it and may restore it.
fn may_start(
    permissions: &shared::models::user::PermissionManager,
) -> Result<(), shared::response::ApiResponse> {
    permissions
        .has_server_permission("control.start")
        .or_else(|_| permissions.has_server_permission("control.restart"))
}

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        limbo: shared::limbo::ApiLimbo,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
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
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        super::may_start(&permissions)?;

        ApiResponse::new_serialized(Response {
            limbo: shared::limbo::view(&state, &server, &user, user.admin).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        limbo::LimboError,
        models::{
            ByUuid,
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Where the server goes when its node has no room; empty for its own node.
        #[garde(skip)]
        location_uuid: Option<uuid::Uuid>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        limbo: shared::limbo::ApiLimbo,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        super::may_start(&permissions)?;

        match shared::limbo::restore(&state, &server, &user, data.location_uuid, user.admin).await {
            Ok(()) => {}
            Err(LimboError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        }

        let server = shared::models::server::Server::by_uuid(&state.database, server.uuid).await?;

        ApiResponse::new_serialized(Response {
            limbo: shared::limbo::view(&state, &server, &user, user.admin).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
