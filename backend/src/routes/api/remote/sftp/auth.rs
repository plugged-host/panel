use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, server::Server, user::User},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AuthenticationType {
        Password,
        PublicKey,
    }

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        r#type: AuthenticationType,
        username: compact_str::CompactString,
        password: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        user: uuid::Uuid,
        server: uuid::Uuid,

        permissions: Vec<&'a str>,
        ignored_files: &'a [compact_str::CompactString],
    }

    /// plugged.host: the user behind an SFTP username when it signs in with noprofit.space, and its
    /// account. `None` without noprofit.space's account API, for unknown usernames and for the
    /// panel's own users (the emergency admin), which keep the panel's password check.
    async fn noprofit_user(
        state: &shared::State,
        username: &str,
    ) -> Result<Option<(&'static shared::noprofit::Config, User, i64)>, anyhow::Error> {
        let Some(noprofit) = shared::noprofit::config().filter(|noprofit| noprofit.sync_enabled())
        else {
            return Ok(None);
        };
        let Some(user) = User::by_username(&state.database, username).await? else {
            return Ok(None);
        };
        let Some(link) = shared::noprofit::Link::by_user_uuid(&state.database, user.uuid).await?
        else {
            return Ok(None);
        };

        Ok(Some((noprofit, user, link.account_id)))
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = EXPECTATION_FAILED, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        node: GetNode,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        let ratelimit = state
            .settings
            .get_as(|s| s.ratelimits.remote_sftp_auth)
            .await?;
        state
            .cache
            .ratelimit(
                "remote/sftp/auth",
                ratelimit.hits,
                ratelimit.window_seconds,
                node.uuid.to_string(),
            )
            .await?;

        let Some((user, server)) = data.username.rsplit_once('.') else {
            return ApiResponse::error("invalid username")
                .with_status(StatusCode::EXPECTATION_FAILED)
                .ok();
        };

        let user = match data.r#type {
            AuthenticationType::Password => {
                if !state
                    .settings
                    .get_as(|s| s.app.password_login_enabled)
                    .await?
                {
                    return ApiResponse::error("password login is disabled")
                        .with_status(StatusCode::EXPECTATION_FAILED)
                        .ok();
                }

                // plugged.host: a user that signs in with noprofit.space has no panel password;
                // noprofit.space checks its account's password instead.
                if let Some((noprofit, linked, account_id)) = noprofit_user(&state, user).await? {
                    match noprofit
                        .verify_password(&state.client, account_id, &data.password)
                        .await
                    {
                        Some(true) => linked,
                        Some(false) => {
                            return ApiResponse::error("invalid password")
                                .with_status(StatusCode::EXPECTATION_FAILED)
                                .ok();
                        }
                        None => {
                            return ApiResponse::error(
                                "noprofit.space could not check the password",
                            )
                            .with_status(StatusCode::EXPECTATION_FAILED)
                            .ok();
                        }
                    }
                } else {
                    match User::by_username_password(&state.database, user, &data.password).await? {
                        Some(user) if user.password_login_disabled => {
                            return ApiResponse::error(
                                "password login is disabled for this account",
                            )
                            .with_status(StatusCode::EXPECTATION_FAILED)
                            .ok();
                        }
                        Some(user) => user,
                        None => {
                            return ApiResponse::error("user not found")
                                .with_status(StatusCode::EXPECTATION_FAILED)
                                .ok();
                        }
                    }
                }
            }
            AuthenticationType::PublicKey => {
                let public_key = match russh::keys::PublicKey::from_openssh(&data.password) {
                    Ok(public_key) => public_key,
                    Err(_) => {
                        return ApiResponse::error("invalid public key")
                            .with_status(StatusCode::EXPECTATION_FAILED)
                            .ok();
                    }
                };

                match User::by_username_public_key(&state.database, user, public_key).await? {
                    Some(user) => user,
                    None => {
                        return ApiResponse::error("user not found")
                            .with_status(StatusCode::EXPECTATION_FAILED)
                            .ok();
                    }
                }
            }
        };

        if user.suspended {
            return ApiResponse::error("account is suspended")
                .with_status(StatusCode::EXPECTATION_FAILED)
                .ok();
        }

        let settings = state.settings.get().await?;
        let two_factor_missing =
            user.require_two_factor(&settings) && !user.satisfies_two_factor(&settings);
        let email_unverified = user.require_email_verification(&settings) && !user.email_verified;
        drop(settings);

        if two_factor_missing {
            return ApiResponse::error("two-factor authentication required")
                .with_status(StatusCode::EXPECTATION_FAILED)
                .ok();
        }

        if email_unverified {
            return ApiResponse::error("email verification required")
                .with_status(StatusCode::EXPECTATION_FAILED)
                .ok();
        }

        let server = match Server::by_user_identifier(&state.database, &user, server).await? {
            Some(server) => server,
            None => {
                return ApiResponse::error("server not found")
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            }
        };

        if server.node.uuid != node.uuid {
            return ApiResponse::error("server not found")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        ApiResponse::new_serialized(Response {
            user: user.uuid,
            server: server.uuid,
            permissions: server.wings_permissions(
                &*state.settings.get().await?,
                &user,
                &shared::models::user::CredentialScope::Session,
            ),
            ignored_files: server.subuser_ignored_files.as_deref().unwrap_or(&[]),
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
