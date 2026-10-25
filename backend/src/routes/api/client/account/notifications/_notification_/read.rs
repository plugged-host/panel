// plugged.host: marking one notification read (docs/features/notifications.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{user::GetUser, user_notification::UserNotification},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "notification" = uuid::Uuid,
            description = "The notification ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        user: GetUser,
        Path(notification): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        if UserNotification::by_user_uuid_uuid(&state.database, user.uuid, notification)
            .await?
            .is_none()
        {
            return ApiResponse::error("notification not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        UserNotification::mark_read(&state.database, user.uuid, Some(notification)).await?;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
