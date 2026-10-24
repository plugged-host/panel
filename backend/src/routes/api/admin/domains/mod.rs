// plugged.host: Admin › Domains, the Cloudflare zones server subdomains are made under
// (shared/src/subdomains, docs/features/subdomains.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _domain_;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        subdomains::ApiDomain,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        domains: Vec<ApiDomain>,
        /// The Cloudflare token is set (Admin › Settings › Subdomains).
        token_set: bool,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("domains.read")?;

        let token_set = state
            .settings
            .get()
            .await?
            .subdomains
            .cloudflare_token
            .is_some();

        ApiResponse::new_serialized(Response {
            domains: shared::subdomains::domains(&state.database).await?,
            token_set,
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
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
        subdomains::ApiDomain,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The domain, as its Cloudflare zone is named (`example.com`).
        #[garde(length(chars, min = 3, max = 253))]
        name: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        domain: ApiDomain,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("domains.create")?;

        let domain = match shared::subdomains::add_domain(&state, &data.name).await {
            Ok(domain) => domain,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "domain:create",
                serde_json::json!({ "uuid": domain.uuid, "name": domain.name }),
            )
            .await;

        ApiResponse::new_serialized(Response { domain }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .nest("/{domain}", _domain_::router(state))
        .with_state(state.clone())
}
