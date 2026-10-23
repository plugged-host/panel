// plugged.host: the documents of the ad frames and the sponsored banners' click counter (docs/features/ads.md).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod unit {
    pub mod get {
        use axum::{
            body::Body,
            extract::Query,
            http::{HeaderMap, StatusCode},
        };
        use serde::Deserialize;
        use shared::{
            GetState,
            response::{ApiResponse, ApiResponseResult},
        };

        #[derive(Deserialize)]
        pub struct Params {
            /// A banner zone's key.
            zone: Option<compact_str::CompactString>,
            /// The native unit.
            #[serde(default)]
            native: bool,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, description = "The unit's HTML document", body = String, content_type = "text/html"),
            (status = NOT_FOUND),
        ), params(
            (
                "zone" = Option<String>, Query,
                description = "A banner zone's key",
            ),
            (
                "native" = Option<bool>, Query,
                description = "The native unit",
            ),
        ))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            Query(params): Query<Params>,
        ) -> ApiResponseResult {
            let settings = state.settings.get().await?;
            let origin = shared::ads::origin(&settings.app.url);
            // its own origin only on the ad frame address, never on the panel's
            let own_origin = shared::ads::is_frame_host(
                &settings.ads,
                &settings.app.url,
                headers.get("host").and_then(|value| value.to_str().ok()),
            );

            // only what the settings hold: a URL never puts code into the page
            let html = if !settings.ads.enabled {
                None
            } else if params.native {
                shared::ads::native_html(&settings.ads)
            } else {
                params.zone.and_then(|key| {
                    shared::ads::parse_zones(&settings.ads.zones)
                        .unwrap_or_default()
                        .into_iter()
                        .find(|zone| zone.key == key)
                        .map(|zone| shared::ads::banner_html(&zone))
                })
            };
            drop(settings);

            let Some(html) = html else {
                return ApiResponse::error("ad unit not found")
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            };

            ApiResponse::new(Body::from(html))
                .with_header("Content-Type", "text/html; charset=utf-8")
                .with_header(
                    "Content-Security-Policy",
                    shared::ads::unit_csp(origin.as_deref(), own_origin),
                )
                .with_header("Cache-Control", "no-store")
                .with_header("X-Content-Type-Options", "nosniff")
                .ok()
        }
    }
}

mod click {
    pub mod get {
        use axum::{
            body::Body,
            extract::Path,
            http::{HeaderMap, StatusCode},
        };
        use shared::{
            GetState,
            models::{ByUuid, ad_banner::AdBanner},
            response::{ApiResponse, ApiResponseResult},
        };

        #[utoipa::path(get, path = "/", responses(
            (status = FOUND, description = "To the banner's link"),
            (status = NOT_FOUND),
        ), params(
            (
                "banner" = uuid::Uuid,
                description = "The banner ID",
                example = "123e4567-e89b-12d3-a456-426614174000",
            ),
        ))]
        pub async fn route(
            state: GetState,
            headers: HeaderMap,
            Path(banner): Path<uuid::Uuid>,
        ) -> ApiResponseResult {
            // a browser's prefetch is not a click
            let prefetch = ["sec-purpose", "purpose", "x-moz"].iter().any(|name| {
                headers
                    .get(*name)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| value.contains("prefetch"))
            });

            let link = if prefetch {
                AdBanner::by_uuid_optional(&state.database, banner)
                    .await?
                    .map(|banner| banner.link)
            } else {
                AdBanner::click(&state.database, banner).await?
            };

            match link {
                Some(link) => ApiResponse::new(Body::empty())
                    .with_header("Location", link.as_str())
                    .with_header("Cache-Control", "no-store")
                    .with_status(StatusCode::FOUND)
                    .ok(),
                None => ApiResponse::error("banner not found")
                    .with_status(StatusCode::NOT_FOUND)
                    .ok(),
            }
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/unit",
            OpenApiRouter::new()
                .routes(routes!(unit::get::route))
                .with_state(state.clone()),
        )
        .nest(
            "/banners/{banner}/click",
            OpenApiRouter::new()
                .routes(routes!(click::get::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
