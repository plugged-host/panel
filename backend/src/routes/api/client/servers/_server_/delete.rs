// plugged.host: owners deleting their own servers, when Admin › Settings › Games allows it
// (docs/features/server-settings.md).

pub mod delete {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            DeletableModel,
            server::GetServer,
            server_backup::ServerBackup,
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
        activity_logger: GetUserActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.delete")?;

        if !permissions.is_admin() && !state.settings.get().await?.games.allow_deleting {
            return ApiResponse::error("deleting servers is turned off")
                .with_status(StatusCode::FORBIDDEN)
                .ok();
        }

        // the backups go with the server: nobody could reach them afterwards
        let backups = ServerBackup::all_by_server_uuid(&state.database, server.uuid).await?;

        if let Err(err) = server
            .delete(
                &state,
                shared::models::server::DeleteServerOptions { force: false },
            )
            .await
        {
            tracing::error!(server = %server.uuid, "failed to delete server: {:?}", err);

            let (err, status) = shared::response::extract_readable_message(&err)
                .unwrap_or_else(|| (err.to_string(), StatusCode::EXPECTATION_FAILED));

            return ApiResponse::error(format!("failed to delete server: {err}"))
                .with_status(status)
                .ok();
        }

        for backup in backups {
            let backup_uuid = backup.uuid;

            if let Err(err) = backup.delete(&state, Default::default()).await {
                tracing::error!(server = %server.uuid, backup = %backup_uuid, "failed to delete backup: {:?}", err);
            }
        }

        activity_logger
            .log(
                "server:delete",
                serde_json::json!({
                    "uuid": server.uuid,
                    "name": server.name,
                    "owner_uuid": server.owner.uuid,
                    "by_owner": server.owner.uuid == user.uuid,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}
