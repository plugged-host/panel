//! plugged.host: incidents and their updates on the status page.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

fn valid_impact(impact: &str) -> bool {
    shared::status::INCIDENT_IMPACTS.contains(&impact)
}

fn valid_status(status: &str) -> bool {
    shared::status::INCIDENT_STATUSES.contains(&status)
}

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        status::AdminApiStatusIncident,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        incidents: Vec<AdminApiStatusIncident>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("status.read")?;

        ApiResponse::new_serialized(Response {
            incidents: shared::status::incidents(&state.database, 100).await?,
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
        #[garde(length(chars, min = 1, max = 255))]
        title: compact_str::CompactString,
        /// `minor`, `major`, `critical` or `maintenance`.
        #[garde(length(chars, min = 1, max = 32))]
        impact: compact_str::CompactString,
        /// `investigating`, `identified`, `monitoring` or `resolved`.
        #[garde(length(chars, min = 1, max = 32))]
        status: compact_str::CompactString,
        /// The affected components; empty: all of them.
        #[garde(length(max = 100))]
        #[serde(default)]
        components: Vec<uuid::Uuid>,
        /// The first update's text.
        #[garde(length(chars, min = 1, max = 5000))]
        message: compact_str::CompactString,
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

        if !super::valid_impact(&data.impact) {
            return super::super::bad_request("unknown impact");
        }
        if !super::valid_status(&data.status) {
            return super::super::bad_request("unknown incident status");
        }

        let uuid = shared::status::create_incident(
            &state.database,
            &data.title,
            &data.impact,
            &data.status,
            &data.components,
            &data.message,
            Some(user.uuid),
        )
        .await?;

        activity_logger
            .log(
                "status:incident.create",
                serde_json::json!({
                    "uuid": uuid,
                    "title": data.title,
                    "impact": data.impact,
                    "status": data.status,
                    "components": data.components,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { uuid }).ok()
    }
}

mod patch {
    use axum::extract::Path;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 255))]
        title: Option<compact_str::CompactString>,
        #[garde(length(chars, min = 1, max = 32))]
        impact: Option<compact_str::CompactString>,
        #[garde(length(max = 100))]
        components: Option<Vec<uuid::Uuid>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/{incident}", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("incident" = uuid::Uuid, Path, description = "The incident ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(incident): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if data
            .impact
            .as_deref()
            .is_some_and(|impact| !super::valid_impact(impact))
        {
            return super::super::bad_request("unknown impact");
        }

        if !shared::status::update_incident(
            &state.database,
            incident,
            data.title.as_deref(),
            data.impact.as_deref(),
            data.components.as_deref(),
        )
        .await?
        {
            return super::super::not_found("incident not found");
        }

        activity_logger
            .log(
                "status:incident.update",
                serde_json::json!({
                    "uuid": incident,
                    "title": data.title,
                    "impact": data.impact,
                    "components": data.components,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
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

    #[utoipa::path(delete, path = "/{incident}", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("incident" = uuid::Uuid, Path, description = "The incident ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(incident): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if !shared::status::delete_incident(&state.database, incident).await? {
            return super::super::not_found("incident not found");
        }

        activity_logger
            .log(
                "status:incident.delete",
                serde_json::json!({ "uuid": incident }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod post_update {
    use axum::extract::Path;
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
        /// The incident's new status; `resolved` resolves it.
        #[garde(length(chars, min = 1, max = 32))]
        status: compact_str::CompactString,
        #[garde(length(chars, min = 1, max = 5000))]
        message: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/{incident}/updates", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("incident" = uuid::Uuid, Path, description = "The incident ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(incident): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if !super::valid_status(&data.status) {
            return super::super::bad_request("unknown incident status");
        }

        if !shared::status::add_incident_update(
            &state.database,
            incident,
            &data.status,
            &data.message,
            Some(user.uuid),
        )
        .await?
        {
            return super::super::not_found("incident not found");
        }

        activity_logger
            .log(
                "status:incident.update-post",
                serde_json::json!({
                    "uuid": incident,
                    "status": data.status,
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
        .routes(routes!(patch::route))
        .routes(routes!(delete::route))
        .routes(routes!(post_update::route))
        .with_state(state.clone())
}
