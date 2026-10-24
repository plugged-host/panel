// plugged.host: one call to one of neo's model slots as saved (the Test of Admin › Settings › AI › neo).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// A chat any model answers in a word.
fn messages() -> [serde_json::Value; 2] {
    [
        serde_json::json!({
            "role": "system",
            "content": "This is a connectivity check. Answer with the single word OK.",
        }),
        serde_json::json!({"role": "user", "content": "Say OK."}),
    ]
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
        neo::{chain::Chain, usage},
        response::{ApiResponse, ApiResponseResult},
        settings::neo::{MAX_PROVIDERS, endpoint_allowed},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(range(min = 1, max = MAX_PROVIDERS))]
        #[schema(minimum = 1, maximum = 6)]
        slot: u8,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        successful: bool,
        duration_ms: i64,
        /// The model the slot asks for.
        model: compact_str::CompactString,
        /// The start of the answer.
        answer: Option<String>,
        /// Input and output tokens the call used.
        tokens: i64,
        /// The provider's HTTP status when it refused.
        status: Option<u16>,
        /// Why the call failed, keys masked.
        error: Option<String>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("settings.update")?;

        let neo = state.settings.get_as(|s| s.neo.clone()).await?;
        let Some(mut provider) = neo
            .providers
            .iter()
            .find(|provider| provider.slot == data.slot)
            .cloned()
        else {
            return ApiResponse::error("no model is saved in this slot")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };
        // a slot that is off can be tried before it is switched on
        provider.enabled = true;
        if !provider.usable() {
            return ApiResponse::error("the model needs an endpoint, an api key and a model name")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        if !endpoint_allowed(&provider.endpoint) {
            return ApiResponse::error("the endpoint must be a public https url")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let (outcome, duration_ms) = Chain {
            scope: "neo",
            probe_tools: false,
        }
        .probe(&state, &neo, &provider, &super::messages())
        .await;

        let response = match outcome {
            Ok(answer) => {
                let event = usage::Event {
                    service: "settings".into(),
                    provider_slot: Some(provider.slot as i16),
                    provider_label: Some(provider.label.to_string()),
                    driver: Some(provider.driver.as_str().into()),
                    model: Some(provider.model.to_string()),
                    prompt_tokens: answer.prompt_tokens,
                    completion_tokens: answer.completion_tokens,
                    finish_reason: Some(answer.finish_reason.into()),
                    duration_ms,
                    ..Default::default()
                };
                if let Err(err) = usage::record(&state.database, None, "test", &event).await {
                    tracing::warn!("failed to record a neo provider test: {err:#}");
                }

                Response {
                    successful: true,
                    duration_ms,
                    model: provider.model.clone(),
                    answer: Some(super::super::excerpt(
                        answer.content.as_deref().unwrap_or_default(),
                        &provider,
                    )),
                    tokens: answer.total_tokens(),
                    status: None,
                    error: None,
                }
            }
            Err(failure) => Response {
                successful: false,
                duration_ms,
                model: provider.model.clone(),
                answer: None,
                tokens: 0,
                status: failure.status,
                error: Some(super::super::excerpt(&failure.reason, &provider)),
            },
        };

        activity_logger
            .log(
                "neo:provider-test",
                serde_json::json!({
                    "slot": provider.slot,
                    "model": provider.model,
                    "successful": response.successful,
                    "status": response.status,
                }),
            )
            .await;

        ApiResponse::new_serialized(response).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_chat_has_a_system_and_a_user_message() {
        let messages = messages();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
    }
}
