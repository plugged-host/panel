// plugged.host: the account's Backups page: the newest copy of each server, deleted ones included
// (docs/features/backups-page.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _backup_;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        deleted_servers::ApiAccountBackup,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        backups: Vec<ApiAccountBackup>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("backups.read")?;

        ApiResponse::new_serialized(Response {
            backups: shared::deleted_servers::for_owner(&state.database, user.uuid).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest("/{backup}", _backup_::router(state))
        .with_state(state.clone())
}
