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
            suggestions: shared::suggestions::list(&state.database, user.uuid).await?,
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

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .routes(routes!(vote::route))
        .with_state(state.clone())
}
