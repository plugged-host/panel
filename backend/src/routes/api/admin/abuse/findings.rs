//! plugged.host: what the abuse checks found, and an admin closing a finding.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        abuse::AdminApiAbuseFinding,
        models::{Pagination, user::GetPermissionManager},
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
        /// `open`, `actioned`, `dismissed` or `resolved`; none: every status.
        #[garde(skip)]
        #[serde(default)]
        status: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        findings: Pagination<AdminApiAbuseFinding>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        ("page" = i64, Query, description = "The page number", example = "1"),
        ("per_page" = i64, Query, description = "The number of items per page", example = "10"),
        ("search" = Option<String>, Query, description = "Search term for items"),
        ("status" = Option<String>, Query, description = "Only findings with this status"),
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

        permissions.has_admin_permission("abuse.read")?;

        let status = params.status.as_deref().filter(|status| !status.is_empty());
        if let Some(status) = status
            && !shared::abuse::STATUSES.contains(&status)
        {
            return super::super::bad_request("unknown status");
        }

        ApiResponse::new_serialized(Response {
            findings: shared::abuse::findings(
                &state.database,
                status,
                params.search.as_deref(),
                params.page,
                params.per_page,
            )
            .await?,
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
        models::{
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// `dismissed` (a false positive: it stays dismissed when seen again, and an abuse
        /// suspension of its server no longer deletes the server) or `resolved`.
        #[garde(length(chars, min = 1, max = 16))]
        status: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/{finding}", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = shared::ApiError),
        (status = NOT_FOUND, body = shared::ApiError),
    ), params(
        ("finding" = uuid::Uuid, Path, description = "The abuse finding ID"),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        Path(finding_uuid): Path<uuid::Uuid>,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("abuse.manage")?;

        if ![
            shared::abuse::STATUS_DISMISSED,
            shared::abuse::STATUS_RESOLVED,
        ]
        .contains(&data.status.as_str())
        {
            return super::super::bad_request("a finding can only be dismissed or resolved");
        }

        let Some(server_uuid) =
            shared::abuse::close_finding(&state.database, finding_uuid, &data.status, user.uuid)
                .await?
        else {
            return super::super::not_found("abuse finding not found");
        };

        activity_logger
            .log(
                if data.status == shared::abuse::STATUS_DISMISSED {
                    "abuse:finding.dismiss"
                } else {
                    "abuse:finding.resolve"
                },
                serde_json::json!({ "uuid": finding_uuid, "server_uuid": server_uuid }),
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
