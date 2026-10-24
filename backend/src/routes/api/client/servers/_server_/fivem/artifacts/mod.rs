// plugged.host: the FXServer build a FiveM server runs (shared/src/fivem/runtime.rs,
// docs/features/fivem.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod edition;

mod get {
    use shared::{
        ApiError, GetState,
        fivem::runtime::Overview,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Overview),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("fivem.artifacts")?;
        shared::fivem::require(&server)?;

        ApiResponse::new_serialized(shared::fivem::runtime::overview(&state, &server).await?).ok()
    }
}

mod put {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        fivem::runtime::{Choice, Outcome},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        version: compact_str::CompactString,
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
    ), request_body = inline(Choice))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Choice>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("fivem.artifacts")?;
        shared::fivem::require(&server)?;
        let may_restart = permissions.has_server_permission("control.restart").is_ok();

        let mut server = server.0;
        let (version, outcome) =
            match shared::fivem::runtime::choose(&state, &mut server, &data, may_restart).await {
                Ok(chosen) => chosen,
                Err(err) => return err.response().ok(),
            };

        activity_logger
            .log(
                "server:fivem.version",
                serde_json::json!({ "version": version, "outcome": outcome }),
            )
            .await;

        ApiResponse::new_serialized(Response { version, outcome }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, put::route))
        .nest("/edition", edition::router(state))
        .with_state(state.clone())
}
