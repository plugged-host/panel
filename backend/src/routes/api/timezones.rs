use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::response::{ApiResponse, ApiResponseResult};
    use std::sync::LazyLock;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        timezones: &'a [&'static str],
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route() -> ApiResponseResult {
        static TIMEZONE_NAMES: LazyLock<Vec<&'static str>> =
            LazyLock::new(|| chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name()).collect());

        ApiResponse::new_serialized(Response {
            timezones: &TIMEZONE_NAMES,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
