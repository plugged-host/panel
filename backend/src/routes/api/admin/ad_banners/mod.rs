// plugged.host: Admin › Sponsored Banners (docs/features/ads.md#sponsored-banners).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod _ad_banner_;

mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            IntoAdminApiObject, Pagination, PaginationParamsWithSearch, ad_banner::AdBanner,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        ad_banners: Pagination<shared::models::ad_banner::AdminApiAdBanner>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ), params(
        (
            "page" = i64, Query,
            description = "The page number",
            example = "1",
        ),
        (
            "per_page" = i64, Query,
            description = "The number of items per page",
            example = "10",
        ),
        (
            "search" = Option<String>, Query,
            description = "Search term for items",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        Query(params): Query<PaginationParamsWithSearch>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&params) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("ad-banners.read")?;

        let ad_banners = AdBanner::all_with_pagination(
            &state.database,
            params.page,
            params.per_page,
            params.search.as_deref(),
        )
        .await?;
        let storage_url_retriever = state.storage.retrieve_urls().await?;

        ApiResponse::new_serialized(Response {
            ad_banners: ad_banners
                .try_async_map(|banner| {
                    banner.into_admin_api_object(&state, &storage_url_retriever)
                })
                .await?,
        })
        .ok()
    }
}

mod post {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            CreatableModel, IntoAdminApiObject,
            ad_banner::{AdBanner, CreateAdBannerOptions},
            admin_activity::GetAdminActivityLogger,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        ad_banner: shared::models::ad_banner::AdminApiAdBanner,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), request_body = inline(CreateAdBannerOptions))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<CreateAdBannerOptions>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("ad-banners.create")?;

        let banner = AdBanner::create(&state, data).await?;

        activity_logger
            .log(
                "ad-banner:create",
                serde_json::json!({
                    "uuid": banner.uuid,
                    "advertiser": banner.advertiser,
                    "link": banner.link,
                    "placement": banner.placement,
                }),
            )
            .await;

        let storage_url_retriever = state.storage.retrieve_urls().await?;

        ApiResponse::new_serialized(Response {
            ad_banner: banner
                .into_admin_api_object(&state, &storage_url_retriever)
                .await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .nest("/{ad_banner}", _ad_banner_::router(state))
        .with_state(state.clone())
}
