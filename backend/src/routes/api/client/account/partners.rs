//! plugged.host: Account › Partners, applying to the partner program (docs/features/partners.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// 404 while the program is switched off (an applicant's partnerships show again when it is on).
async fn switched_off(state: &shared::State) -> Result<bool, anyhow::Error> {
    Ok(!state.settings.get().await?.partners.enabled)
}

fn not_offered() -> shared::response::ApiResponseResult {
    shared::response::ApiResponse::error("the partner program is not open on this panel")
        .with_status(axum::http::StatusCode::NOT_FOUND)
        .ok()
}

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::user::{GetPermissionManager, GetUser},
        partners::ApiPartners,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    pub struct Response {
        pub partners: ApiPartners,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("partners.read")?;

        if super::switched_off(&state).await? {
            return super::not_offered();
        }

        ApiResponse::new_serialized(Response {
            partners: shared::partners::page(&state, &user).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// An invite link (`https://discord.gg/<code>`), an invite code or the server's id.
        #[garde(length(chars, min = 1, max = 200))]
        server: String,
        /// `fivem`, `minecraft` or `both`.
        #[garde(length(chars, min = 1, max = 16))]
        game: String,
        #[garde(length(chars, min = 1, max = 1100))]
        description: String,
        /// The panel's servers the community runs (the account's own or ones it is a subuser of).
        #[garde(length(max = 10))]
        #[serde(default)]
        servers: Vec<uuid::Uuid>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(super::get::Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = UNPROCESSABLE_ENTITY, body = ApiError, description = "Refused: `refusal` says what the check found"),
        (status = TOO_MANY_REQUESTS, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_user_permission("partners.apply")?;
        permissions.deny_while_impersonating()?;

        if super::switched_off(&state).await? {
            return super::not_offered();
        }
        // every try asks Discord a few times
        state
            .cache
            .ratelimit("partners/apply", 6, 600, user.uuid.to_string())
            .await?;

        let application_uuid = match shared::partners::apply(
            &state,
            &user,
            shared::partners::Apply {
                server: data.server,
                game: data.game,
                description: data.description,
                servers: data.servers,
            },
        )
        .await
        {
            Ok(application_uuid) => application_uuid,
            Err(err) => return err.response(),
        };

        let page = shared::partners::page(&state, &user).await?;
        let guild = page
            .applications
            .iter()
            .find(|application| application.uuid == application_uuid);
        activity_logger
            .log(
                "account:partners.apply",
                serde_json::json!({
                    "uuid": application_uuid,
                    "guild_id": guild.map(|guild| &guild.guild_id),
                    "guild_name": guild.map(|guild| &guild.guild_name),
                }),
            )
            .await;
        shared::notifications::send_admins(
            &state,
            "partners.manage",
            None,
            shared::notifications::ADMIN_PARTNER_APPLICATION,
            serde_json::json!({
                "guild": guild.map(|guild| &guild.guild_name),
                "user": user.username,
            }),
        )
        .await;
        shared::partners::spawn_assess(&state, application_uuid);

        ApiResponse::new_serialized(super::get::Response { partners: page }).ok()
    }
}

mod delete {
    use axum::http::StatusCode;
    use shared::{
        ApiError, GetState,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(super::get::Response)),
        (status = NOT_FOUND, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_user_permission("partners.apply")?;
        permissions.deny_while_impersonating()?;

        if super::switched_off(&state).await? {
            return super::not_offered();
        }
        if !shared::partners::withdraw(&state, user.uuid).await? {
            return ApiResponse::error("you have no application waiting")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        }

        activity_logger
            .log("account:partners.withdraw", serde_json::json!({}))
            .await;

        ApiResponse::new_serialized(super::get::Response {
            partners: shared::partners::page(&state, &user).await?,
        })
        .ok()
    }
}

mod check {
    pub mod post {
        use axum::extract::Path;
        use shared::{
            ApiError, GetState,
            models::{
                user::{GetPermissionManager, GetUser},
                user_activity::GetUserActivityLogger,
            },
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(post, path = "/{application}/check", responses(
            (status = OK, body = inline(super::super::get::Response)),
            (status = NOT_FOUND, body = ApiError),
            (status = CONFLICT, body = ApiError),
            (status = TOO_MANY_REQUESTS, body = ApiError),
            (status = BAD_GATEWAY, body = ApiError),
        ), params(
            ("application" = uuid::Uuid, Path, description = "The partner application ID"),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
            activity_logger: GetUserActivityLogger,
            Path(application_uuid): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            permissions.has_user_permission("partners.apply")?;

            if super::super::switched_off(&state).await? {
                return super::super::not_offered();
            }
            state
                .cache
                .ratelimit("partners/check", 5, 3600, user.uuid.to_string())
                .await?;

            if let Err(err) =
                shared::partners::check_partnership(&state, user.uuid, application_uuid).await
            {
                return err.response();
            }
            activity_logger
                .log(
                    "account:partners.check",
                    serde_json::json!({ "uuid": application_uuid }),
                )
                .await;

            ApiResponse::new_serialized(super::super::get::Response {
                partners: shared::partners::page(&state, &user).await?,
            })
            .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .routes(routes!(delete::route))
        .routes(routes!(check::post::route))
        .with_state(state.clone())
}
