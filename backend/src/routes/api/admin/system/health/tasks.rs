//! plugged.host: when each background task last ran and whether it failed (docs/features/health.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ResponseTask {
        name: &'static str,
        /// When it last started (when the panel started, for one that has not run yet).
        last_run: chrono::DateTime<chrono::Utc>,
        /// The error of its last run, if it failed.
        error: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// Background tasks run on the primary instance only; elsewhere the list is empty.
        primary: bool,
        tasks: Vec<ResponseTask>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("stats.read")?;

        let now = chrono::Utc::now();
        let mut tasks: Vec<ResponseTask> = state
            .background_tasks
            .get_tasks()
            .await
            .values()
            .map(|task| ResponseTask {
                name: task.name,
                last_run: now
                    - chrono::Duration::from_std(task.last_execution.elapsed())
                        .unwrap_or(chrono::Duration::zero()),
                error: task.last_error.as_ref().map(|err| format!("{err:#}")),
            })
            .collect();
        tasks.sort_by_key(|task| task.name);

        ApiResponse::new_serialized(Response {
            primary: state.env.app_primary,
            tasks,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
