// plugged.host: one plan in Admin › Plans (docs/features/create-server.md).

use super::State;
use axum::{
    extract::{Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use shared::{
    GetState,
    models::{ByUuid, plan::Plan, user::GetPermissionManager},
    response::ApiResponse,
};
use utoipa_axum::{router::OpenApiRouter, routes};

pub type GetPlan = shared::extract::ConsumingExtension<Plan>;

pub async fn auth(
    state: GetState,
    permissions: GetPermissionManager,
    Path(plan): Path<Vec<String>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let plan = match plan.first().map(|s| s.parse::<uuid::Uuid>()) {
        Some(Ok(id)) => id,
        _ => {
            return Ok(ApiResponse::error("invalid plan uuid")
                .with_status(StatusCode::BAD_REQUEST)
                .into_response());
        }
    };

    if let Err(err) = permissions.has_admin_permission("plans.read") {
        return Ok(err.into_response());
    }

    let plan = match Plan::by_uuid_optional(&state.database, plan).await {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            return Ok(ApiResponse::error("plan not found")
                .with_status(StatusCode::NOT_FOUND)
                .into_response());
        }
        Err(err) => return Ok(ApiResponse::from(err).into_response()),
    };

    req.extensions_mut().insert(plan);

    Ok(next.run(req).await)
}

mod get {
    use crate::routes::api::admin::plans::_plan_::GetPlan;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{IntoAdminApiObject, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        plan: shared::models::plan::AdminApiPlan,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "plan" = uuid::Uuid,
            description = "The plan ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        plan: GetPlan,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("plans.read")?;

        ApiResponse::new_serialized(Response {
            plan: plan.0.into_admin_api_object(&state, ()).await?,
        })
        .ok()
    }
}

mod delete {
    use crate::routes::api::admin::plans::_plan_::GetPlan;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            DeletableModel, admin_activity::GetAdminActivityLogger, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "plan" = uuid::Uuid,
            description = "The plan ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        plan: GetPlan,
        activity_logger: GetAdminActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("plans.delete")?;

        // Servers keep their resources: a plan is only the preset they were created from.
        plan.delete(&state, ()).await?;

        activity_logger
            .log(
                "plan:delete",
                serde_json::json!({
                    "uuid": plan.uuid,
                    "name": plan.name,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod patch {
    use crate::routes::api::admin::plans::_plan_::GetPlan;
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel, admin_activity::GetAdminActivityLogger, plan::UpdatePlanOptions,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_REQUEST, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "plan" = uuid::Uuid,
            description = "The plan ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(UpdatePlanOptions))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut plan: GetPlan,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<UpdatePlanOptions>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("plans.update")?;

        match plan.update(&state, data).await {
            Ok(_) => {}
            Err(err) if err.is_unique_violation() => {
                return ApiResponse::error("plan with name already exists")
                    .with_status(StatusCode::CONFLICT)
                    .ok();
            }
            Err(err) => return ApiResponse::from(err).ok(),
        }

        activity_logger
            .log(
                "plan:update",
                serde_json::json!({
                    "uuid": plan.uuid,
                    "name": plan.name,
                    "cpu": plan.cpu,
                    "memory": plan.memory,
                    "disk": plan.disk,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(delete::route))
        .routes(routes!(patch::route))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone())
}
