// plugged.host: tokens and earning them through sponsored links (docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod earn;
mod redeem;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        earn::ApiTokens,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        tokens: ApiTokens,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.read")?;

        ApiResponse::new_serialized(Response {
            tokens: shared::earn::tokens(&state, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/earn", earn::router(state))
        .nest("/redeem", redeem::router(state))
        .with_state(state.clone())
}
