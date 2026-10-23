//! plugged.host: an admin forcing a component's status, optionally for a time window.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        status::AdminApiStatusOverride,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        overrides: Vec<AdminApiStatusOverride>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("status.read")?;

        ApiResponse::new_serialized(Response {
            overrides: shared::status::overrides(&state.database).await?,
        })
        .ok()
    }
}

mod post {
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The component; none: all of them.
        #[garde(skip)]
        component_uuid: Option<uuid::Uuid>,
        #[garde(length(chars, min = 1, max = 32))]
        status: compact_str::CompactString,
        #[garde(length(chars, max = 1000))]
        note: Option<compact_str::CompactString>,
        #[garde(skip)]
        starts: Option<chrono::DateTime<chrono::Utc>>,
        #[garde(skip)]
        ends: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        uuid: uuid::Uuid,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = shared::ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if !shared::status::STATUSES.contains(&data.status.as_str()) {
            return super::super::bad_request("unknown status");
        }
        if let (Some(starts), Some(ends)) = (data.starts, data.ends)
            && ends <= starts
        {
            return super::super::bad_request("the override must end after it starts");
        }

        let uuid = shared::status::create_override(
            &state.database,
            data.component_uuid,
            &data.status,
            data.note.as_deref().filter(|note| !note.trim().is_empty()),
            data.starts,
            data.ends,
            Some(user.uuid),
        )
        .await?;

        activity_logger
            .log(
                "status:override.create",
                serde_json::json!({
                    "uuid": uuid,
                    "component_uuid": data.component_uuid,
                    "status": data.status,
                    "note": data.note,
                    "starts": data.starts,
                    "ends": data.ends,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { uuid }).ok()
    }
}

mod delete {
    use axum::extract::Path;
    use serde::Serialize;
    use shared::{
        GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/{override}", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("override" = uuid::Uuid, Path, description = "The status override ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(override_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if !shared::status::delete_override(&state.database, override_uuid).await? {
            return super::super::not_found("status override not found");
        }

        activity_logger
            .log(
                "status:override.delete",
                serde_json::json!({ "uuid": override_uuid }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
