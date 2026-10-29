// plugged.host: the membership popup (shared/src/promos.rs, docs/features/promos.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        promos::ApiPromo,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// The popup to show now; `null` when it is not due (or not for this account).
        promo: Option<ApiPromo>,
    }

    /// Claims the popup: when it is due it is marked shown and returned, once.
    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.read")?;

        ApiResponse::new_serialized(Response {
            promo: shared::promos::claim(&state, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
