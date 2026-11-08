//! plugged.host: Admin › Suggestions (shared/src/suggestions.rs, docs/features/suggestions.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            Pagination,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        suggestions::{ApiSuggestion, STATUSES},
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
        #[garde(skip)]
        #[serde(default)]
        status: Option<compact_str::CompactString>,
        #[garde(skip)]
        #[serde(default)]
        server_type: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        suggestions: Pagination<ApiSuggestion>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        ("page" = i64, Query, description = "The page number", example = "1"),
        ("per_page" = i64, Query, description = "The number of items per page", example = "10"),
        ("search" = Option<String>, Query, description = "In the title, the description or the author"),
        ("status" = Option<String>, Query, description = "under_review, planned, in_progress, shipped, declined or merged"),
        ("server_type" = Option<String>, Query, description = "minecraft, fivem, rust, teamspeak or other"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&params) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("suggestions.read")?;

        let status = params.status.as_deref().filter(|status| !status.is_empty());
        if let Some(status) = status
            && !STATUSES.contains(&status)
        {
            return ApiResponse::error("unknown status")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        let server_type = params
            .server_type
            .as_deref()
            .filter(|server_type| !server_type.is_empty());
        if let Some(server_type) = server_type
            && !shared::suggestions::SERVER_TYPES.contains(&server_type)
        {
            return ApiResponse::error("unknown server type")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        ApiResponse::new_serialized(Response {
            suggestions: shared::suggestions::admin_list(
                &state,
                user.uuid,
                shared::suggestions::AdminFilter {
                    status,
                    server_type,
                    search: params.search.as_deref(),
                },
                params.page,
                params.per_page,
            )
            .await?,
        })
        .ok()
    }
}

mod suggestion {
    use serde::Serialize;
    use shared::{
        response::{ApiResponse, ApiResponseResult},
        suggestions::ApiSuggestion,
    };
    use utoipa::ToSchema;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(ToSchema, Serialize)]
    pub struct ActionResponse {
        suggestion: ApiSuggestion,
    }

    async fn answer(
        state: &shared::State,
        viewer_uuid: uuid::Uuid,
        suggestion_uuid: uuid::Uuid,
    ) -> ApiResponseResult {
        match shared::suggestions::view(&state.database, viewer_uuid, suggestion_uuid).await? {
            Some(suggestion) => ApiResponse::new_serialized(ActionResponse { suggestion }).ok(),
            None => ApiResponse::error("suggestion not found")
                .with_status(axum::http::StatusCode::NOT_FOUND)
                .ok(),
        }
    }

    mod thread {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::user::{GetPermissionManager, GetUser},
            response::{ApiResponse, ApiResponseResult},
            suggestions::ApiSuggestionThread,
        };

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(ApiSuggestionThread)),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            Path(suggestion_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("suggestions.read")?;
            match shared::suggestions::thread(&state, user.uuid, suggestion_uuid).await? {
                Some(thread) => ApiResponse::new_serialized(thread).ok(),
                None => ApiResponse::error("suggestion not found")
                    .with_status(axum::http::StatusCode::NOT_FOUND)
                    .ok(),
            }
        }
    }

    mod comment {
        use axum::extract::Path;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            models::user::{GetPermissionManager, GetUser},
            response::{ApiResponse, ApiResponseResult},
            suggestions::{AUTHOR_ADMIN, ApiSuggestionThread},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(length(chars, min = 1, max = 5000))]
            body: String,
        }

        #[utoipa::path(post, path = "/comments", responses(
            (status = OK, body = inline(ApiSuggestionThread)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            Path(suggestion_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }
            permissions.has_admin_permission("suggestions.manage")?;

            match shared::suggestions::comment(
                &state,
                suggestion_uuid,
                user.uuid,
                AUTHOR_ADMIN,
                &data.body,
            )
            .await
            {
                Ok(thread) => ApiResponse::new_serialized(thread).ok(),
                Err(err) => err.response().ok(),
            }
        }
    }

    mod patch {
        use axum::extract::Path;
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
            /// `under_review`, `planned`, `in_progress`, `shipped` or `declined`.
            #[garde(length(chars, min = 1, max = 16))]
            status: String,
            /// What shipped, shown on the board when the status is `shipped`.
            #[garde(length(chars, max = 5000))]
            #[serde(default)]
            changelog: Option<String>,
        }

        #[utoipa::path(patch, path = "/", responses(
            (status = OK, body = inline(super::ActionResponse)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetAdminActivityLogger,
            Path(suggestion_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("suggestions.manage")?;

            if let Err(err) = shared::suggestions::update(
                &state,
                suggestion_uuid,
                &data.status,
                data.changelog.as_deref(),
            )
            .await
            {
                return err.response().ok();
            }
            activity_logger
                .log(
                    "suggestions:suggestion.update",
                    serde_json::json!({
                        "uuid": suggestion_uuid,
                        "status": data.status,
                    }),
                )
                .await;

            super::answer(&state, user.uuid, suggestion_uuid).await
        }
    }

    mod merge {
        use axum::extract::Path;
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
            /// The idea the votes move to; the merged one is hidden from the board.
            #[garde(skip)]
            target_uuid: uuid::Uuid,
        }

        #[utoipa::path(post, path = "/merge", responses(
            (status = OK, body = inline(super::ActionResponse)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetAdminActivityLogger,
            Path(suggestion_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("suggestions.manage")?;

            if let Err(err) =
                shared::suggestions::merge(&state, suggestion_uuid, data.target_uuid).await
            {
                return err.response().ok();
            }
            activity_logger
                .log(
                    "suggestions:suggestion.merge",
                    serde_json::json!({
                        "uuid": suggestion_uuid,
                        "target_uuid": data.target_uuid,
                    }),
                )
                .await;

            super::answer(&state, user.uuid, suggestion_uuid).await
        }
    }

    mod delete {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(delete, path = "/", responses(
            (status = OK, body = Object),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            activity_logger: GetAdminActivityLogger,
            Path(suggestion_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("suggestions.manage")?;

            if let Err(err) = shared::suggestions::delete(&state, suggestion_uuid).await {
                return err.response().ok();
            }
            activity_logger
                .log(
                    "suggestions:suggestion.delete",
                    serde_json::json!({ "uuid": suggestion_uuid }),
                )
                .await;

            ApiResponse::new_serialized(serde_json::json!({})).ok()
        }
    }

    pub fn router(state: &super::super::State) -> OpenApiRouter<super::super::State> {
        OpenApiRouter::new()
            .routes(routes!(thread::route))
            .routes(routes!(comment::route))
            .routes(routes!(patch::route))
            .routes(routes!(merge::route))
            .routes(routes!(delete::route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/{suggestion}", suggestion::router(state))
        .with_state(state.clone())
}
