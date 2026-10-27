// plugged.host: a startup command of the user's own, applied once the panel's rules (and neo, when it is offered) let it through (shared/src/startup_command.rs, docs/features/startup-commands.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel,
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        startup_command::{Refusal, Verdict},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// The whole command, `{{NAME}}` for a variable. Longer than 4096 characters is refused
        /// with `too_long`.
        #[garde(length(max = 65536))]
        #[schema(max_length = 65536)]
        command: compact_str::CompactString,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Whether it is now the server's startup command.
        applied: bool,
        /// The startup command it now has: the one sent, or neo's tuned version of it (a Minecraft
        /// server's is always tuned: a smooth game on the least memory and CPU).
        command: Option<String>,
        /// Why it was not applied.
        refusal: Option<Refusal>,
        /// neo's verdict, when neo was asked.
        verdict: Option<Verdict>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
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
        user: GetUser,
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("startup.custom-command")?;

        // each check may cost a neo turn
        state
            .cache
            .ratelimit(
                "client/servers/startup/custom-command",
                20,
                300,
                user.uuid.to_string(),
            )
            .await?;

        let outcome = shared::startup_command::check(&state, &user, &server, &data.command).await?;
        let summary = outcome
            .verdict
            .as_ref()
            .map(|verdict| verdict.summary.clone());

        if let Some(refusal) = &outcome.refusal {
            let problems = outcome
                .verdict
                .as_ref()
                .map(|verdict| verdict.problems.clone());

            activity_logger
                .log(
                    "server:startup.command-refused",
                    serde_json::json!({
                        "command": shared::startup_command::for_log(&data.command),
                        "reason": refusal.code,
                        "detail": refusal.detail,
                        "checked_by": outcome.checked_by,
                        "problems": problems,
                        "summary": summary,
                    }),
                )
                .await;

            return ApiResponse::new_serialized(Response {
                applied: false,
                command: None,
                refusal: outcome.refusal,
                verdict: outcome.verdict,
            })
            .ok();
        }

        server
            .update(
                &state,
                shared::models::server::UpdateServerOptions {
                    startup: Some(match &outcome.command {
                        Some(tuned) => tuned.as_str().into(),
                        None => data.command,
                    }),
                    ..Default::default()
                },
            )
            .await?;

        activity_logger
            .log(
                "server:startup.command",
                serde_json::json!({
                    "command": server.startup,
                    "checked_by": outcome.checked_by,
                    "summary": summary,
                    "tuned_by_neo": outcome.command.is_some(),
                }),
            )
            .await;

        let applied = server.startup.to_string();
        server.0.batch_sync(&state).await;

        ApiResponse::new_serialized(Response {
            applied: true,
            command: Some(applied),
            refusal: None,
            verdict: outcome.verdict,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
