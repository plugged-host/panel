// plugged.host: turning the whitelist on or off (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod put {
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::players::Outcome,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        enabled: bool,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = Outcome),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
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
        permissions.has_server_permission("minecraft.players")?;
        shared::minecraft::require(&server)?;

        let console = permissions.has_server_permission("control.console").is_ok();

        match shared::minecraft::players::enforce(&state, &server, data.enabled, console).await {
            Ok(outcome) => {
                activity_logger
                    .log(
                        "server:minecraft.players.enforce",
                        serde_json::json!({
                            "enabled": data.enabled,
                            "live": outcome.live,
                        }),
                    )
                    .await;

                ApiResponse::new_serialized(outcome).ok()
            }
            Err(err) => err.response().ok(),
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(put::route))
        .with_state(state.clone())
}
