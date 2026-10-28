// plugged.host: what a survey wall tells the panel, server to server: CPX Research's postback (shared/src/earn.rs, docs/features/economy.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod cpx_research {
    pub mod get {
        use axum::{body::Body, extract::Query, http::StatusCode};
        use shared::{
            ApiError, GetIp, GetState,
            earn::{CpxPostback, PostbackOutcome},
            models::user_activity::UserActivityLogger,
            response::{ApiResponse, ApiResponseResult},
        };
        use std::collections::HashMap;

        /// Pays a survey CPX Research confirmed (`status=1`) or takes one back (`status=2`).
        /// Unauthenticated: the hash of the transaction id with the app's secure hash proves it
        /// came from CPX. Anything it signed is answered `1`, also when there was nothing to do,
        /// so CPX does not send it again.
        #[utoipa::path(get, path = "/", responses(
            (status = OK, description = "Handled (`1`)", body = String, content_type = "text/plain"),
            (status = FORBIDDEN, description = "CPX Research is off or not set up, or the hash does not match", body = ApiError),
        ), params(
            ("status" = Option<String>, Query, description = "1: credited, 2: reversed"),
            ("trans_id" = Option<String>, Query, description = "CPX Research's transaction id"),
            ("user_id" = Option<String>, Query, description = "The user's uuid (the wall's ext_user_id)"),
            ("amount_local" = Option<String>, Query, description = "What it pays, in tokens"),
            ("type" = Option<String>, Query, description = "complete, out (a screen-out) or bonus"),
            ("ip_click" = Option<String>, Query, description = "The user's address when they took the survey"),
            ("hash" = Option<String>, Query, description = "md5 of `<trans_id>-<the app's secure hash>`"),
        ))]
        pub async fn route(
            state: GetState,
            ip: GetIp,
            Query(params): Query<HashMap<String, String>>,
        ) -> ApiResponseResult {
            let postback = CpxPostback::from_params(&params);
            let outcome = shared::earn::survey_postback(&state, &postback).await?;

            let trans_id = postback.trans_id.as_str();
            match &outcome {
                PostbackOutcome::Refused => {
                    return ApiResponse::error("postback refused")
                        .with_status(StatusCode::FORBIDDEN)
                        .ok();
                }
                PostbackOutcome::Granted {
                    user_uuid,
                    amount,
                    event,
                } => {
                    tracing::info!(
                        user = %user_uuid,
                        trans_id,
                        "cpx research paid {amount} tokens ({})",
                        event.key()
                    );

                    // in the account's activity like a link, at the address the survey was taken
                    // from
                    UserActivityLogger {
                        state: state.0.clone(),
                        user_uuid: *user_uuid,
                        impersonator_uuid: None,
                        api_key_uuid: None,
                        ip: postback.ip_click.parse().unwrap_or(ip.0),
                    }
                    .log(
                        "account:tokens.earn",
                        serde_json::json!({
                            "provider": shared::earn::Provider::CpxResearch,
                            "amount": amount,
                            "type": event.key(),
                        }),
                    )
                    .await;
                }
                PostbackOutcome::Capped { user_uuid } => {
                    tracing::info!(
                        user = %user_uuid,
                        trans_id,
                        "cpx research survey over the daily cap, not paid"
                    );
                }
                PostbackOutcome::NoAmount { user_uuid } => {
                    tracing::info!(
                        user = %user_uuid,
                        trans_id,
                        amount_local = %postback.amount_local,
                        "cpx research survey pays nothing"
                    );
                }
                PostbackOutcome::Reversed { user_uuid, amount } => {
                    tracing::info!(
                        user = %user_uuid,
                        trans_id,
                        "cpx research reversed a survey, took back {amount} tokens"
                    );
                }
                PostbackOutcome::UnknownUser => {
                    tracing::warn!(
                        trans_id,
                        user_id = %postback.user_id,
                        "cpx research paid a survey for no known user"
                    );
                }
                PostbackOutcome::Duplicate
                | PostbackOutcome::Ignored
                | PostbackOutcome::NothingToReverse => {
                    tracing::debug!(trans_id, "cpx research postback: {outcome:?}");
                }
            }

            ApiResponse::new(Body::from("1"))
                .with_header("Content-Type", "text/plain; charset=utf-8")
                .with_header("Cache-Control", "no-store")
                .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/cpx-research",
            OpenApiRouter::new()
                .routes(routes!(cpx_research::get::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
