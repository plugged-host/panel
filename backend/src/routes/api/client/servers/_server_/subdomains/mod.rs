// plugged.host: the server's subdomains (shared/src/subdomains, docs/features/subdomains.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _subdomain_;

mod get {
    use shared::{
        ApiError, GetState,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        subdomains::ApiOverview,
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = ApiOverview),
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
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("subdomains.read")?;

        ApiResponse::new_serialized(shared::subdomains::overview(&state, &server, user.uuid).await?)
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
        subdomains::{ApiSubdomain, Create, records::Service},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The name in front of the domain (`play`).
        #[garde(length(chars, min = 1, max = 63))]
        name: String,
        /// One of the domains from the list; the first when left out.
        #[garde(skip)]
        domain_uuid: Option<uuid::Uuid>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        subdomain: ApiSubdomain,
    }

    #[utoipa::path(post, path = "/", responses(
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

        permissions.has_server_permission("subdomains.create")?;

        let subdomain = match shared::subdomains::create(
            &state,
            &server,
            Create {
                name: data.name,
                domain_uuid: data.domain_uuid,
            },
        )
        .await
        {
            Ok(subdomain) => subdomain,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "server:subdomain.create",
                serde_json::json!({
                    "subdomain": subdomain.fqdn(),
                    "tokens": subdomain.token_cost,
                }),
            )
            .await;

        let port = server.allocation.as_ref().map(|a| a.allocation.port);
        ApiResponse::new_serialized(Response {
            subdomain: subdomain.into_api(Service::of_features(&server.egg.features), port),
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .nest("/{subdomain}", _subdomain_::router(state))
        .with_state(state.clone())
}
