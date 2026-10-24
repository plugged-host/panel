// plugged.host: one domain of Admin › Domains (docs/features/subdomains.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod sync {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
        subdomains::ApiDomain,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        domain: ApiDomain,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "domain" = uuid::Uuid,
            description = "The domain ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(domain): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("domains.update")?;

        if shared::subdomains::domain(&state.database, domain)
            .await?
            .is_none()
        {
            return ApiResponse::error("domain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        let domain = match shared::subdomains::sync_domain(&state, domain).await {
            Ok(domain) => domain,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "domain:sync",
                serde_json::json!({ "uuid": domain.uuid, "name": domain.name }),
            )
            .await;

        ApiResponse::new_serialized(Response { domain }).ok()
    }
}

mod delete {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "domain" = uuid::Uuid,
            description = "The domain ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(domain): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("domains.delete")?;

        let Some(domain) = shared::subdomains::domain(&state.database, domain).await? else {
            return ApiResponse::error("domain not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        shared::subdomains::delete_domain(&state, domain.uuid).await?;

        activity_logger
            .log(
                "domain:delete",
                serde_json::json!({
                    "uuid": domain.uuid,
                    "name": domain.name,
                    "subdomains": domain.subdomains,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .nest(
            "/sync",
            OpenApiRouter::new()
                .routes(routes!(sync::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
