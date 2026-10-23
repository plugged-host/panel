// plugged.host: a player moving their server to another location (shared/src/server_transfers.rs,
// docs/features/transfers.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod cancel;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        transfer: shared::server_transfers::ApiTransferOptions,
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
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.transfer")?;

        ApiResponse::new_serialized(Response {
            transfer: shared::server_transfers::options(&state, &server).await?,
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
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_transfers::TransferError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        location_uuid: uuid::Uuid,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        request: shared::server_transfers::ApiTransferRequest,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
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
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.transfer")?;

        let request = match shared::server_transfers::request(
            &state,
            &server,
            &user,
            data.location_uuid,
            permissions.is_admin(),
        )
        .await
        {
            Ok(request) => request,
            Err(TransferError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        };

        activity_logger
            .log(
                "server:transfer.request",
                serde_json::json!({
                    "location_uuid": data.location_uuid,
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
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .nest("/cancel", cancel::router(state))
        .with_state(state.clone())
}
