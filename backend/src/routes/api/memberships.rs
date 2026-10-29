// plugged.host: what the membership bundle's other panel tells this one, server to server: when its accounts' second membership ends (shared/src/membership_bundle.rs, docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod bundle {
    pub mod post {
        use axum::http::{HeaderMap, StatusCode};
        use serde::Serialize;
        use shared::{
            ApiError, GetIp, GetState,
            membership_bundle::Received,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// Accounts whose user here got the date.
            applied: usize,
            /// Accounts with no user here yet: kept until one is linked.
            pending: usize,
        }

        fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
        }

        /// `{"memberships": [{"account_id", "premium_until"}]}`: until when each noprofit.space
        /// account's second membership on the other panel runs, which includes Plus here.
        /// Unauthenticated: `X-Signature` is `hex(hmac_sha256(shared secret, "<X-Timestamp>.<body>"))`
        /// and `X-Timestamp` at most 300 seconds off.
        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = NOT_FOUND, body = ApiError),
            (status = UNPROCESSABLE_ENTITY, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            headers: HeaderMap,
            body: String,
        ) -> ApiResponseResult {
            state
                .cache
                .ratelimit("memberships/bundle", 60, 60, ip.0.to_string())
                .await?;

            match shared::membership_bundle::receive(
                &state,
                header(&headers, "x-timestamp"),
                header(&headers, "x-signature"),
                body.as_bytes(),
            )
            .await?
            {
                Received::Off => ApiResponse::error("not found")
                    .with_status(StatusCode::NOT_FOUND)
                    .ok(),
                Received::Unauthorized => ApiResponse::error("unauthorized")
                    .with_status(StatusCode::UNAUTHORIZED)
                    .ok(),
                Received::Invalid => ApiResponse::error("invalid payload")
                    .with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .ok(),
                Received::Done { applied, pending } => {
                    tracing::info!(
                        applied,
                        pending,
                        "membership bundle: second memberships received"
                    );

                    ApiResponse::new_serialized(Response { applied, pending }).ok()
                }
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/bundle",
            OpenApiRouter::new()
                .routes(routes!(bundle::post::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
