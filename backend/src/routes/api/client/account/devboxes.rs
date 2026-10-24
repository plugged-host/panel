//! plugged.host: back to a devbox in the browser (docs/features/devboxes.md). The edge sends a
//! browser without a valid cookie to the panel's `/devbox/open?host=…`, which asks here after the
//! panel's own sign-in: a fresh one-time link, or the game server whose VS Code page explains why not.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod open {
    pub mod post {
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            devboxes::web::Reopen,
            models::user::GetUser,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// The devbox host the browser came from.
            #[garde(length(chars, max = 255))]
            host: String,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// A one-time link that signs the browser in on the devbox.
            url: Option<String>,
            /// The game server to show instead: the devbox is stopped or not theirs.
            server: Option<uuid::Uuid>,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ), request_body = inline(Payload))]
        pub async fn route(
            state: GetState,
            ip: shared::GetIp,
            user: GetUser,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(axum::http::StatusCode::BAD_REQUEST)
                    .ok();
            }
            state
                .cache
                .ratelimit(
                    format!("devboxes/open/{}", user.uuid),
                    20,
                    60,
                    ip.to_string(),
                )
                .await?;

            let (url, server) =
                match shared::devboxes::web::reopen(&state, user.uuid, &data.host).await? {
                    Reopen::Url(url) => (Some(url), None),
                    Reopen::Server(server) => (None, Some(server)),
                    Reopen::Panel => (None, None),
                };

            ApiResponse::new_serialized(Response { url, server }).ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/open",
            OpenApiRouter::new()
                .routes(routes!(open::post::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
