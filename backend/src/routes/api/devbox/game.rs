// plugged.host: what `ph` may do with the devbox's game server: its status, power (starts through
// the start gates), and a Wings websocket token for the console (docs/features/devboxes.md).
use super::State;
use axum::http::{HeaderMap, StatusCode};
use shared::{
    devboxes::{Context, Devbox},
    models::{ByUuid, server::Server},
    response::ApiResponse,
};
use utoipa_axum::{router::OpenApiRouter, routes};

fn forbidden(message: &str) -> ApiResponse {
    ApiResponse::error(message).with_status(StatusCode::FORBIDDEN)
}

/// The devbox, its game server and the settings, while anyone may still use the devbox.
async fn game_of(
    state: &shared::State,
    headers: &HeaderMap,
    action: &str,
    per_minute: u64,
) -> Result<(Context, Devbox, Server), ApiResponse> {
    let (context, devbox) =
        super::devbox_of(state, headers, &format!("game-{action}"), per_minute).await?;
    let Some(game) = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?
    else {
        return Err(forbidden("not allowed"));
    };
    if shared::devboxes::people(state, &context, &devbox, &game)
        .await?
        .is_empty()
    {
        return Err(forbidden("not allowed"));
    }

    Ok((context, devbox, game))
}

mod status {
    pub mod post {
        use axum::http::HeaderMap;
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            devboxes::agent::GameAction,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            name: String,
            id: String,
            state: String,
            memory_bytes: Option<u64>,
            memory_limit_mib: i64,
            cpu_percent: Option<f64>,
            disk_bytes: Option<u64>,
            disk_limit_mib: i64,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
        ))]
        pub async fn route(state: GetState, headers: HeaderMap) -> ApiResponseResult {
            let (_, devbox, game) = super::super::game_of(&state, &headers, "status", 30).await?;
            if !shared::devboxes::agent::may(&state, &devbox, &game, GameAction::Status).await? {
                return Err(super::super::forbidden("not allowed"));
            }

            let details = async {
                game.node
                    .fetch_cached(&state.database)
                    .await
                    .ok()?
                    .api_client(&state.database)
                    .await
                    .ok()?
                    .get_servers_server(game.uuid)
                    .await
                    .ok()
            }
            .await;
            let state_name = match &details {
                Some(details) => serde_json::to_value(details.state)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "unknown".to_string()),
                None => "unknown".to_string(),
            };

            ApiResponse::new_serialized(Response {
                name: game.name.to_string(),
                id: format!("{:08x}", game.uuid_short as u32),
                state: state_name,
                memory_bytes: details.as_ref().map(|d| d.utilization.memory_bytes),
                memory_limit_mib: game.memory,
                cpu_percent: details.as_ref().map(|d| d.utilization.cpu_absolute),
                disk_bytes: details.as_ref().map(|d| d.utilization.disk_bytes),
                disk_limit_mib: game.disk,
            })
            .ok()
        }
    }
}

mod power {
    pub mod post {
        use axum::http::{HeaderMap, StatusCode};
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            devboxes::agent::GameAction,
            models::{ByUuid, user::User},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// start, stop, restart or kill
            #[garde(skip)]
            #[serde(default)]
            action: String,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            ok: bool,
            message: String,
            /// Where the start waits (the start queue, the waiting room, a restore).
            #[serde(skip_serializing_if = "Option::is_none")]
            continue_at: Option<String>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
            (status = UNPROCESSABLE_ENTITY, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            let (context, devbox, game) =
                super::super::game_of(&state, &headers, "power", 10).await?;
            let Some(action) = GameAction::parse(&data.action) else {
                return ApiResponse::error("action must be start, stop, restart or kill")
                    .with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .ok();
            };
            if !shared::devboxes::agent::may(&state, &devbox, &game, action).await? {
                return Err(super::super::forbidden(&format!(
                    "not allowed to {} this server",
                    data.action
                )));
            }
            let Some(power) = action.power() else {
                return Err(super::super::forbidden("not allowed"));
            };

            let actor = shared::devboxes::agent::actor(&devbox, &game);
            if let Some(user) = User::by_uuid_optional(&state.database, actor).await?
                && let Some(held) = shared::start_gates::run(&state, &game, &user, power).await
            {
                let url = if held.url.starts_with('/') {
                    format!("{}{}", context.app_url, held.url)
                } else {
                    held.url
                };

                return ApiResponse::new_serialized(Response {
                    ok: false,
                    message: format!(
                        "The {} is waiting in the panel (start queue, waiting room or restore). Continue it there.",
                        data.action
                    ),
                    continue_at: Some(url),
                })
                .ok();
            }

            game.node
                .fetch_cached(&state.database)
                .await?
                .api_client(&state.database)
                .await?
                .post_servers_server_power(
                    game.uuid,
                    &wings_api::servers_server_power::post::RequestBody {
                        action: power,
                        wait_seconds: None,
                    },
                )
                .await?;
            shared::devboxes::log_game(
                &state,
                game.uuid,
                Some(actor),
                "server:devbox.power",
                serde_json::json!({ "action": data.action, "shared": devbox.is_shared() }),
            )
            .await;

            ApiResponse::new_serialized(Response {
                ok: true,
                message: format!("Sent {}.", data.action),
                continue_at: None,
            })
            .ok()
        }
    }
}

mod websocket {
    pub mod post {
        use axum::http::HeaderMap;
        use compact_str::ToCompactString;
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            jwt::BasePayload,
            models::{ByUuid, user::User},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            token: String,
            socket: String,
            origin: String,
            permissions: Vec<&'static str>,
        }

        #[derive(Serialize)]
        struct WebsocketJwt<'a> {
            #[serde(flatten)]
            base: BasePayload,
            user_uuid: uuid::Uuid,
            user_name: &'a str,
            user_avatar: Option<String>,
            server_uuid: uuid::Uuid,
            permissions: &'a [&'static str],
            full_permissions: &'a [&'static str],
            ignored_files: &'a [compact_str::CompactString],
            credential: Option<uuid::Uuid>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
        ))]
        pub async fn route(state: GetState, headers: HeaderMap) -> ApiResponseResult {
            let (context, devbox, game) =
                super::super::game_of(&state, &headers, "websocket", 20).await?;
            let permissions =
                shared::devboxes::agent::socket_permissions(&state, &devbox, &game).await?;
            let actor = shared::devboxes::agent::actor(&devbox, &game);
            let Some(user) = User::by_uuid_optional(&state.database, actor).await? else {
                return Err(super::super::forbidden("not allowed"));
            };
            if permissions.is_empty() {
                return Err(super::super::forbidden(
                    "not allowed to read this server's console",
                ));
            }

            let node = game.node.fetch_cached(&state.database).await?;
            let now = chrono::Utc::now().timestamp();
            let token = node
                .create_jwt(
                    &state.database,
                    &state.jwt,
                    &WebsocketJwt {
                        base: BasePayload {
                            scope: "websocket".into(),
                            issuer: "panel".into(),
                            subject: Some(user.uuid.to_compact_string()),
                            audience: Vec::new(),
                            expiration_time: Some(now + 600),
                            not_before: None,
                            issued_at: Some(now),
                            jwt_id: uuid::Uuid::new_v4().to_compact_string(),
                        },
                        user_uuid: user.uuid,
                        user_name: &user.username,
                        user_avatar: None,
                        server_uuid: game.uuid,
                        permissions: &permissions,
                        full_permissions: &permissions,
                        ignored_files: &[],
                        credential: None,
                    },
                )
                .await?;

            let mut url = node
                .public_url(&state, None, &format!("/api/servers/{}/ws", game.uuid))
                .await?;
            if url.scheme() == "http" {
                url.set_scheme("ws").ok();
            } else if url.scheme() == "https" {
                url.set_scheme("wss").ok();
            }

            ApiResponse::new_serialized(Response {
                token,
                socket: url.to_string(),
                origin: context.app_url.clone(),
                permissions,
            })
            .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/status",
            OpenApiRouter::new()
                .routes(routes!(status::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/power",
            OpenApiRouter::new()
                .routes(routes!(power::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/websocket",
            OpenApiRouter::new()
                .routes(routes!(websocket::post::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
