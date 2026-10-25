//! plugged.host: Admin › Partners, deciding partner applications and running partnerships
//! (shared/src/partners.rs, docs/features/partners.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// What an action answers: the application as it is now.
#[derive(utoipa::ToSchema, serde::Serialize)]
pub struct ActionResponse {
    application: Option<shared::partners::AdminApiPartnerApplication>,
}

async fn answer(
    state: &shared::State,
    application_uuid: uuid::Uuid,
) -> shared::response::ApiResponseResult {
    shared::response::ApiResponse::new_serialized(ActionResponse {
        application: shared::partners::admin_view(state, application_uuid).await?,
    })
    .ok()
}

/// What the activity log keeps of a decision.
fn logged(
    application_uuid: uuid::Uuid,
    decided: &shared::partners::Decided,
    extra: serde_json::Value,
) -> serde_json::Value {
    let mut data = serde_json::json!({
        "uuid": application_uuid,
        "user_uuid": decided.user_uuid,
        "guild_id": decided.guild_id,
        "guild_name": decided.guild_name,
    });
    if let (Some(data), serde_json::Value::Object(extra)) = (data.as_object_mut(), extra) {
        data.extend(extra);
    }

    data
}

mod get {
    use axum::{extract::Query, http::StatusCode};
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{Pagination, user::GetPermissionManager},
        partners::AdminApiPartnerApplication,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Params {
        #[garde(range(min = 1))]
        #[serde(default = "Pagination::default_page")]
        page: i64,
        #[garde(range(min = 1, max = 100))]
        #[serde(default = "Pagination::default_per_page")]
        per_page: i64,
        #[garde(length(chars, min = 1, max = 128))]
        #[serde(
            default,
            deserialize_with = "shared::deserialize::deserialize_search_option"
        )]
        search: Option<compact_str::CompactString>,
        /// A status (`pending`, `approved`, `paused`, `suspended`, `rejected`, `revoked`,
        /// `withdrawn`) or `partners` (approved, paused and suspended); none: every status.
        #[garde(skip)]
        #[serde(default)]
        status: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        applications: Pagination<AdminApiPartnerApplication>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        ("page" = i64, Query, description = "The page number", example = "1"),
        ("per_page" = i64, Query, description = "The number of items per page", example = "10"),
        ("search" = Option<String>, Query, description = "In the Discord server's name or id and the applicant's username"),
        ("status" = Option<String>, Query, description = "Only applications with this status, or `partners`"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&params) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.read")?;

        let status = params.status.as_deref().filter(|status| !status.is_empty());
        if let Some(status) = status
            && status != "partners"
            && !shared::partners::STATUSES.contains(&status)
        {
            return ApiResponse::error("unknown status")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        ApiResponse::new_serialized(Response {
            applications: shared::partners::admin_list(
                &state,
                shared::partners::AdminFilter {
                    status,
                    search: params.search.as_deref(),
                    uuid: None,
                },
                params.page,
                params.per_page,
            )
            .await?,
        })
        .ok()
    }
}

mod approve {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Credits a month, 0 up to the settings' largest amount; the first month is paid now.
        #[garde(range(min = 0))]
        credits: i64,
    }

    #[utoipa::path(post, path = "/{application}/approve", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.manage")?;

        let decided = match shared::partners::approve(
            &state,
            application_uuid,
            data.credits,
            user.uuid,
        )
        .await
        {
            Ok(decided) => decided,
            Err(err) => return err.response(),
        };
        activity_logger
            .log(
                "partners:application.approve",
                super::logged(
                    application_uuid,
                    &decided,
                    serde_json::json!({ "credits": data.credits }),
                ),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod reject {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Shown to the applicant.
        #[garde(length(chars, min = 1, max = 500))]
        reason: String,
    }

    #[utoipa::path(post, path = "/{application}/reject", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.manage")?;

        let decided =
            match shared::partners::reject(&state, application_uuid, &data.reason, user.uuid).await
            {
                Ok(decided) => decided,
                Err(err) => return err.response(),
            };
        activity_logger
            .log(
                "partners:application.reject",
                super::logged(
                    application_uuid,
                    &decided,
                    serde_json::json!({ "reason": data.reason }),
                ),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod recheck {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        response::ApiResponseResult,
    };

    #[utoipa::path(post, path = "/{application}/recheck", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
        (status = SERVICE_UNAVAILABLE, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("partners.manage")?;

        if let Err(err) = shared::partners::recheck(&state, application_uuid).await {
            return err.response();
        }
        activity_logger
            .log(
                "partners:application.recheck",
                serde_json::json!({ "uuid": application_uuid }),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod revoke {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Shown to the partner.
        #[garde(length(chars, min = 1, max = 500))]
        reason: String,
    }

    #[utoipa::path(post, path = "/{application}/revoke", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.manage")?;

        let decided =
            match shared::partners::revoke(&state, application_uuid, &data.reason, user.uuid).await
            {
                Ok(decided) => decided,
                Err(err) => return err.response(),
            };
        activity_logger
            .log(
                "partners:application.revoke",
                super::logged(
                    application_uuid,
                    &decided,
                    serde_json::json!({ "reason": data.reason }),
                ),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod pause {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Shown to the partner; may be empty.
        #[garde(length(chars, max = 500))]
        #[serde(default)]
        reason: String,
    }

    #[utoipa::path(post, path = "/{application}/pause", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.manage")?;

        let decided = match shared::partners::pause(
            &state,
            application_uuid,
            &data.reason,
            user.uuid,
        )
        .await
        {
            Ok(decided) => decided,
            Err(err) => return err.response(),
        };
        activity_logger
            .log(
                "partners:application.pause",
                super::logged(
                    application_uuid,
                    &decided,
                    serde_json::json!({ "reason": data.reason }),
                ),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod resume {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::ApiResponseResult,
    };

    #[utoipa::path(post, path = "/{application}/resume", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("partners.manage")?;

        let decided = match shared::partners::resume(&state, application_uuid, user.uuid).await {
            Ok(decided) => decided,
            Err(err) => return err.response(),
        };
        activity_logger
            .log(
                "partners:application.resume",
                super::logged(application_uuid, &decided, serde_json::json!({})),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

mod credits {
    use axum::{extract::Path, http::StatusCode};
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Credits a month from the next payment on, 0 up to the settings' largest amount.
        #[garde(range(min = 0))]
        credits: i64,
    }

    #[utoipa::path(put, path = "/{application}/credits", responses(
        (status = OK, body = inline(super::ActionResponse)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        ("application" = uuid::Uuid, Path, description = "The partner application ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(application_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("partners.manage")?;

        let (decided, before) =
            match shared::partners::set_credits(&state, application_uuid, data.credits, user.uuid)
                .await
            {
                Ok(changed) => changed,
                Err(err) => return err.response(),
            };
        activity_logger
            .log(
                "partners:application.credits",
                super::logged(
                    application_uuid,
                    &decided,
                    serde_json::json!({ "before": before, "credits": data.credits }),
                ),
            )
            .await;

        super::answer(&state, application_uuid).await
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(approve::route))
        .routes(routes!(reject::route))
        .routes(routes!(recheck::route))
        .routes(routes!(revoke::route))
        .routes(routes!(pause::route))
        .routes(routes!(resume::route))
        .routes(routes!(credits::route))
        .with_state(state.clone())
}
