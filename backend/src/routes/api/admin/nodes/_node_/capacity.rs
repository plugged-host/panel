use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{node::GetNode, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize, Deserialize)]
    struct ResponseStats {
        servers: i64,
        cpu: i64,
        memory: i64,
        memory_overhead: i64,
        disk: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseServer {
        uuid: uuid::Uuid,
        name: compact_str::CompactString,
        suspended: bool,
        cpu: i32,
        memory: i64,
        memory_overhead: i64,
        disk: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseLimits {
        memory: i64,
        disk: i64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        limits: ResponseLimits,
        #[schema(inline)]
        allocated: ResponseStats,
        #[schema(inline)]
        servers: Vec<ResponseServer>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "node" = uuid::Uuid,
            description = "The node ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        node: GetNode,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("nodes.read")?;

        let (allocated, servers) = tokio::try_join!(
            sqlx::query_as_unchecked!(
                ResponseStats,
                "SELECT
                    COUNT(*) as servers,
                    COALESCE(SUM(cpu), 0)::int8 as cpu,
                    COALESCE(SUM(memory), 0)::int8 as memory,
                    COALESCE(SUM(memory_overhead), 0)::int8 as memory_overhead,
                    COALESCE(SUM(disk), 0)::int8 as disk
                FROM servers
                WHERE servers.node_uuid = $1",
                node.uuid
            )
            .fetch_one(state.database.read()),
            sqlx::query!(
                "SELECT
                    servers.uuid,
                    servers.name,
                    servers.suspended,
                    servers.cpu,
                    servers.memory,
                    servers.memory_overhead,
                    servers.disk
                FROM servers
                WHERE servers.node_uuid = $1",
                node.uuid
            )
            .fetch_all(state.database.read()),
        )?;

        let servers = servers
            .into_iter()
            .map(|row| ResponseServer {
                uuid: row.uuid,
                name: row.name.into(),
                suspended: row.suspended,
                cpu: row.cpu,
                memory: row.memory,
                memory_overhead: row.memory_overhead,
                disk: row.disk,
            })
            .collect();

        ApiResponse::new_serialized(Response {
            limits: ResponseLimits {
                memory: node.memory,
                disk: node.disk,
            },
            allocated,
            servers,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
