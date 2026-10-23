// plugged.host: owners resizing their servers within their allowance (shared/src/server_resize.rs).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

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
        resize: shared::server_resize::ApiResize,
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
        permissions.has_server_permission("settings.resize")?;

        ApiResponse::new_serialized(Response {
            resize: shared::server_resize::options(&state, &server).await?,
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
        allowance::Resources,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        server_resize::ResizeError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(range(min = 0))]
        #[schema(minimum = 0)]
        cpu: i64,
        #[garde(range(min = 0))]
        #[schema(minimum = 0)]
        memory: i64,
        #[garde(range(min = 0))]
        #[schema(minimum = 0)]
        disk: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
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
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("settings.resize")?;

        let from = shared::server_resize::current(&server);
        let to = Resources::new(data.cpu, data.memory, data.disk);

        match shared::server_resize::resize(&state, &mut server, to, permissions.is_admin()).await {
            Ok(()) => {}
            Err(ResizeError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        }

        activity_logger
            .log(
                "server:settings.resize",
                serde_json::json!({
                    "from": from,
                    "to": to,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
