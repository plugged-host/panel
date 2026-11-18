// plugged.host: a paid server's month, paid with noprofit.space Funds, and its renewal
// (shared/src/dedicated.rs, docs/features/dedicated.md). Reachable while it is suspended for an
// unpaid month, so its owner can pay.
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::server::GetServer,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// `None` for a server nobody pays for here.
        subscription: Option<shared::dedicated::ApiSubscription>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(state: GetState, server: GetServer) -> ApiResponseResult {
        ApiResponse::new_serialized(Response {
            subscription: shared::dedicated::subscription(&state, server.uuid).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        dedicated::Renewal,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetUser,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// Pay the next month from the owner's Funds when this one ends. Turned on after the month
        /// ended, it pays now and lifts the unpaid suspension.
        #[garde(skip)]
        renews: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        subscription: Option<shared::dedicated::ApiSubscription>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = PAYMENT_REQUIRED, body = ApiError),
        (status = SERVICE_UNAVAILABLE, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        user: GetUser,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        // it is the owner's money
        if server.owner.uuid != user.uuid {
            return ApiResponse::error("only the server's owner can change what it is paid with")
                .with_status(StatusCode::FORBIDDEN)
                .ok();
        }
        if shared::dedicated::subscription(&state, server.uuid)
            .await?
            .is_none()
        {
            return ApiResponse::error("this server is not paid for here")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        let renewal = shared::dedicated::set_renews(&state, server.uuid, data.renews).await?;
        activity_logger
            .log(
                "server:billing.renews",
                serde_json::json!({ "renews": data.renews, "paid": renewal == Some(Renewal::Paid) }),
            )
            .await;

        match renewal {
            Some(Renewal::Short) => {
                return ApiResponse::error("not enough Funds: add Funds on noprofit.space")
                    .with_status(StatusCode::PAYMENT_REQUIRED)
                    .ok();
            }
            Some(Renewal::Unavailable) => {
                return ApiResponse::error("noprofit.space did not answer, try again in a moment")
                    .with_status(StatusCode::SERVICE_UNAVAILABLE)
                    .ok();
            }
            _ => {}
        }

        ApiResponse::new_serialized(Response {
            subscription: shared::dedicated::subscription(&state, server.uuid).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .with_state(state.clone())
}
