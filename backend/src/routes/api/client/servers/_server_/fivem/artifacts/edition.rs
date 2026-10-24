// plugged.host: switching a FiveM server between FiveM and FiveM for GTAV Enhanced
// (shared/src/fivem/runtime.rs, docs/features/fivem.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod put {
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        fivem::runtime::{Edition, Outcome},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        edition: Edition,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        outcome: Outcome,
    }

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
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
        permissions.has_server_permission("fivem.artifacts")?;
        shared::fivem::require(&server)?;
        let may_restart = permissions.has_server_permission("control.restart").is_ok();

        let mut server = server.0;
        let outcome = match shared::fivem::runtime::switch_edition(
            &state,
            &mut server,
            data.edition,
            may_restart,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "server:fivem.edition",
                serde_json::json!({ "edition": data.edition, "outcome": outcome }),
            )
            .await;

        ApiResponse::new_serialized(Response { outcome }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(put::route))
        .with_state(state.clone())
}
