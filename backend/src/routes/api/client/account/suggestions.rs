//! plugged.host: Account › Suggestions, the ideas board (shared/src/suggestions.rs,
//! docs/features/suggestions.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
        suggestions::ApiSuggestion,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        suggestions: Vec<ApiSuggestion>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = FORBIDDEN, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("suggestions.read")?;

        ApiResponse::new_serialized(Response {
            suggestions: shared::suggestions::list(&state, user.uuid).await?,
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
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
        suggestions::ApiSuggestion,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 3, max = 120))]
        title: String,
        /// One sentence; what the idea does and why.
        #[garde(length(chars, min = 10, max = 500))]
        body: String,
        /// `minecraft`, `fivem`, `rust`, `teamspeak` or `other`.
        #[garde(length(chars, min = 1, max = 16))]
        server_type: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        suggestion: ApiSuggestion,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_user_permission("suggestions.create")?;

        let suggestion_uuid = match shared::suggestions::create(
            &state,
            user.uuid,
            &user.username,
            shared::suggestions::Create {
                title: data.title,
                body: data.body,
                server_type: data.server_type,
            },
        )
        .await
        {
            Ok(suggestion_uuid) => suggestion_uuid,
            Err(err) => return err.response().ok(),
        };
        activity_logger
            .log(
                "account:suggestion.create",
                serde_json::json!({ "uuid": suggestion_uuid }),
            )
            .await;

        let suggestion = shared::suggestions::view(&state.database, user.uuid, suggestion_uuid)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the suggestion went missing"))?;
        ApiResponse::new_serialized(Response { suggestion }).ok()
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

    #[utoipa::path(get, path = "/{suggestion}", responses(
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
        permissions.has_user_permission("suggestions.read")?;
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
        suggestions::{ApiSuggestionThread, AUTHOR_USER},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 5000))]
        body: String,
    }

    #[utoipa::path(post, path = "/{suggestion}/comments", responses(
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
        permissions.has_user_permission("suggestions.read")?;

        match shared::suggestions::comment(
            &state,
            suggestion_uuid,
            user.uuid,
            AUTHOR_USER,
            &data.body,
        )
        .await
        {
            Ok(thread) => ApiResponse::new_serialized(thread).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

mod attachments {
    use axum::extract::{Multipart, Path};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
        suggestions::ApiSuggestionAttachment,
    };
    use utoipa::ToSchema;

    const MAX_BYTES: usize = 10 * 1024 * 1024;
    const MAX_FILES: usize = 6;

    #[derive(ToSchema, Serialize)]
    struct Response {
        attachments: Vec<ApiSuggestionAttachment>,
    }

    #[utoipa::path(post, path = "/{suggestion}/attachments", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        Path(suggestion_uuid): Path<uuid::Uuid>,
        mut multipart: Multipart,
    ) -> ApiResponseResult {
        permissions.has_user_permission("suggestions.create")?;
        let owner: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT user_uuid FROM suggestions WHERE uuid = $1")
                .bind(suggestion_uuid)
                .fetch_optional(state.database.read())
                .await?;
        let Some(owner) = owner else {
            return ApiResponse::error("suggestion not found")
                .with_status(axum::http::StatusCode::NOT_FOUND)
                .ok();
        };
        if owner != user.uuid && !user.admin {
            return ApiResponse::error("you can only add images to your own idea")
                .with_status(axum::http::StatusCode::FORBIDDEN)
                .ok();
        }

        let mut count = 0usize;
        while let Some(field) = multipart.next_field().await? {
            let Some(raw_name) = field.file_name() else {
                continue;
            };
            if count >= MAX_FILES {
                return ApiResponse::error("at most 6 files per idea")
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }
            let name = std::path::Path::new(raw_name)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file")
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect::<String>();
            if name.is_empty() || name.contains("..") {
                return ApiResponse::error("invalid file name")
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }
            let mime = shared::storage::content_type(&name);
            let allowed = mime.starts_with("image/")
                || matches!(
                    mime,
                    "application/pdf"
                        | "text/plain"
                        | "application/zip"
                        | "application/gzip"
                        | "application/json"
                );
            if !allowed || mime == "image/svg+xml" {
                return ApiResponse::error(
                    "only images, PDF, text, JSON and zip files are accepted",
                )
                .with_status(axum::http::StatusCode::BAD_REQUEST)
                .ok();
            }
            let data = field.bytes().await?;
            if data.len() > MAX_BYTES {
                return ApiResponse::error("each file must be 10 MB or less")
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }
            let attachment_uuid = uuid::Uuid::new_v4();
            let path = format!("suggestions/{suggestion_uuid}/{attachment_uuid}-{name}");
            state
                .storage
                .store(&path, std::io::Cursor::new(data.as_ref()), mime)
                .await?;
            sqlx::query(
                "INSERT INTO suggestion_attachments
                    (uuid, suggestion_uuid, uploader_uuid, name, mime, size, path)
                VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(attachment_uuid)
            .bind(suggestion_uuid)
            .bind(user.uuid)
            .bind(&name)
            .bind(mime)
            .bind(data.len() as i64)
            .bind(&path)
            .execute(state.database.write())
            .await?;
            count += 1;
        }

        ApiResponse::new_serialized(Response {
            attachments: shared::suggestions::attachments(&state, suggestion_uuid).await?,
        })
        .ok()
    }
}

mod vote {
    use axum::extract::Path;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
        suggestions::ApiSuggestion,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        suggestion: ApiSuggestion,
    }

    #[utoipa::path(post, path = "/{suggestion}/vote", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
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
        permissions.has_user_permission("suggestions.vote")?;

        match shared::suggestions::toggle_vote(&state, user.uuid, suggestion_uuid).await {
            Ok(suggestion) => ApiResponse::new_serialized(Response { suggestion }).ok(),
            Err(err) => err.response().ok(),
        }
    }
}

mod delete {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(delete, path = "/{suggestion}", responses(
        (status = OK, body = Object),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        ("suggestion" = uuid::Uuid, Path, description = "The suggestion ID"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        Path(suggestion_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("suggestions.create")?;
        if let Err(err) =
            shared::suggestions::delete_owned(&state, user.uuid, suggestion_uuid).await
        {
            return err.response().ok();
        }
        activity_logger
            .log(
                "account:suggestion.delete",
                serde_json::json!({ "uuid": suggestion_uuid }),
            )
            .await;
        ApiResponse::new_serialized(serde_json::json!({})).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .routes(routes!(thread::route))
        .routes(routes!(comment::route))
        .routes(routes!(attachments::route))
        .routes(routes!(vote::route))
        .routes(routes!(delete::route))
        .with_state(state.clone())
}
