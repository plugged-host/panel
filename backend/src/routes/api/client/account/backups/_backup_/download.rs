// plugged.host: downloading a copy from the account's Backups page, by the server's owner only
// (docs/features/backups-page.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Path, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetAuthMethod, GetPermissionManager, GetUser},
        models::user_activity::GetUserActivityLogger,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(format = "uri")]
        url: String,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
    ), params(
        (
            "backup" = uuid::Uuid,
            description = "The backup ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    #[allow(clippy::too_many_arguments)]
    pub async fn route(
        state: GetState,
        request_host: shared::GetRequestHost,
        permissions: GetPermissionManager,
        auth: GetAuthMethod,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        Path(backup): Path<uuid::Uuid>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("backups.download")?;

        let Some(backup) =
            shared::deleted_servers::owned_backup(&state.database, user.uuid, backup).await?
        else {
            return ApiResponse::error("backup not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };

        // the node that took it: a deleted server has no node of its own any more
        let node = backup.node.fetch_cached(&state.database).await?;

        let url = backup
            .download_url(
                &state,
                request_host.as_deref(),
                &user,
                auth.api_key_uuid(),
                &node,
                wings_api::StreamableArchiveFormat::default(),
            )
            .await?;

        activity_logger
            .log(
                "account:backup.download",
                serde_json::json!({
                    "uuid": backup.uuid,
                    "name": backup.name,
                    "server_uuid": backup.server.as_ref().map(|server| server.uuid),
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { url }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
