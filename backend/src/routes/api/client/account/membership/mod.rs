// plugged.host: the memberships, bought with tokens or credits (docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod plus;
mod plusplus;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        economy::ApiEconomy,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        economy: ApiEconomy,
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
            economy: shared::economy::summary(&state, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/plus", plus::router(state))
        .nest("/plusplus", plusplus::router(state))
        .with_state(state.clone())
}
