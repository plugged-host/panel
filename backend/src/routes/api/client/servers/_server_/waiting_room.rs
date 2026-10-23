// plugged.host: a server's start waiting room (shared/src/waiting_room.rs, docs/features/ads.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Whoever may start or restart the server has a room for it.
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
        waiting_room::ApiWaitingRoom,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        room: ApiWaitingRoom,
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
            room: shared::waiting_room::view(&state, &server, &user).await?,
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
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        waiting_room::LeaveError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Whether the page found an ad blocker, for the activity entry.
        #[garde(skip)]
        blocked: Option<bool>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

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

        match shared::waiting_room::leave(&state, &server, &user, data.blocked).await {
            Ok(()) => ApiResponse::new_serialized(Response {}).ok(),
            Err(LeaveError::NotWaiting) => {
                ApiResponse::error("the server is not in a waiting room")
                    .with_status(StatusCode::CONFLICT)
                    .ok()
            }
            Err(LeaveError::NotReady) => ApiResponse::error("the waiting room is not over yet")
                .with_status(StatusCode::CONFLICT)
                .ok(),
            Err(LeaveError::Other(err)) => Err(err.into()),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
