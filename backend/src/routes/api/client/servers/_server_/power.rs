use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        #[serde(alias = "signal")]
        action: wings_api::ServerPowerAction,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// plugged.host: the start gate that holds this start (202), see shared/src/start_gates.rs
        #[serde(skip_serializing_if = "Option::is_none")]
        gate: Option<&'static str>,
        /// plugged.host: where the user goes instead (202)
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = ACCEPTED, body = inline(Response)),
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
        permissions.has_server_permission(match data.action {
            wings_api::ServerPowerAction::Start => "control.start",
            wings_api::ServerPowerAction::Stop => "control.stop",
            wings_api::ServerPowerAction::Kill => "control.stop",
            wings_api::ServerPowerAction::Restart => "control.restart",
        })?;

        // plugged.host: a start gate may hold the start and send the user to its page instead
        if let Some(held) = shared::start_gates::run(&state, &server.0, &user.0, data.action).await
        {
            return ApiResponse::new_serialized(Response {
                gate: Some(held.gate),
                url: Some(held.url),
            })
            .with_status(StatusCode::ACCEPTED)
            .ok();
        }

        tokio::spawn(async move {
            server
                .node
                .fetch_cached(&state.database)
                .await?
                .api_client(&state.database)
                .await?
                .post_servers_server_power(
                    server.uuid,
                    &wings_api::servers_server_power::post::RequestBody {
                        action: data.action,
                        wait_seconds: None,
                    },
                )
                .await?;

            activity_logger
                .log(
                    "server:power.action",
                    serde_json::json!({
                        "action": data.action
                    }),
                )
                .await;

            ApiResponse::new_serialized(Response {
                gate: None,
                url: None,
            })
            .ok()
        })
        .await?
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
