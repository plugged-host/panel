use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod cancel;
mod unlock;

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger, ServerInstallOptions},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        truncate_directory: bool,
        #[serde(default)]
        start_on_completion: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
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
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.install")?;
        if data.start_on_completion {
            permissions.has_server_permission("control.start")?;
        }
        // plugged.host: a start after the install would skip the start gates (start_gates.rs)
        let start_on_completion = data.start_on_completion && permissions.is_admin();

        tokio::spawn(async move {
            server
                .install_with_options(
                    &state,
                    ServerInstallOptions {
                        truncate_directory: data.truncate_directory,
                        start_on_completion,
                        ..Default::default()
                    },
                )
                .await?;

            activity_logger
                .log(
                    "server:settings.install",
                    serde_json::json!({
                        "truncate_directory": data.truncate_directory,
                        "start_on_completion": start_on_completion,
                    }),
                )
                .await;

            ApiResponse::new_serialized(Response {})
                .with_status(StatusCode::ACCEPTED)
                .ok()
        })
        .await?
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .nest("/cancel", cancel::router(state))
        .nest("/unlock", unlock::router(state))
        .with_state(state.clone())
}
