// plugged.host: the server's VS Code page: the devbox its person works in (shared/src/devboxes,
// docs/features/devboxes.md). Every action checks access again: the person may have lost their
// membership or permission since the page loaded.
use super::State;
use shared::{
    devboxes::{Context, Devbox, DevboxError},
    models::server::Server,
    response::{ApiResponse, ApiResponseResult},
};
use utoipa_axum::{router::OpenApiRouter, routes};

/// The card after an action.
async fn card(
    state: &shared::State,
    context: &Context,
    user: &shared::models::user::User,
    game: &Server,
) -> ApiResponseResult {
    ApiResponse::new_serialized(
        shared::devboxes::view::card(state, context, user.uuid, &user.username, game).await?,
    )
    .ok()
}

/// The devbox this person works in on the server, when there is one.
async fn current(
    state: &shared::State,
    context: &Context,
    user_uuid: uuid::Uuid,
    game: &Server,
) -> Result<Option<Devbox>, DevboxError> {
    Ok(shared::devboxes::resolve(state, context, user_uuid, game)
        .await?
        .and_then(|(_, devbox)| devbox))
}

mod get {
    use shared::{
        ApiError, GetState,
        devboxes::{Context, view::ApiDevboxCard},
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::ApiResponseResult,
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = ApiDevboxCard),
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
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("devbox.connect")?;
        let context = Context::load(&state).await?;

        // the page polls while a stop is on its way: carry it on
        if let Ok(Some(devbox)) = super::current(&state, &context, user.uuid, &server).await
            && devbox.is_stopping()
            && let Err(err) = shared::devboxes::power::settle(&state, &devbox, None).await
        {
            tracing::debug!(devbox = %devbox.uuid, "failed to settle a devbox stop: {}", err.message());
        }

        super::card(&state, &context, &user, &server).await
    }
}

mod start {
    pub mod post {
        use shared::{
            ApiError, GetState,
            devboxes::{Context, DevboxError, view::ApiDevboxCard},
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = ApiDevboxCard),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
            (status = CONFLICT, body = ApiError),
            (status = BAD_GATEWAY, body = ApiError),
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
            user: GetUser,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
        ) -> ApiResponseResult {
            permissions.has_server_permission("devbox.connect")?;
            let context = Context::load(&state).await?;

            let started = async {
                let devbox =
                    shared::devboxes::provision::ensure(&state, &context, user.uuid, &server)
                        .await?;
                if devbox.is_stopping() {
                    return Err(DevboxError::Stopping);
                }
                if devbox.token_hash.is_some() {
                    return Ok(None); // already running
                }
                shared::devboxes::power::start(&state, &context, &devbox, user.uuid).await?;

                Ok(Some(devbox))
            }
            .await;
            match started {
                Ok(Some(devbox)) => {
                    activity_logger
                        .log(
                            "server:devbox.start",
                            serde_json::json!({ "shared": devbox.is_shared() }),
                        )
                        .await;
                }
                Ok(None) => {}
                Err(err) => return err.response().ok(),
            }

            super::super::card(&state, &context, &user, &server).await
        }
    }
}

mod stop {
    pub mod post {
        use shared::{
            ApiError, GetState,
            devboxes::{Context, DevboxError, view::ApiDevboxCard},
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::{GetPermissionManager, GetUser},
            },
            response::ApiResponseResult,
        };

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = ApiDevboxCard),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
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
            user: GetUser,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
        ) -> ApiResponseResult {
            permissions.has_server_permission("devbox.connect")?;
            let context = Context::load(&state).await?;

            let devbox = match super::super::current(&state, &context, user.uuid, &server).await {
                Ok(Some(devbox)) => devbox,
                Ok(None) => return DevboxError::Missing.response().ok(),
                Err(err) => return err.response().ok(),
            };
            // a shared devbox is the owner's to stop
            if devbox.is_shared() && server.owner.uuid != user.uuid {
                return DevboxError::NotAllowed.response().ok();
            }
            if let Err(err) = shared::devboxes::power::stop(&state, &devbox).await {
                return err.response().ok();
            }
            activity_logger
                .log(
                    "server:devbox.stop",
                    serde_json::json!({ "shared": devbox.is_shared() }),
                )
                .await;

            super::super::card(&state, &context, &user, &server).await
        }
    }
}

mod browser {
    pub mod post {
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            devboxes::{Context, DevboxError},
            models::{
                server::GetServer,
                user::{GetPermissionManager, GetUser},
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// A one-time link that signs the browser in on the devbox's own address.
            url: String,
        }

        #[utoipa::path(post, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
            (status = CONFLICT, body = ApiError),
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
            user: GetUser,
            server: GetServer,
        ) -> ApiResponseResult {
            permissions.has_server_permission("devbox.connect")?;
            let context = Context::load(&state).await?;

            let devbox = match super::super::current(&state, &context, user.uuid, &server).await {
                Ok(Some(devbox)) => devbox,
                Ok(None) => return DevboxError::Missing.response().ok(),
                Err(err) => return err.response().ok(),
            };
            if !shared::devboxes::web::allowed(&state, &context, &devbox, user.uuid).await? {
                return DevboxError::NotAllowed.response().ok();
            }

            ApiResponse::new_serialized(Response {
                url: shared::devboxes::web::login_url(&state, &context, &devbox, user.uuid).await?,
            })
            .ok()
        }
    }
}

mod shared_devbox {
    pub mod put {
        use axum::http::StatusCode;
        use garde::Validate;
        use serde::Deserialize;
        use shared::{
            ApiError, GetState,
            devboxes::{Context, DevboxError, SharedRights, view::ApiDevboxCard},
            models::{
                server::{GetServer, GetServerActivityLogger},
                user::{GetPermissionManager, GetUser},
            },
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            /// One devbox for the owner and their subusers instead of one each.
            #[garde(skip)]
            enabled: Option<bool>,
            /// What `ph` in the shared devbox may do on the server.
            #[garde(skip)]
            rights: Option<SharedRights>,
        }

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = ApiDevboxCard),
            (status = UNAUTHORIZED, body = ApiError),
            (status = FORBIDDEN, body = ApiError),
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
            user: GetUser,
            server: GetServer,
            activity_logger: GetServerActivityLogger,
            shared::Payload(data): shared::Payload<Payload>,
        ) -> ApiResponseResult {
            if let Err(errors) = shared::utils::validate_data(&data) {
                return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
            permissions.has_server_permission("devbox.connect")?;
            let context = Context::load(&state).await?;
            // the owner's to decide, and to pay for
            if server.owner.uuid != user.uuid || !context.ready() {
                return DevboxError::NotAllowed.response().ok();
            }

            match data.enabled {
                Some(true) => {
                    if shared::devboxes::tier(&state, &context, user.uuid)
                        .await?
                        .is_none()
                    {
                        return DevboxError::NotAllowed.response().ok();
                    }
                    if let Err(err) =
                        shared::devboxes::provision::enable_shared(&state, &context, &server).await
                    {
                        return err.response().ok();
                    }
                    activity_logger
                        .log("server:devbox.share", serde_json::json!({}))
                        .await;
                }
                Some(false) => {
                    if let Err(err) =
                        shared::devboxes::provision::disable_shared(&state, &context, &server).await
                    {
                        return err.response().ok();
                    }
                    activity_logger
                        .log("server:devbox.unshare", serde_json::json!({}))
                        .await;
                }
                None => {}
            }

            if let Some(rights) = data.rights
                && let Some(devbox) =
                    shared::devboxes::Devbox::shared_of(&state.database, server.uuid).await?
            {
                sqlx::query("UPDATE devboxes SET shared_rights = $2 WHERE uuid = $1")
                    .bind(devbox.uuid)
                    .bind(serde_json::to_value(rights)?)
                    .execute(state.database.write())
                    .await?;
                activity_logger
                    .log(
                        "server:devbox.rights",
                        serde_json::json!({
                            "status": rights.status,
                            "logs": rights.logs,
                            "power": rights.power,
                            "console": rights.console,
                        }),
                    )
                    .await;
                // a running devbox's token carried the old rights: it starts over
                if devbox.is_running()
                    && let Err(err) =
                        shared::devboxes::power::restart(&state, &context, &devbox).await
                {
                    return err.response().ok();
                }
            }

            super::super::card(&state, &context, &user, &server).await
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .nest(
            "/start",
            OpenApiRouter::new()
                .routes(routes!(start::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/stop",
            OpenApiRouter::new()
                .routes(routes!(stop::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/browser",
            OpenApiRouter::new()
                .routes(routes!(browser::post::route))
                .with_state(state.clone()),
        )
        .nest(
            "/shared",
            OpenApiRouter::new()
                .routes(routes!(shared_devbox::put::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
