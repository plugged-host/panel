// plugged.host: whether this account sees ads, and the sponsored banners shown now (docs/features/ads.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        ads::Audience,
        models::{
            ad_banner::{AdBanner, ApiAdBanner},
            user::GetUser,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// `none` (no ads), `member` (the slower refresh) or `everyone`.
        audience: Audience,
        banners: Vec<ApiAdBanner>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        let audience = shared::ads::audience(&state, &user).await?;
        let banners = if audience == Audience::None {
            Vec::new()
        } else {
            let banners = AdBanner::live(&state.database).await?;
            let storage_url_retriever = state.storage.retrieve_urls().await?;

            banners
                .into_iter()
                .map(|banner| banner.into_api_object(&storage_url_retriever))
                .collect()
        };

        ApiResponse::new_serialized(Response { audience, banners }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
