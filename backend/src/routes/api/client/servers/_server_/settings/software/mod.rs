// plugged.host: switching a server's software and Minecraft version
// (shared/src/server_software.rs).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod versions;

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
        /// `null` when the server's game offers no choice.
        software: Option<shared::server_software::ApiSoftware>,
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
        permissions.has_server_permission("settings.software")?;

        ApiResponse::new_serialized(Response {
            software: shared::server_software::options(&state, &server).await?,
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
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        server_software::SoftwareError,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The software choice; `null` keeps the current one.
        #[garde(inner(length(chars, min = 1, max = 64)))]
        #[schema(min_length = 1, max_length = 64)]
        #[serde(default)]
        variant: Option<compact_str::CompactString>,
        /// The Minecraft version (`latest` or a version); `null` keeps the current one.
        #[garde(inner(length(chars, min = 1, max = 32)))]
        #[schema(min_length = 1, max_length = 32)]
        #[serde(default)]
        version: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

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
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("settings.software")?;

        if data.variant.is_none() && data.version.is_none() {
            return ApiResponse::error("choose a software or a version")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        // not in the middle of a setup's or a modpack's switch and files
        let Some(_lock) =
            shared::server_software::lock(&state, server.uuid, std::time::Duration::ZERO).await
        else {
            return ApiResponse::error(
                "the server's software is being changed, try again when it is done",
            )
            .with_status(StatusCode::CONFLICT)
            .ok();
        };

        match shared::server_software::switch(
            &state,
            &mut server,
            data.variant.as_deref(),
            data.version.as_deref(),
        )
        .await
        {
            Ok(()) => {}
            Err(SoftwareError::Other(err)) => return Err(err.into()),
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
                "server:settings.software",
                serde_json::json!({
                    "variant": data.variant,
                    "version": data.version,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {})
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .nest("/versions", versions::router(state))
        .with_state(state.clone())
}
