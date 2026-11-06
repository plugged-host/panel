//! plugged.host: a server's own domains on its Subdomains page (shared/src/subdomains/custom.rs,
//! docs/features/subdomains.md#custom-domains).

use super::super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

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
        subdomains::custom::ApiCustomDomain,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The domain the person owns (`play.example.com`).
        #[garde(length(chars, min = 4, max = 253))]
        domain: String,
    }

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        custom_domain: ApiCustomDomain,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        ("server" = uuid::Uuid, Path, description = "The server ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("subdomains.custom")?;

        let custom_domain =
            match shared::subdomains::custom::add(&state, &server, &data.domain).await {
                Ok(custom_domain) => custom_domain.into_api(),
                Err(err) => return err.response().ok(),
            };
        activity_logger
            .log(
                "server:custom-domain.create",
                serde_json::json!({ "domain": custom_domain.domain }),
            )
            .await;

        ApiResponse::new_serialized(Response { custom_domain }).ok()
    }
}

mod custom_domain {
    use axum::extract::Path;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        subdomains::custom::ApiCustomDomain,
    };
    use utoipa::ToSchema;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(ToSchema, Serialize)]
    struct Response {
        custom_domain: ApiCustomDomain,
    }

    mod verify {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(post, path = "/verify", responses(
            (status = OK, body = inline(super::Response)),
            (status = NOT_FOUND, body = ApiError),
            (status = UNPROCESSABLE_ENTITY, body = ApiError),
            (status = BAD_GATEWAY, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, Path, description = "The server ID"),
            ("custom" = uuid::Uuid, Path, description = "The custom domain ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            Path((_server_uuid, custom_uuid)): Path<(uuid::Uuid, uuid::Uuid)>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("subdomains.custom")?;

            let Some(custom_domain) =
                shared::subdomains::custom::by_uuid(&state.database, server.uuid, custom_uuid)
                    .await?
            else {
                return ApiResponse::error("custom domain not found")
                    .with_status(axum::http::StatusCode::NOT_FOUND)
                    .ok();
            };
            let custom_domain =
                match shared::subdomains::custom::verify(&state, custom_domain).await {
                    Ok(custom_domain) => custom_domain.into_api(),
                    Err(err) => return err.response().ok(),
                };
            activity_logger
                .log(
                    "server:custom-domain.verify",
                    serde_json::json!({ "domain": custom_domain.domain }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { custom_domain }).ok()
        }
    }

    mod delete {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::GetPermissionManager,
            },
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(delete, path = "/", responses(
            (status = OK, body = Object),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("server" = uuid::Uuid, Path, description = "The server ID"),
            ("custom" = uuid::Uuid, Path, description = "The custom domain ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            Path((_server_uuid, custom_uuid)): Path<(uuid::Uuid, uuid::Uuid)>,
        ) -> ApiResponseResult {
            permissions.has_server_permission("subdomains.custom")?;

            if !shared::subdomains::custom::delete(&state, server.uuid, custom_uuid).await? {
                return ApiResponse::error("custom domain not found")
                    .with_status(axum::http::StatusCode::NOT_FOUND)
                    .ok();
            }
            activity_logger
                .log(
                    "server:custom-domain.delete",
                    serde_json::json!({ "uuid": custom_uuid }),
                )
                .await;

            ApiResponse::new_serialized(serde_json::json!({})).ok()
        }
    }

    pub fn router(state: &super::super::super::State) -> OpenApiRouter<super::super::super::State> {
        OpenApiRouter::new()
            .routes(routes!(verify::route))
            .routes(routes!(delete::route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .nest("/{custom}", custom_domain::router(state))
        .with_state(state.clone())
}
