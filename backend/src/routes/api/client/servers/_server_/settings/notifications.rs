// plugged.host: the person's backup mails for this server (shared/src/backup_notifications.rs).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        backup_notifications::Preferences,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        notifications: Preferences,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
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
    ) -> ApiResponseResult {
        permissions.has_server_permission("backups.read")?;

        ApiResponse::new_serialized(Response {
            notifications: shared::backup_notifications::get(
                &state.database,
                server.uuid,
                user.uuid,
            )
            .await?,
        })
        .ok()
    }
}

mod put {
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        backup_notifications::Preferences,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        manual_backups: bool,
        #[garde(skip)]
        scheduled_backups: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        notifications: Preferences,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("backups.read")?;

        let notifications = Preferences {
            manual_backups: data.manual_backups,
            scheduled_backups: data.scheduled_backups,
        };
        shared::backup_notifications::set(&state.database, server.uuid, user.uuid, notifications)
            .await?;

        activity_logger
            .log(
                "server:settings.notifications",
                serde_json::json!({
                    "manual_backups": notifications.manual_backups,
                    "scheduled_backups": notifications.scheduled_backups,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { notifications }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
