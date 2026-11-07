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

        match shared::tickets::view(&state.database, user.uuid, ticket_uuid).await? {
            Some(ticket) => ApiResponse::new_serialized(Response { ticket }).ok(),
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

    pub fn router(state: &super::State) -> OpenApiRouter<super::State> {
        OpenApiRouter::new()
            .routes(routes!(get_route))
            .routes(routes!(reply::route))
            .routes(routes!(status::route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .nest("/{ticket}", ticket::router(state))
        .with_state(state.clone())
}
