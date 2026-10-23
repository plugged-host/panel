//! plugged.host: what each location can still take, and the moves of servers under way
//! (shared/src/server_transfers.rs, docs/features/transfers.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
        server_transfers::{ApiAdminTransferRequest, ApiLocationCapacity},
    };
    use utoipa::ToSchema;

    /// Moves listed: those under way first, then the newest finished ones.
    const MOVES: i64 = 50;

    #[derive(ToSchema, Serialize)]
    struct Response {
        locations: Vec<ApiLocationCapacity>,
        transfers: Vec<ApiAdminTransferRequest>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        ApiResponse::new_serialized(Response {
            locations: shared::server_transfers::capacity(&state).await?,
            transfers: shared::server_transfers::recent(&state, MOVES).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
