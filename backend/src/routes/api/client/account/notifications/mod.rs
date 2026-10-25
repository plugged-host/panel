//! plugged.host: the signed-in person's notification inbox (`shared::notifications`,
//! docs/features/notifications.md). Their own notifications only, so no permission.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _notification_;
mod read;
mod unread;

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject, Pagination, PaginationParams, user::GetUser,
            user_notification::UserNotification,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        notifications: Pagination<shared::models::user_notification::ApiUserNotification>,
        /// How many of all the person's notifications are unread.
        unread: i64,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        (
            "page" = i64, Query,
            description = "The page number",
            example = "1",
        ),
        (
            "per_page" = i64, Query,
            description = "The number of items per page",
            example = "10",
        ),
    ))]
    pub async fn route(
        state: GetState,
        user: GetUser,
        Query(params): Query<PaginationParams>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&params) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let notifications = UserNotification::by_user_uuid_with_pagination(
            &state.database,
            user.uuid,
            params.page,
            params.per_page,
        )
        .await?;
        let unread =
            UserNotification::count_unread_by_user_uuid(&state.database, user.uuid).await?;

        ApiResponse::new_serialized(Response {
            notifications: notifications
                .try_async_map(|notification| notification.into_api_object(&state, ()))
                .await?,
            unread,
        })
        .ok()
    }
}

mod delete {
    use serde::Serialize;
    use shared::{
        GetState,
        models::{user::GetUser, user_notification::UserNotification},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        deleted: u64,
    }

    /// Deletes every notification of the signed-in person.
    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        let deleted = UserNotification::delete_by_user_uuid(&state.database, user.uuid).await?;

        ApiResponse::new_serialized(Response { deleted }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, delete::route))
        .nest("/unread", unread::router(state))
        .nest("/read", read::router(state))
        .nest("/{notification}", _notification_::router(state))
        .with_state(state.clone())
}
