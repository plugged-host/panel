// plugged.host: the chat between a server's owner and its subusers (docs/features/server-chat.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject, Pagination, PaginationParams,
            server::GetServer,
            server_chat::{ApiServerChatMessage, ServerChatMessage},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        messages: Pagination<ApiServerChatMessage>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "page" = i64, Query,
            description = "The page number",
            example = "1",
        ),
        (
            "per_page" = i64, Query,
            description = "The number of items per page",
            example = "50",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        Query(pagination): Query<PaginationParams>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&pagination) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("chat.read")?;

        let messages = ServerChatMessage::by_server_uuid_with_pagination(
            &state.database,
            server.uuid,
            pagination.page,
            pagination.per_page,
        )
        .await?;
        let storage_url_retriever = state.storage.retrieve_urls().await?;

        let mut messages = messages
            .try_async_map(|message| async {
                let user = match &message.user {
                    Some(user) => Some(
                        user.clone()
                            .into_api_object(&state, &storage_url_retriever)
                            .await?,
                    ),
                    None => None,
                };

                Ok::<ApiServerChatMessage, shared::database::DatabaseError>(
                    ApiServerChatMessage::from_message(message, user),
                )
            })
            .await?;
        // The query returns the newest page first; a chat reads top to bottom.
        messages.data.reverse();

        ApiResponse::new_serialized(Response { messages }).ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject,
            server::GetServer,
            server_chat::{ApiServerChatMessage, ServerChatMessage},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 1, max = 2000))]
        #[schema(min_length = 1, max_length = 2000)]
        message: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        message: ApiServerChatMessage,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = BAD_REQUEST, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("chat.write")?;

        let message = data.message.trim();
        if message.is_empty() {
            return ApiResponse::error("the message is empty")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let uuid =
            ServerChatMessage::create(&state.database, server.uuid, user.uuid, message).await?;
        let message = ServerChatMessage::by_uuid(&state.database, uuid)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the message could not be read back"))?;
        let author = match &message.user {
            Some(author) => Some(
                author
                    .clone()
                    .into_api_object(&state, &state.storage.retrieve_urls().await?)
                    .await?,
            ),
            None => None,
        };

        ApiResponse::new_serialized(Response {
            message: ApiServerChatMessage::from_message(message, author),
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
