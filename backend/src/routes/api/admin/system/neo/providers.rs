// plugged.host: where each of neo's model slots stands: cooldown, tokens today, last error.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        neo::chain::{Chain, LastError},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Slot {
        slot: u8,
        /// On, with an endpoint, a key and a model.
        usable: bool,
        /// Unix seconds until which the slot is skipped after a failure.
        cooling_down_until: Option<i64>,
        /// Input and output tokens it used today (UTC).
        tokens_today: i64,
        /// Its last failure in the past day.
        last_error: Option<LastError>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        providers: Vec<Slot>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("settings.read")?;

        let neo = state.settings.get_as(|s| s.neo.clone()).await?;
        let chain = Chain {
            scope: "neo",
            probe_tools: true,
        };

        let mut providers = Vec::with_capacity(neo.providers.len());
        for provider in &neo.providers {
            let status = chain.status(&state, provider).await;
            providers.push(Slot {
                slot: status.slot,
                usable: provider.usable(),
                cooling_down_until: status.cooling_down_until,
                tokens_today: status.tokens_today,
                last_error: status.last_error.map(|error| LastError {
                    reason: super::super::excerpt(&error.reason, provider),
                    ..error
                }),
            });
        }

        ApiResponse::new_serialized(Response { providers }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
