// plugged.host: a server's subdomains in Admin › Servers (docs/features/subdomains.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _subdomain_;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
        subdomains::{ApiSubdomain, Subdomain, records::Service},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        subdomains: Vec<ApiSubdomain>,
        /// The server's own limit; `None` follows the settings' default.
        limit: Option<i32>,
        default_limit: i32,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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
        permissions.has_admin_permission("servers.subdomains")?;

        let default_limit = state.settings.get().await?.subdomains.default_limit;
        let service = Service::of_features(&server.egg.features);
        let port = server.allocation.as_ref().map(|a| a.allocation.port);

        ApiResponse::new_serialized(Response {
            subdomains: Subdomain::all_by_server_uuid(&state.database, server.uuid)
                .await?
                .into_iter()
                .map(|subdomain| subdomain.into_api(service, port))
                .collect(),
            limit: shared::subdomains::own_limit(&state.database, server.uuid).await?,
            default_limit,
        })
        .ok()
    }
}

mod put {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, server::GetServer, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The server's own limit; `null` follows the settings' default.
        #[garde(inner(range(min = 0, max = 100)))]
        limit: Option<i32>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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
        server: GetServer,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("servers.subdomains")?;

        shared::subdomains::set_limit(&state.database, server.uuid, data.limit).await?;

        activity_logger
            .log(
                "server:subdomains.limit",
                serde_json::json!({ "uuid": server.uuid, "limit": data.limit }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, put::route))
        .nest("/{subdomain}", _subdomain_::router(state))
        .with_state(state.clone())
}
