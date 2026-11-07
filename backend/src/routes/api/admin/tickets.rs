//! plugged.host: Admin › Tickets (shared/src/tickets.rs, docs/features/tickets.md).

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
        tickets::{ApiTicketSummary, STATUSES},
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
        category: Option<compact_str::CompactString>,
        /// Only tickets assigned to this administrator (`me` for the caller).
        #[garde(skip)]
        #[serde(default)]
        assigned: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        tickets: Pagination<ApiTicketSummary>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        ("page" = i64, Query, description = "The page number", example = "1"),
        ("per_page" = i64, Query, description = "The number of items per page", example = "10"),
        ("search" = Option<String>, Query, description = "In the subject, the account or the server"),
        ("status" = Option<String>, Query, description = "open, waiting, resolved or closed"),
        ("category" = Option<String>, Query, description = "general, billing, technical, abuse or other"),
        ("assigned" = Option<String>, Query, description = "`me`, or an administrator's UUID"),
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

        permissions.has_admin_permission("tickets.read")?;

        let status = params.status.as_deref().filter(|status| !status.is_empty());
        if let Some(status) = status
            && !STATUSES.contains(&status)
        {
            return ApiResponse::error("unknown status")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        let category = params
            .category
            .as_deref()
            .filter(|category| !category.is_empty());
        if let Some(category) = category
            && !shared::tickets::CATEGORIES.contains(&category)
        {
            return ApiResponse::error("unknown category")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        let assigned_to = match params.assigned.as_deref() {
            Some("me") => Some(user.uuid),
            Some(value) => match value.parse::<uuid::Uuid>() {
                Ok(uuid) => Some(uuid),
                Err(_) => {
                    return ApiResponse::error("unknown assignee")
                        .with_status(StatusCode::BAD_REQUEST)
                        .ok();
                }
            },
            None => None,
        };

        ApiResponse::new_serialized(Response {
            tickets: shared::tickets::admin_list(
                &state,
                shared::tickets::AdminFilter {
                    status,
                    category,
                    search: params.search.as_deref(),
                    assigned_to,
                },
                params.page,
                params.per_page,
            )
            .await?,
        })
        .ok()
    }
}

mod ticket {
    use axum::extract::Path;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        tickets::ApiTicket,
    };
    use utoipa::ToSchema;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        #[serde(flatten)]
        pub ticket: ApiTicket,
    }

    async fn answer(
        state: &shared::State,
        ticket_uuid: uuid::Uuid,
    ) -> shared::response::ApiResponseResult {
        // Backfill an answer for a ticket opened before neo could reply.
        shared::tickets::spawn_neo_reply(state, ticket_uuid);
        match shared::tickets::admin_view(&state.database, ticket_uuid).await? {
            Some(ticket) => ApiResponse::new_serialized(Response { ticket }).ok(),
            None => ApiResponse::error("ticket not found")
                .with_status(axum::http::StatusCode::NOT_FOUND)
                .ok(),
        }
    }

    mod get_route {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState, models::user::GetPermissionManager, response::ApiResponseResult,
        };

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(super::Response)),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            Path(ticket_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("tickets.read")?;

            super::answer(&state, ticket_uuid).await
        }
    }

    mod reply {
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
            #[garde(length(chars, min = 1, max = 20000))]
            body: String,
        }

        #[utoipa::path(post, path = "/reply", responses(
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
            activity_logger: GetAdminActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("tickets.manage")?;

            let ticket = match shared::tickets::reply(
                &state,
                user.uuid,
                ticket_uuid,
                &data.body,
                true,
            )
            .await
            {
                Ok(ticket) => ticket,
                Err(err) => return err.response().ok(),
            };
            activity_logger
                .log(
                    "tickets:ticket.reply",
                    serde_json::json!({ "uuid": ticket_uuid }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
        }
    }

    mod assign {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{
                admin_activity::GetAdminActivityLogger,
                user::{GetPermissionManager, GetUser},
            },
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(post, path = "/assign", responses(
            (status = OK, body = inline(super::Response)),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetAdminActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("tickets.manage")?;

            let ticket = match shared::tickets::assign(&state, ticket_uuid, Some(user.uuid)).await {
                Ok(ticket) => ticket,
                Err(err) => return err.response().ok(),
            };
            activity_logger
                .log(
                    "tickets:ticket.assign",
                    serde_json::json!({ "uuid": ticket_uuid, "assigned_to": user.uuid }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
        }
    }

    mod unassign {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(delete, path = "/assign", responses(
            (status = OK, body = inline(super::Response)),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            activity_logger: GetAdminActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("tickets.manage")?;

            let ticket = match shared::tickets::assign(&state, ticket_uuid, None).await {
                Ok(ticket) => ticket,
                Err(err) => return err.response().ok(),
            };
            activity_logger
                .log(
                    "tickets:ticket.unassign",
                    serde_json::json!({ "uuid": ticket_uuid }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
        }
    }

    mod patch {
        use axum::extract::Path;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// `open`, `waiting`, `resolved` or `closed`.
            #[garde(length(chars, min = 1, max = 16))]
            #[serde(default)]
            status: Option<String>,
            /// `low`, `normal` or `high`.
            #[garde(length(chars, min = 1, max = 16))]
            #[serde(default)]
            priority: Option<String>,
        }

        #[utoipa::path(patch, path = "/", responses(
            (status = OK, body = inline(super::Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
        ), params(
            ("ticket" = uuid::Uuid, Path, description = "The ticket ID"),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            activity_logger: GetAdminActivityLogger,
            Path(ticket_uuid): Path<uuid::Uuid>,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }

            permissions.has_admin_permission("tickets.manage")?;

            let mut ticket = match shared::tickets::admin_view(&state.database, ticket_uuid).await?
            {
                Some(ticket) => ticket,
                None => {
                    return ApiResponse::error("ticket not found")
                        .with_status(axum::http::StatusCode::NOT_FOUND)
                        .ok();
                }
            };
            if let Some(status) = data.status.as_deref() {
                ticket = match shared::tickets::set_status(&state, ticket_uuid, None, status).await
                {
                    Ok(ticket) => ticket,
                    Err(err) => return err.response().ok(),
                };
            }
            if let Some(priority) = data.priority.as_deref() {
                ticket = match shared::tickets::set_priority(&state, ticket_uuid, priority).await {
                    Ok(ticket) => ticket,
                    Err(err) => return err.response().ok(),
                };
            }
            activity_logger
                .log(
                    "tickets:ticket.update",
                    serde_json::json!({
                        "uuid": ticket_uuid,
                        "status": data.status,
                        "priority": data.priority,
                    }),
                )
                .await;

            ApiResponse::new_serialized(super::Response { ticket }).ok()
        }
    }

    pub fn router(state: &super::super::State) -> OpenApiRouter<super::super::State> {
        OpenApiRouter::new()
            .routes(routes!(get_route::route))
            .routes(routes!(reply::route))
            .routes(routes!(assign::route, unassign::route))
            .routes(routes!(patch::route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/{ticket}", ticket::router(state))
        .with_state(state.clone())
}
