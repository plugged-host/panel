use super::State;
use axum::{
    extract::{Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use shared::{
    GetState,
    models::{
        ByUuid,
        user::{GetPermissionManager, User},
    },
    response::ApiResponse,
};
use std::ops::{Deref, DerefMut};
use utoipa_axum::{router::OpenApiRouter, routes};

mod activity;
mod economy; // plugged.host
mod email;
mod oauth_links;
mod servers;
mod two_factor;

#[derive(Clone)]
pub struct ParamUser(pub User);

impl Deref for ParamUser {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParamUser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type GetParamUser = shared::extract::ConsumingExtension<ParamUser>;

pub async fn auth(
    state: GetState,
    permissions: GetPermissionManager,
    Path(user): Path<Vec<String>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Err(err) = permissions.has_admin_permission("users.read") {
        return Ok(err.into_response());
    }

    let user = match uuid::Uuid::parse_str(&user[0]) {
        Ok(user) => user,
        Err(_) => {
            return Ok(ApiResponse::error("invalid user uuid")
                .with_status(StatusCode::BAD_REQUEST)
                .into_response());
        }
    };
    let user = User::by_uuid_optional(&state.database, user).await;
    let user = match user {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(ApiResponse::error("user not found")
                .with_status(StatusCode::NOT_FOUND)
                .into_response());
        }
        Err(err) => return Ok(ApiResponse::from(err).into_response()),
    };

    req.extensions_mut().insert(ParamUser(user));

    Ok(next.run(req).await)
}

mod get {
    use crate::routes::api::admin::users::_user_::GetParamUser;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{IntoAdminApiObject, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        user: shared::models::user::AdminApiUser,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "user" = uuid::Uuid,
            description = "The user ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetParamUser,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("users.read")?;

        let mut user = user
            .0
            .0
            .into_admin_api_object(&state, &state.storage.retrieve_urls().await?)
            .await?;
        shared::user_links::attach(&state, std::slice::from_mut(&mut user)).await?; // plugged.host

        ApiResponse::new_serialized(Response { user }).ok()
    }
}

mod delete {
    use crate::routes::api::admin::users::_user_::GetParamUser;
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize}; // plugged.host: Deserialize
    use shared::{
        ApiError, GetState,
        models::{
            DeletableModel, admin_activity::GetAdminActivityLogger, server::Server,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    // plugged.host: `?delete_servers=true` deletes the user's servers first, `&force=true` even when
    // their node cannot be reached (docs/features/admin-users.md)
    #[derive(Deserialize, Default)]
    pub struct Params {
        #[serde(default)]
        delete_servers: bool,
        #[serde(default)]
        force: bool,
    }

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "user" = uuid::Uuid,
            description = "The user ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        // plugged.host
        (
            "delete_servers" = Option<bool>, Query,
            description = "Delete the user's servers first (needs servers.delete)",
        ),
        (
            "force" = Option<bool>, Query,
            description = "With delete_servers: delete them even when their node cannot be reached",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetParamUser,
        activity_logger: GetAdminActivityLogger,
        axum::extract::Query(params): axum::extract::Query<Params>, // plugged.host
    ) -> ApiResponseResult {
        permissions.has_admin_permission("users.delete")?;
        permissions.can_modify_user(&user)?;

        // plugged.host: the user's servers go first when asked
        let deleted_servers = if params.delete_servers {
            permissions.has_admin_permission("servers.delete")?;

            let mut deleted = Vec::new();
            loop {
                let servers =
                    Server::by_owner_uuid_with_pagination(&state.database, user.uuid, 1, 50, None)
                        .await?;
                if servers.data.is_empty() {
                    break;
                }

                for server in servers.data {
                    let (uuid, name) = (server.uuid, server.name.clone());
                    if let Err(err) = server
                        .delete(
                            &state,
                            shared::models::server::DeleteServerOptions {
                                force: params.force,
                            },
                        )
                        .await
                    {
                        tracing::error!(server = %uuid, "failed to delete a server of a deleted user: {:?}", err);

                        return ApiResponse::error(format!(
                            "failed to delete server {name}: {err}"
                        ))
                        .with_status(StatusCode::EXPECTATION_FAILED)
                        .ok();
                    }

                    deleted.push(serde_json::json!({ "uuid": uuid, "name": name }));
                }
            }

            deleted
        } else {
            Vec::new()
        };

        let servers = Server::count_by_user_uuid(&state.database, user.uuid).await?;
        if servers > 0 {
            return ApiResponse::error("user has servers, cannot delete")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        user.delete(&state, ()).await?;

        activity_logger
            .log(
                "user:delete",
                serde_json::json!({
                    "uuid": user.uuid,
                    "username": user.username,
                    "email": user.email,
                    "name_first": user.name_first,
                    "name_last": user.name_last,
                    "admin": user.admin,
                    "deleted_servers": deleted_servers, // plugged.host
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod patch {
    use crate::routes::api::admin::users::_user_::GetParamUser;
    use axum::http::StatusCode;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel,
            admin_activity::GetAdminActivityLogger,
            user::{GetPermissionManager, GetUser, UpdateUserOptions},
            user_email_verification::UserEmailVerification,
            user_password_reset::UserPasswordReset,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_REQUEST, body = ApiError),
        (status = CONFLICT, body = ApiError),
    ), params(
        (
            "user" = uuid::Uuid,
            description = "The user ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(UpdateUserOptions))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        caller: GetUser,
        mut user: GetParamUser,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<UpdateUserOptions>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("users.update")?;
        permissions.can_modify_user(&user)?;

        if !permissions.is_unrestricted_admin() {
            if data.admin.is_some_and(|admin| admin != user.admin) {
                return ApiResponse::error("you cannot change admin status")
                    .with_status(StatusCode::FORBIDDEN)
                    .ok();
            }
            if data
                .role_uuid
                .is_some_and(|role_uuid| role_uuid != user.role.as_ref().map(|role| role.uuid))
            {
                return ApiResponse::error("you cannot change a user's role")
                    .with_status(StatusCode::FORBIDDEN)
                    .ok();
            }

            if caller.uuid != user.uuid || !permissions.scope().is_session() {
                if data.password.is_some() {
                    return ApiResponse::error(
                        "you can only change your own password from a session",
                    )
                    .with_status(StatusCode::FORBIDDEN)
                    .ok();
                }
                if data
                    .email
                    .as_ref()
                    .is_some_and(|email| *email != user.email)
                {
                    return ApiResponse::error("you can only change your own email from a session")
                        .with_status(StatusCode::FORBIDDEN)
                        .ok();
                }
            }
        }

        // plugged.host: a noprofit.space account's username and email are noprofit.space's
        // (the Pelican panel's identity lock; user_links.rs)
        if (data
            .username
            .as_ref()
            .is_some_and(|username| *username != user.username)
            || data
                .email
                .as_ref()
                .is_some_and(|email| *email != user.email))
            && shared::user_links::identity_locked(&state, user.uuid).await?
        {
            return ApiResponse::error("username and email are managed on noprofit.space")
                .with_status(StatusCode::CONFLICT)
                .ok();
        }

        let old_email = user.email.clone();

        match user.update(&state, data).await {
            Ok(_) => {}
            Err(err) if err.is_unique_violation() => {
                return ApiResponse::error("user with email/username/external_id already exists")
                    .with_status(StatusCode::CONFLICT)
                    .ok();
            }
            Err(err) => return ApiResponse::from(err).ok(),
        }

        if user.email != old_email {
            UserEmailVerification::delete_by_user_uuid(&state.database, user.uuid).await?;
            UserPasswordReset::delete_by_user_uuid(&state.database, user.uuid).await?;
        }

        activity_logger
            .log(
                "user:update",
                serde_json::json!({
                    "uuid": user.uuid,
                    "role_uuid": user.role.as_ref().map(|r| r.uuid),
                    "username": user.username,
                    "email": user.email,
                    "name_first": user.name_first,
                    "name_last": user.name_last,
                    "admin": user.admin,
                    "language": user.language,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(delete::route))
        .routes(routes!(patch::route))
        .nest("/two-factor", two_factor::router(state))
        .nest("/servers", servers::router(state))
        .nest("/activity", activity::router(state))
        .nest("/oauth-links", oauth_links::router(state))
        .nest("/email", email::router(state))
        .nest("/economy", economy::router(state)) // plugged.host
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone())
}
