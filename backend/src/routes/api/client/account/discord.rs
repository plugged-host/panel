//! plugged.host: the account's Discord rewards page (docs/features/discord.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        discord::rewards::ApiDiscordRewards,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        discord: ApiDiscordRewards,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("discord.read")?;

        if !state.settings.get().await?.discord.active() {
            return ApiResponse::error("Discord is not set up on this panel")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        ApiResponse::new_serialized(Response {
            discord: shared::discord::rewards::page(&state, user.uuid).await?,
        })
        .ok()
    }
}

mod check {
    pub mod post {
        use axum::http::StatusCode;
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            discord::rewards::ApiDiscordRewards,
            models::{
                user::{GetPermissionManager, GetUser},
                user_activity::GetUserActivityLogger,
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            discord: ApiDiscordRewards,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = FORBIDDEN, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetUserActivityLogger,
        ) -> ApiResponseResult {
            permissions.has_user_permission("discord.check")?;

            if !state.settings.get().await?.discord.active() {
                return ApiResponse::error("Discord is not set up on this panel")
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            }
            // once a minute: every check asks Discord
            state
                .cache
                .ratelimit("discord/check", 2, 60, user.uuid.to_string())
                .await?;

            shared::discord::rewards::check(&state, user.uuid).await?;
            activity_logger
                .log("account:discord.check", serde_json::json!({}))
                .await;

            ApiResponse::new_serialized(Response {
                discord: shared::discord::rewards::page(&state, user.uuid).await?,
            })
            .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest(
            "/check",
            OpenApiRouter::new()
                .routes(routes!(check::post::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
