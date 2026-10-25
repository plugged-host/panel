//! plugged.host: an admin moves a server to the best node of a location, through the same moves as players' (shared/src/server_transfers.rs, docs/features/transfers.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_transfers::ApiTransferRequest,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The location whose best node takes the server; the server's own location finds another
        /// node there.
        #[garde(skip)]
        location_uuid: uuid::Uuid,
        /// Start the server on its new node even when it was off.
        #[garde(skip)]
        #[serde(default)]
        start: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        request: ApiTransferRequest,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
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
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("servers.transfer")?;

        let request = match shared::server_transfers::admin_request(
            &state,
            &server,
            &user,
            data.location_uuid,
            data.start,
        )
        .await
        {
            Ok(request) => request,
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status())
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    )
                    .ok();
            }
        };

        activity_logger
            .log(
                "server:transfer",
                serde_json::json!({
                    "uuid": server.uuid,
                    "location_uuid": data.location_uuid,
                    "request": request.uuid,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { request })
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
