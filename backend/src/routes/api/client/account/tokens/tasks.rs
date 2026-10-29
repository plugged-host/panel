// plugged.host: claiming a first-week task (shared/src/starter_tasks.rs,
// docs/features/economy.md#getting-started).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        economy::EconomyError,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
        starter_tasks::StarterTask,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        task: StarterTask,
    }

    #[derive(ToSchema, Serialize)]
    struct Claimed {
        task: StarterTask,
        /// Tokens paid.
        amount: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        claim: Claimed,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("economy.earn")?;

        let amount = match shared::starter_tasks::claim(&state, &user, data.task).await {
            Ok(amount) => amount,
            Err(EconomyError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        };

        activity_logger
            .log(
                "account:tokens.task",
                serde_json::json!({ "task": data.task, "amount": amount }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            claim: Claimed {
                task: data.task,
                amount,
            },
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
