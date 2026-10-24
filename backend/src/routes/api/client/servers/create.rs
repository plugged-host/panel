// plugged.host: players creating their own servers (docs/features/create-server.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::extract::Query;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        allowance::Resources,
        models::user::{GetPermissionManager, GetUser},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    /// The size the page has picked so far; with all three, the locations say whether they have
    /// room for it.
    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        cpu: Option<i64>,
        memory: Option<i64>,
        disk: Option<i64>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        options: shared::server_creation::ApiCreateOptions,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ), params(
        ("cpu" = Option<i64>, Query, description = "CPU of the server to place, in percent"),
        ("memory" = Option<i64>, Query, description = "Memory of the server to place, in MiB"),
        ("disk" = Option<i64>, Query, description = "Disk of the server to place, in MiB"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_user_permission("servers.deploy")?;

        let size = match (params.cpu, params.memory, params.disk) {
            (Some(cpu), Some(memory), Some(disk)) => Some(Resources::new(cpu, memory, disk)),
            _ => None,
        };

        ApiResponse::new_serialized(Response {
            options: shared::server_creation::options(&state, &user, size).await?,
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
        allowance::Resources,
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
        #[garde(length(chars, min = 1, max = 191))]
        #[schema(min_length = 1, max_length = 191)]
        name: compact_str::CompactString,
        #[garde(skip)]
        egg_uuid: uuid::Uuid,
        #[garde(skip)]
        location_uuid: uuid::Uuid,
        /// A plan to take the size from; without one, `cpu`, `memory` and `disk`.
        #[garde(skip)]
        #[serde(default)]
        plan_uuid: Option<uuid::Uuid>,
        /// CPU in percent of one thread.
        #[garde(inner(range(min = 1)))]
        #[serde(default)]
        cpu: Option<i32>,
        /// Memory in MiB.
        #[garde(inner(range(min = 1)))]
        #[serde(default)]
        memory: Option<i64>,
        /// Disk in MiB.
        #[garde(inner(range(min = 1)))]
        #[serde(default)]
        disk: Option<i64>,
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
                size: match (data.cpu, data.memory, data.disk) {
                    (Some(cpu), Some(memory), Some(disk)) => {
                        Some(Resources::new(cpu as i64, memory, disk))
                    }
                    _ => None,
                },
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
                    "cpu": server.cpu,
                    "memory": server.memory,
                    "disk": server.disk,
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
