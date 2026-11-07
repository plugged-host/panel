//! plugged.host: Account › Support, the account's tickets (shared/src/tickets.rs,
//! docs/features/tickets.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
        tickets::ApiTicketSummary,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        tickets: Vec<ApiTicketSummary>,
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
        permissions.has_user_permission("tickets.read")?;

        ApiResponse::new_serialized(Response {
            tickets: shared::tickets::list(&state.database, user.uuid).await?,
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
        tickets::ApiTicket,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 3, max = 200))]
        subject: String,
        /// `general`, `billing`, `technical`, `abuse` or `other`.
        #[garde(length(chars, min = 1, max = 16))]
        category: String,
        /// One of the account's servers, for context; optional.
        #[garde(skip)]
        server_uuid: Option<uuid::Uuid>,
        #[garde(length(chars, min = 1, max = 20000))]
        body: String,
        /// A bug report's reproduction steps (category `bug`).
        #[garde(length(chars, max = 20000))]
        #[serde(default)]
        bug_steps: Option<String>,
        #[garde(length(chars, max = 20000))]
        #[serde(default)]
        bug_expected: Option<String>,
        #[garde(length(chars, max = 20000))]
        #[serde(default)]
        bug_actual: Option<String>,
        #[garde(length(chars, max = 2000))]
        #[serde(default)]
        bug_environment: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[serde(flatten)]
        ticket: ApiTicket,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
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

        permissions.has_user_permission("tickets.create")?;

        let ticket_uuid = match shared::tickets::create(
            &state,
            &user,
            shared::tickets::Create {
                subject: data.subject,
                category: data.category,
                server_uuid: data.server_uuid,
                body: data.body,
                bug_steps: data.bug_steps,
                bug_expected: data.bug_expected,
                bug_actual: data.bug_actual,
                bug_environment: data.bug_environment,
            },
        )
        .await
        {
            Ok(ticket_uuid) => ticket_uuid,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "account:ticket.create",
                serde_json::json!({ "uuid": ticket_uuid }),
            )
            .await;

        let ticket = shared::tickets::view(&state.database, user.uuid, ticket_uuid)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the ticket went missing"))?;
        ApiResponse::new_serialized(Response { ticket }).ok()
    }
}

mod ticket {
    use axum::extract::Path;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
        tickets::ApiTicket,
    };
    use utoipa_axum::router::OpenApiRouter;
    use utoipa_axum::routes;

    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[serde(flatten)]
        ticket: ApiTicket,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
    ))]
    pub async fn get_route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        Path(ticket_uuid): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("tickets.read")?;

        // An existing ticket with no neo/system answer gets one on open (the task checks first).
        shared::tickets::spawn_neo_reply(&state, ticket_uuid);
        match shared::tickets::view(&state.database, user.uuid, ticket_uuid).await? {
            Some(mut ticket) => {
                shared::tickets::with_attachments(&state, &mut ticket).await?;
                ApiResponse::new_serialized(Response { ticket }).ok()
            }
            None => ApiResponse::error("ticket not found")
                .with_status(axum::http::StatusCode::NOT_FOUND)
                .ok(),
        }
    }

    mod reply {
        use axum::extract::Path;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            models::{
                user::{GetPermissionManager, GetUser},
                user_activity::GetUserActivityLogger,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(length(chars, min = 1, max = 20000))]
            body: String,
        }

        #[utoipa::path(post, path = "/reply", responses(
            (status = OK, body = inline(super::Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
            (status = CONFLICT, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetUserActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_user_permission("tickets.create")?;

            let ticket =
                match shared::tickets::reply(&state, user.uuid, ticket_uuid, &data.body, false)
                    .await
                {
                    Ok(ticket) => ticket,
                    Err(err) => return err.response().ok(),
                };
            activity_logger
                .log(
                    "account:ticket.reply",
                    serde_json::json!({ "uuid": ticket_uuid }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
        }
    }

    mod status {
        use axum::extract::Path;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            models::{
                user::{GetPermissionManager, GetUser},
                user_activity::GetUserActivityLogger,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Deserialize)]
        pub struct Payload {
            /// `open` or `closed`.
            status: String,
        }

        #[utoipa::path(post, path = "/status", responses(
            (status = OK, body = inline(super::Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetUserActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            permissions.has_user_permission("tickets.create")?;

            let ticket = match shared::tickets::set_status(
                &state,
                ticket_uuid,
                Some(user.uuid),
                &data.status,
            )
            .await
            {
                Ok(ticket) => ticket,
                Err(err) => return err.response().ok(),
            };
            activity_logger
                .log(
                    "account:ticket.status",
                    serde_json::json!({ "uuid": ticket_uuid, "status": data.status }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
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

        #[utoipa::path(delete, path = "/", responses(
            (status = OK, body = Object),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetUserActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_user_permission("tickets.create")?;
            match shared::tickets::delete(&state, ticket_uuid, Some(user.uuid)).await {
                Ok(true) => {}
                Ok(false) => {
                    return ApiResponse::error("ticket not found")
                        .with_status(axum::http::StatusCode::NOT_FOUND)
                        .ok();
                }
                Err(err) => return err.response().ok(),
            }
            activity_logger
                .log(
                    "account:ticket.delete",
                    serde_json::json!({ "uuid": ticket_uuid }),
                )
                .await;
            ApiResponse::new_serialized(serde_json::json!({})).ok()
        }
    }

    pub fn router(state: &super::State) -> OpenApiRouter<super::State> {
        OpenApiRouter::new()
            .routes(routes!(get_route))
            .routes(routes!(delete::route))
            .routes(routes!(reply::route))
            .routes(routes!(status::route))
            .nest("/attachments", attachments::router(state))
            .with_state(state.clone())
    }

    mod attachments {
        use axum::extract::{Multipart, State};
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::user::{GetPermissionManager, GetUser},
            response::{ApiResponse, ApiResponseResult},
            tickets::ApiTicketAttachment,
        };
        use utoipa::ToSchema;
        use utoipa_axum::{router::OpenApiRouter, routes};

        const MAX_BYTES: usize = 10 * 1024 * 1024;
        const MAX_FILES: usize = 6;

        #[derive(ToSchema, Serialize)]
        struct Response {
            attachments: Vec<ApiTicketAttachment>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            axum::extract::Path(ticket_uuid): axum::extract::Path<uuid::Uuid>,
            mut multipart: Multipart,
        ) -> ApiResponseResult {
            permissions.has_user_permission("tickets.create")?;
            if !shared::tickets::owns_ticket(&state.database, ticket_uuid, user.uuid).await? {
                return ApiResponse::error("ticket not found")
                    .with_status(axum::http::StatusCode::NOT_FOUND)
                    .ok();
            }

            let mut count = 0usize;
            while let Some(field) = multipart.next_field().await? {
                let Some(raw_name) = field.file_name() else {
                    continue;
                };
                if count >= MAX_FILES {
                    return ApiResponse::error("at most 6 files per ticket")
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
                let path = format!("tickets/{ticket_uuid}/{attachment_uuid}-{name}");
                state
                    .storage
                    .store(&path, std::io::Cursor::new(data.as_ref()), mime)
                    .await?;
                sqlx::query(
                    "INSERT INTO ticket_attachments (uuid, ticket_uuid, uploader_uuid, name, mime, size, path)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(attachment_uuid)
                .bind(ticket_uuid)
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
                attachments: shared::tickets::attachments(&state, ticket_uuid).await?,
            })
            .ok()
        }

        pub fn router(state: &super::super::State) -> OpenApiRouter<super::super::State> {
            OpenApiRouter::new()
                .routes(routes!(route))
                .with_state(state.clone())
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .nest("/{ticket}", ticket::router(state))
        .with_state(state.clone())
}
