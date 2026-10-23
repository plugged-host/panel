// plugged.host: one banner in Admin › Sponsored Banners (docs/features/ads.md#sponsored-banners).

use super::State;
use axum::{
    extract::{DefaultBodyLimit, Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use shared::{
    GetState,
    models::{ByUuid, ad_banner::AdBanner, user::GetPermissionManager},
    response::ApiResponse,
};
use utoipa_axum::{
    router::{OpenApiRouter, UtoipaMethodRouterExt},
    routes,
};

pub type GetAdBanner = shared::extract::ConsumingExtension<AdBanner>;

/// The largest image a banner may have.
const IMAGE_LIMIT: usize = 4 * 1024 * 1024;

pub async fn auth(
    state: GetState,
    permissions: GetPermissionManager,
    Path(ad_banner): Path<Vec<String>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ad_banner = match ad_banner.first().map(|s| s.parse::<uuid::Uuid>()) {
        Some(Ok(id)) => id,
        _ => {
            return Ok(ApiResponse::error("invalid banner uuid")
                .with_status(StatusCode::BAD_REQUEST)
                .into_response());
        }
    };

    if let Err(err) = permissions.has_admin_permission("ad-banners.read") {
        return Ok(err.into_response());
    }

    let ad_banner = match AdBanner::by_uuid_optional(&state.database, ad_banner).await {
        Ok(Some(ad_banner)) => ad_banner,
        Ok(None) => {
            return Ok(ApiResponse::error("banner not found")
                .with_status(StatusCode::NOT_FOUND)
                .into_response());
        }
        Err(err) => return Ok(ApiResponse::from(err).into_response()),
    };

    req.extensions_mut().insert(ad_banner);

    Ok(next.run(req).await)
}

mod get {
    use super::GetAdBanner;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{IntoAdminApiObject, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        ad_banner: shared::models::ad_banner::AdminApiAdBanner,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "ad_banner" = uuid::Uuid,
            description = "The banner ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        ad_banner: GetAdBanner,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("ad-banners.read")?;

        let storage_url_retriever = state.storage.retrieve_urls().await?;

        ApiResponse::new_serialized(Response {
            ad_banner: ad_banner
                .0
                .into_admin_api_object(&state, &storage_url_retriever)
                .await?,
        })
        .ok()
    }
}

mod delete {
    use super::GetAdBanner;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            DeletableModel, admin_activity::GetAdminActivityLogger, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "ad_banner" = uuid::Uuid,
            description = "The banner ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        ad_banner: GetAdBanner,
        activity_logger: GetAdminActivityLogger,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("ad-banners.delete")?;

        ad_banner.delete(&state, ()).await?;
        if let Err(err) = state.storage.remove(ad_banner.image.as_deref()).await {
            tracing::warn!(banner = %ad_banner.uuid, "failed to remove a banner's image: {err:#}");
        }

        activity_logger
            .log(
                "ad-banner:delete",
                serde_json::json!({
                    "uuid": ad_banner.uuid,
                    "advertiser": ad_banner.advertiser,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod patch {
    use super::GetAdBanner;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel, ad_banner::UpdateAdBannerOptions,
            admin_activity::GetAdminActivityLogger, user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        (
            "ad_banner" = uuid::Uuid,
            description = "The banner ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(UpdateAdBannerOptions))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut ad_banner: GetAdBanner,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<UpdateAdBannerOptions>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("ad-banners.update")?;

        ad_banner.update(&state, data).await?;

        activity_logger
            .log(
                "ad-banner:update",
                serde_json::json!({
                    "uuid": ad_banner.uuid,
                    "advertiser": ad_banner.advertiser,
                    "link": ad_banner.link,
                    "placement": ad_banner.placement,
                    "active": ad_banner.active,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod image {
    pub mod put {
        use super::super::GetAdBanner;
        use axum::{body::Bytes, http::StatusCode};
        use serde::Serialize;
        use shared::{
            ApiError, GetState,
            models::{admin_activity::GetAdminActivityLogger, user::GetPermissionManager},
            response::{ApiResponse, ApiResponseResult},
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Response {
            image_url: String,
            width: i32,
            height: i32,
        }

        #[utoipa::path(put, path = "/", responses(
            (status = OK, body = inline(Response)),
            (status = NOT_FOUND, body = ApiError),
            (status = BAD_REQUEST, body = ApiError),
        ), params(
            (
                "ad_banner" = uuid::Uuid,
                description = "The banner ID",
                example = "123e4567-e89b-12d3-a456-426614174000",
            ),
        ), request_body = String)]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
            mut ad_banner: GetAdBanner,
            activity_logger: GetAdminActivityLogger,
            image: Bytes,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("ad-banners.update")?;

            let image_info =
                match shared::models::ad_banner::read_image(&image, ad_banner.placement) {
                    Ok(image_info) => image_info,
                    Err(error) => {
                        return ApiResponse::error(error)
                            .with_status(StatusCode::BAD_REQUEST)
                            .ok();
                    }
                };
            let (width, height) = (image_info.width, image_info.height);

            // a new name each time, so no cache keeps the old picture
            let path = format!(
                "publicdata/ad-banners/{}-{}.{}",
                ad_banner.uuid.simple(),
                uuid::Uuid::new_v4().simple(),
                image_info.extension
            );
            state
                .storage
                .store(&path, &image[..], image_info.content_type)
                .await?;
            let previous = ad_banner.image.clone();
            ad_banner
                .set_image(&state.database, &path, width, height)
                .await?;
            if let Err(err) = state.storage.remove(previous.as_deref()).await {
                tracing::warn!(banner = %ad_banner.uuid, "failed to remove a banner's old image: {err:#}");
            }

            activity_logger
                .log(
                    "ad-banner:update-image",
                    serde_json::json!({
                        "uuid": ad_banner.uuid,
                        "width": width,
                        "height": height,
                    }),
                )
                .await;

            ApiResponse::new_serialized(Response {
                image_url: state.storage.retrieve_urls().await?.get_url(&path),
                width,
                height,
            })
            .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(delete::route))
        .routes(routes!(patch::route))
        .nest(
            "/image",
            OpenApiRouter::new()
                .routes(routes!(image::put::route).layer(DefaultBodyLimit::max(IMAGE_LIMIT)))
                .with_state(state.clone()),
        )
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone())
}
