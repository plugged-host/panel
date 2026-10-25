// plugged.host: "Mark all as read" in the notification inbox (docs/features/notifications.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use serde::Serialize;
    use shared::{
        GetState,
        models::{user::GetUser, user_notification::UserNotification},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        updated: u64,
    }

    /// Marks every unread notification of the signed-in person read.
    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        let updated = UserNotification::mark_read(&state.database, user.uuid, None).await?;

        ApiResponse::new_serialized(Response { updated }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
