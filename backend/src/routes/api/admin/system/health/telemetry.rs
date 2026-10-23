//! plugged.host: nodes whose telemetry sample is missing or old, which means the sampler stopped
//! or cannot finish (docs/features/node-telemetry.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use sqlx::Row;
    use utoipa::ToSchema;

    /// A sample older than this is stale (the sampler runs every minute).
    const STALE_AFTER_SECONDS: i64 = 150;

    #[derive(ToSchema, Serialize)]
    struct ResponseNode {
        uuid: uuid::Uuid,
        name: String,
        sampled: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        total: i64,
        stale_after_seconds: i64,
        stale: Vec<ResponseNode>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("stats.read")?;

        let rows = sqlx::query(
            r#"
            SELECT nodes.uuid, nodes.name, node_telemetry.sampled
            FROM nodes
            LEFT JOIN node_telemetry ON node_telemetry.node_uuid = nodes.uuid
            ORDER BY nodes.name
            "#,
        )
        .fetch_all(state.database.read())
        .await?;

        let now = chrono::Utc::now();
        let mut stale = Vec::new();
        for row in &rows {
            let sampled = row
                .try_get::<Option<chrono::NaiveDateTime>, _>("sampled")?
                .map(|sampled| sampled.and_utc());
            if sampled.is_none_or(|sampled| (now - sampled).num_seconds() > STALE_AFTER_SECONDS) {
                stale.push(ResponseNode {
                    uuid: row.try_get("uuid")?,
                    name: row.try_get("name")?,
                    sampled,
                });
            }
        }

        ApiResponse::new_serialized(Response {
            total: rows.len() as i64,
            stale_after_seconds: STALE_AFTER_SECONDS,
            stale,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
