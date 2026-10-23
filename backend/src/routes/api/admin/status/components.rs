//! plugged.host: the status page's components (the website, the panel, one per node).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        status::AdminApiStatusComponent,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        components: Vec<AdminApiStatusComponent>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("status.read")?;

        ApiResponse::new_serialized(Response {
            components: shared::status::components(&state.database).await?,
        })
        .ok()
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
        #[garde(skip)]
        published: Option<bool>,
        #[garde(range(min = 0, max = 100000))]
        sort: Option<i32>,
        /// Only the website's and the panel's: a node's label follows its name.
        #[garde(length(chars, min = 1, max = 255))]
        label: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/{component}", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("component" = uuid::Uuid, Path, description = "The status component ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(component): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("status.manage")?;

        if !shared::status::update_component(
            &state.database,
            component,
            data.published,
            data.sort,
            data.label.as_deref(),
        )
        .await?
        {
            return super::super::not_found("status component not found");
        }

        activity_logger
            .log(
                "status:component.update",
                serde_json::json!({
                    "uuid": component,
                    "published": data.published,
                    "sort": data.sort,
                    "label": data.label,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(patch::route))
        .with_state(state.clone())
}
