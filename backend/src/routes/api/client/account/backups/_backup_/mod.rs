// plugged.host: one copy on the account's Backups page (docs/features/backups-page.md).

use super::State;
use utoipa_axum::router::OpenApiRouter;

mod download;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/download", download::router(state))
        .with_state(state.clone())
}
