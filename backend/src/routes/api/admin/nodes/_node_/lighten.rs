//! plugged.host: "Lighten load": move some of a node's servers to other nodes of its location
//! (shared/src/server_transfers.rs, docs/features/transfers.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            node::GetNode,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_transfers::LightenPick,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        pick: LightenPick,
        #[garde(range(min = 1, max = 100))]
        #[schema(minimum = 1, maximum = 100)]
        count: u32,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        queued: usize,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        node: GetNode,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("nodes.transfers")?;

        let queued =
            shared::server_transfers::lighten(&state, &node, &user, data.pick, data.count as usize)
                .await?;

        activity_logger
            .log(
                "node:lighten",
                serde_json::json!({
                    "uuid": node.uuid,
                    "pick": data.pick,
                    "count": data.count,
                    "queued": queued,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { queued }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
