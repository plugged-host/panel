// plugged.host: one subdomain of the server (docs/features/subdomains.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod patch {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        subdomains::{ApiSubdomain, Subdomain, records::Service},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// A new name; the same (or none) writes the records again.
        #[garde(inner(length(chars, min = 1, max = 63)))]
        name: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        subdomain: ApiSubdomain,
    }

    #[utoipa::path(patch, path = "/", responses(
        (status = OK, body = inline(Response)),
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
        (
            "subdomain" = uuid::Uuid,
            description = "The subdomain ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        Path((_server, subdomain)): Path<(String, uuid::Uuid)>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("subdomains.update")?;

        let Some(subdomain) =
            Subdomain::by_server_uuid_uuid(&state.database, server.uuid, subdomain).await?
        else {
            return ApiResponse::error("subdomain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };
        let old = subdomain.fqdn();

        let subdomain =
            match shared::subdomains::update(&state, &server, subdomain, data.name).await {
                Ok(subdomain) => subdomain,
                Err(err) => return err.response().ok(),
            };

        activity_logger
            .log(
                "server:subdomain.update",
                serde_json::json!({ "subdomain": subdomain.fqdn(), "old": old }),
            )
            .await;

        let port = server.allocation.as_ref().map(|a| a.allocation.port);
        ApiResponse::new_serialized(Response {
            subdomain: subdomain.into_api(Service::of_egg(&server.egg), port),
        })
        .ok()
    }
}

mod delete {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
        subdomains::{Subdomain, records::Service},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Tokens that went back to the owner.
        refunded: i32,
    }

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "subdomain" = uuid::Uuid,
            description = "The subdomain ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        Path((_server, subdomain)): Path<(String, uuid::Uuid)>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("subdomains.delete")?;

        let Some(subdomain) =
            Subdomain::by_server_uuid_uuid(&state.database, server.uuid, subdomain).await?
        else {
            return ApiResponse::error("subdomain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        let Some(refunded) =
            shared::subdomains::delete(&state, &subdomain, Service::of_egg(&server.egg)).await?
        else {
            return ApiResponse::error("subdomain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        activity_logger
            .log(
                "server:subdomain.delete",
                serde_json::json!({ "subdomain": subdomain.fqdn(), "refunded": refunded }),
            )
            .await;

        ApiResponse::new_serialized(Response { refunded }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(patch::route, delete::route))
        .with_state(state.clone())
}
