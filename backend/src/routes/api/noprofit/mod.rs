// plugged.host: what noprofit.space asks the panel when someone deletes their account there, signed with the API secret (shared/src/noprofit.rs, docs/noprofit.md).
use super::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use shared::response::ApiResponse;
use utoipa_axum::{router::OpenApiRouter, routes};

fn unauthorized() -> ApiResponse {
    ApiResponse::error("unauthorized").with_status(StatusCode::UNAUTHORIZED)
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

/// The payload of a request noprofit.space signed (`X-NP-*` headers): refused with a 401 unless
/// the panel's API secret signed this method, path and body less than a minute ago, with a
/// nonce not used before.
async fn signed<T>(
    state: &shared::State,
    ip: std::net::IpAddr,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<T, ApiResponse>
where
    T: serde::de::DeserializeOwned + garde::Validate,
    T::Context: Default,
{
    state
        .cache
        .ratelimit("noprofit/account", 60, 60, ip.to_string())
        .await?;

    let noprofit = state.settings.get().await?.noprofit.config();
    let Some(noprofit) = noprofit else {
        return Err(unauthorized());
    };

    let request = shared::noprofit::SignedRequest {
        method: method.as_str(),
        path: uri.path(),
        timestamp: header(headers, "x-np-timestamp"),
        nonce: header(headers, "x-np-nonce"),
        signature: header(headers, "x-np-signature"),
        body,
    };
    if !noprofit.verify(&request, chrono::Utc::now().timestamp()) {
        return Err(unauthorized());
    }

    // counted atomically: a nonce seen before counts 2 and is refused, until it could no
    // longer pass the time check anyway
    if state
        .cache
        .ratelimit(
            "noprofit/nonce",
            2,
            2 * shared::noprofit::REQUEST_TOLERANCE as u64 + 10,
            request.nonce,
        )
        .await
        .is_err()
    {
        return Err(unauthorized());
    }

    let data: T = serde_json::from_slice(body)
        .map_err(|err| ApiResponse::error(format!("invalid payload: {err}")))?;
    shared::utils::validate_data(&data).map_err(ApiResponse::errors)?;

    Ok(data)
}

mod account {
    use garde::Validate;
    use serde::Deserialize;
    use utoipa::ToSchema;

    /// A noprofit.space account.
    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(range(min = 1))]
        pub account_id: i64,
    }

    pub mod summary {
        pub mod post {
            use axum::{
                body::Bytes,
                extract::OriginalUri,
                http::{HeaderMap, Method},
            };
            use serde::Serialize;
            use shared::{
                ApiError, GetIp, GetState,
                response::{ApiResponse, ApiResponseResult},
            };
            use utoipa::ToSchema;

            #[derive(ToSchema, Serialize)]
            struct Response {
                /// Whether a user here has the account. Nothing else is sent when not.
                found: bool,
                #[serde(skip_serializing_if = "Option::is_none")]
                username: Option<compact_str::CompactString>,
                /// The names of the user's servers.
                #[serde(skip_serializing_if = "Option::is_none")]
                servers: Option<Vec<compact_str::CompactString>>,
            }

            /// What deleting the account would delete here, for noprofit.space's confirmation
            /// page.
            #[utoipa::path(post, path = "/", responses(
                (status = OK, body = inline(Response)),
                (status = UNAUTHORIZED, body = ApiError),
                (status = TOO_MANY_REQUESTS, body = ApiError),
            ), request_body = inline(super::super::Payload))]
            pub async fn route(
                state: GetState,
                ip: GetIp,
                method: Method,
                OriginalUri(uri): OriginalUri,
                headers: HeaderMap,
                body: Bytes,
            ) -> ApiResponseResult {
                let data: super::super::Payload =
                    super::super::super::signed(&state, ip.0, &method, &uri, &headers, &body)
                        .await?;

                let response =
                    match shared::noprofit::account_summary(&state, data.account_id).await? {
                        Some((username, servers)) => Response {
                            found: true,
                            username: Some(username),
                            servers: Some(servers),
                        },
                        None => Response {
                            found: false,
                            username: None,
                            servers: None,
                        },
                    };

                ApiResponse::new_serialized(response).ok()
            }
        }
    }

    pub mod delete {
        pub mod post {
            use axum::{
                body::Bytes,
                extract::OriginalUri,
                http::{HeaderMap, Method, StatusCode},
            };
            use serde::Serialize;
            use shared::{
                ApiError, GetIp, GetState,
                noprofit::AccountDeletion,
                response::{ApiResponse, ApiResponseResult},
            };
            use utoipa::ToSchema;

            #[derive(ToSchema, Serialize)]
            struct Response {
                deleted: bool,
                /// How many servers were deleted with the user.
                servers: usize,
            }

            #[derive(ToSchema, Serialize)]
            struct Refused {
                /// Always `refused`.
                error: &'static str,
                /// Why, shown on noprofit.space.
                message: String,
            }

            /// Deletes the user of the account with its servers (on their nodes too), their
            /// backups and its activity. Nothing to delete is a success too. A 409 when the
            /// panel refused: the last admin, or a server its node could not delete.
            #[utoipa::path(post, path = "/", responses(
                (status = OK, body = inline(Response)),
                (status = UNAUTHORIZED, body = ApiError),
                (status = CONFLICT, body = inline(Refused)),
                (status = TOO_MANY_REQUESTS, body = ApiError),
            ), request_body = inline(super::super::Payload))]
            pub async fn route(
                state: GetState,
                ip: GetIp,
                method: Method,
                OriginalUri(uri): OriginalUri,
                headers: HeaderMap,
                body: Bytes,
            ) -> ApiResponseResult {
                let data: super::super::Payload =
                    super::super::super::signed(&state, ip.0, &method, &uri, &headers, &body)
                        .await?;

                // finishes even if noprofit.space stops waiting: a deletion cut in half
                // would leave servers without their user's account
                let deletion = {
                    let state = state.0.clone();
                    let account_id = data.account_id;

                    tokio::spawn(async move {
                        shared::noprofit::delete_account(&state, account_id).await
                    })
                    .await??
                };

                match deletion {
                    AccountDeletion::Deleted { servers } => ApiResponse::new_serialized(Response {
                        deleted: true,
                        servers,
                    })
                    .ok(),
                    AccountDeletion::Refused(message) => ApiResponse::new_serialized(Refused {
                        error: "refused",
                        message,
                    })
                    .with_status(StatusCode::CONFLICT)
                    .ok(),
                }
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/account/summary",
            OpenApiRouter::new()
                .routes(routes!(account::summary::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/account/delete",
            OpenApiRouter::new()
                .routes(routes!(account::delete::post::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
