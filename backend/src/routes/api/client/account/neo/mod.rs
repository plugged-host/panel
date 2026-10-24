// plugged.host: neo, the assistant (shared/src/neo, docs/features/neo.md): the account's chat,
// the person's switches, their limits and models, chat images, and deleting every chat.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

pub(crate) mod chat;

mod delete {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        neo::conversations::Conversation,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        deleted: u64,
    }

    /// Deletes every chat of the account, on every server, with their images.
    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetUserActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_user_permission("neo.chat")?;

        let deleted = Conversation::delete_all(&state.database, user.uuid).await?;

        activity_logger
            .log(
                "account:neo.delete-chats",
                serde_json::json!({"chats": deleted}),
            )
            .await;

        ApiResponse::new_serialized(Response { deleted }).ok()
    }
}

mod preferences {
    pub mod patch {
        use axum::http::StatusCode;
        use garde::Validate;
        use serde::{Deserialize, Serialize};
        use shared::{
            ApiError, GetState,
            models::{
                user::{GetPermissionManager, GetUser},
                user_activity::GetUserActivityLogger,
            },
            neo::conversations::{PreferenceChange, Preferences},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Validate, Deserialize)]
        pub struct Payload {
            #[garde(skip)]
            enabled: Option<bool>,
            /// Run changes without asking first (destructive ones may still ask, critical ones
            /// always do).
            #[garde(skip)]
            auto_approve: Option<bool>,
            /// The model the chat uses; empty for the default.
            #[garde(length(chars, max = 100))]
            model: Option<String>,
            /// The person read what neo is and agreed to it.
            #[garde(skip)]
            acknowledged: Option<bool>,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            preferences: Preferences,
        }

        #[utoipa::path(patch, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = BAD_REQUEST, body = ApiError),
            (status = UNAUTHORIZED, body = ApiError),
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
            permissions.has_user_permission("neo.chat")?;

            let mut changes = Vec::new();
            if let Some(enabled) = data.enabled {
                changes.push(PreferenceChange::Enabled(enabled));
            }
            if let Some(auto_approve) = data.auto_approve {
                changes.push(PreferenceChange::AutoApprove(auto_approve));
            }
            if let Some(model) = &data.model {
                let model = model.trim();
                changes.push(PreferenceChange::Model(
                    (!model.is_empty()).then(|| model.to_string()),
                ));
            }
            if data.acknowledged == Some(true) {
                changes.push(PreferenceChange::Acknowledged);
            }

            let mut preferences = Preferences::of(&state.database, user.uuid).await?;
            for change in changes {
                preferences = Preferences::change(&state.database, user.uuid, change).await?;
            }

            activity_logger
                .log(
                    "account:neo.preferences",
                    serde_json::json!({
                        "enabled": data.enabled,
                        "auto_approve": data.auto_approve,
                        "model": data.model,
                        "acknowledged": data.acknowledged,
                    }),
                )
                .await;

            ApiResponse::new_serialized(Response { preferences }).ok()
        }
    }
}

mod usage {
    pub mod get {
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::user::{GetPermissionManager, GetUser},
            neo::UsageInfo,
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// None while neo cannot tell (noprofit.space not answering).
            usage: Option<UsageInfo>,
        }

        /// Where the person's limits stand and the models they can pick.
        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = UNAUTHORIZED, body = ApiError),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            user: GetUser,
        ) -> ApiResponseResult {
            permissions.has_user_permission("neo.chat")?;

            ApiResponse::new_serialized(Response {
                usage: shared::neo::usage(&state, user.uuid).await,
            })
            .ok()
        }
    }
}

mod images {
    pub mod _image_ {
        pub mod get {
            use axum::{extract::Path, http::StatusCode};
            use shared::{
                ApiError, GetState,
                models::user::{GetPermissionManager, GetUser},
                response::{ApiResponse, ApiResponseResult},
            };

            /// An image of one of the person's chats, as JPEG.
            #[utoipa::path(get, path = "/", responses(
                (status = OK, content_type = "image/jpeg", body = Vec<u8>),
                (status = NOT_FOUND, body = ApiError),
            ), params(
                (
                    "image" = uuid::Uuid,
                    description = "The image ID",
                    example = "123e4567-e89b-12d3-a456-426614174000",
                ),
            ))]
            pub async fn route(
                state: GetState,
                permissions: GetPermissionManager,
                user: GetUser,
                Path(image): Path<uuid::Uuid>,
            ) -> ApiResponseResult {
                permissions.has_user_permission("neo.chat")?;

                match shared::neo::images::own(&state.database, image, user.uuid).await? {
                    Some(jpeg) => ApiResponse::new(axum::body::Body::from(jpeg))
                        .with_header("Content-Type", "image/jpeg")
                        .with_header("Cache-Control", "private, max-age=86400")
                        .ok(),
                    None => ApiResponse::error("image not found")
                        .with_status(StatusCode::NOT_FOUND)
                        .ok(),
                }
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .nest(
            "/preferences",
            OpenApiRouter::new()
                .routes(routes!(preferences::patch::route))
                .with_state(state.clone()),
        )
        .nest(
            "/usage",
            OpenApiRouter::new()
                .routes(routes!(usage::get::route))
                .with_state(state.clone()),
        )
        .nest(
            "/images/{image}",
            OpenApiRouter::new()
                .routes(routes!(images::_image_::get::route))
                .with_state(state.clone()),
        )
        .merge(chat::router(state))
        .with_state(state.clone())
}
