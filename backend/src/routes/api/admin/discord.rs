//! plugged.host: the Discord bots' state for Admin › Settings › Accounts › Discord
//! (shared/src/discord/gateway.rs, docs/features/discord.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod status {
    pub mod get {
        use serde::Serialize;
        use shared::{
            GetState,
            discord::gateway::BotStatus,
            models::user::GetPermissionManager,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// The connected bots as the primary instance last stored them; `None` when it did
            /// not (no bot on, or no primary instance running).
            bots: Option<Vec<BotStatus>>,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("settings.read")?;

            ApiResponse::new_serialized(Response {
                bots: shared::discord::gateway::status(&state).await,
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
                .routes(routes!(status::get::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
