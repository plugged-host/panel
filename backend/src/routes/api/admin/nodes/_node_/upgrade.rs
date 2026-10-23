use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = shared::updates::UpgradeStatus),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        let token = state
            .settings
            .get()
            .await?
            .updates
            .wings_release_token_for(&permissions);

        ApiResponse::new_serialized(node.upgrade_status(&state, token).await?).ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger, node::GetNode, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    fn default_version() -> compact_str::CompactString {
        "latest".into()
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 64))]
        #[serde(default = "default_version")]
        version: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        applied: bool,
        version: Option<compact_str::CompactString>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = NOT_IMPLEMENTED, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("nodes.upgrade")?;

        let client = node.api_client(&state.database).await?;

        if client.get_system_overview().await?.upgrade.is_none() {
            return ApiResponse::error("this node does not support upgrading itself")
                .with_status(StatusCode::NOT_IMPLEMENTED)
                .ok();
        }

        let token = state
            .settings
            .get()
            .await?
            .updates
            .wings_release_token_for(&permissions);
        let token_attached = token.is_some();

        let response = match client
            .post_system_upgrade(&wings_api::system_upgrade::post::RequestBody {
                version: data.version.clone(),
                token,
            })
            .await
        {
            Ok(wings_api::system_upgrade::post::Response::Ok(response)) => Response {
                applied: response.applied,
                version: response.version,
            },
            Ok(wings_api::system_upgrade::post::Response::Accepted(response)) => Response {
                applied: response.applied,
                version: response.version,
            },
            Err(wings_api::client::ApiHttpError::Http(
                status @ (StatusCode::BAD_REQUEST
                | StatusCode::NOT_FOUND
                | StatusCode::CONFLICT
                | StatusCode::NOT_IMPLEMENTED),
                err,
            )) => {
                return ApiResponse::new_serialized(ApiError::new_wings_value(err))
                    .with_status(status)
                    .ok();
            }
            Err(err) => return Err(err.into()),
        };

        state.cache.invalidate("nodes::versions").await?;
        state.cache.invalidate("nodes::versions::token").await?;

        activity_logger
            .log(
                "node:upgrade",
                serde_json::json!({
                    "node_uuid": node.uuid,
                    "version": data.version,
                    "applied": response.applied,
                    "resolved_version": response.version,
                    "token_attached": token_attached,
                }),
            )
            .await;

        ApiResponse::new_serialized(response).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
