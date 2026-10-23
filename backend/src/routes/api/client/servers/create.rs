// plugged.host: players creating their own servers (docs/features/create-server.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        options: shared::server_creation::ApiCreateOptions,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
    ) -> ApiResponseResult {
        permissions.has_user_permission("servers.deploy")?;

        ApiResponse::new_serialized(Response {
            options: shared::server_creation::options(&state, &user).await?,
        })
        .ok()
    }
}

pub mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            IntoApiObject,
            user::{GetPermissionManager, GetUser},
            user_activity::GetUserActivityLogger,
        },
        response::{ApiResponse, ApiResponseResult},
        server_creation::CreateRequest,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(chars, min = 3, max = 255))]
        #[schema(min_length = 3, max_length = 255)]
        name: compact_str::CompactString,
        #[garde(skip)]
        egg_uuid: uuid::Uuid,
        #[garde(skip)]
        location_uuid: uuid::Uuid,
        #[garde(skip)]
        plan_uuid: uuid::Uuid,
        #[garde(inner(length(chars, min = 1, max = 64)))]
        #[serde(default)]
        variant: Option<compact_str::CompactString>,
        #[garde(skip)]
        #[serde(default)]
        eula: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        server: shared::models::server::ApiServer,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = TOO_MANY_REQUESTS, body = ApiError),
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

        permissions.has_user_permission("servers.deploy")?;

        let server = match shared::server_creation::create(
            &state,
            &user,
            CreateRequest {
                name: data.name.trim().into(),
                egg_uuid: data.egg_uuid,
                location_uuid: data.location_uuid,
                plan_uuid: data.plan_uuid,
                variant: data.variant,
                eula: data.eula,
            },
        )
        .await
        {
            Ok(server) => server,
            Err(shared::server_creation::CreateError::Other(err)) => return Err(err.into()),
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
                "server:create",
                serde_json::json!({
                    "uuid": server.uuid,
                    "name": server.name,
                    "egg_uuid": server.egg.uuid,
                    "node_uuid": server.node.uuid,
                    "location_uuid": data.location_uuid,
                    "plan_uuid": data.plan_uuid,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            server: server.into_api_object(&state, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
