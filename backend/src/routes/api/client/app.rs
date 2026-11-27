//! plugged.host: the desktop app's download and update check (docs/features/app.md).
//!
//! The installers are files on the box, not uploaded assets. `PLUGGED_APP_INSTALLER` names the
//! Windows installer, `PLUGGED_APP_LINUX_APPIMAGE` and `PLUGGED_APP_LINUX_DEB` the Linux builds;
//! an unset one is not offered. Every signed-in user may read and download them.

use super::State;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, IntoParams)]
pub struct PlatformParams {
    /// `windows` (the default, what the Windows app asks for), `linux` (the AppImage) or `linux-deb`
    platform: Option<String>,
}

fn version() -> String {
    shared::desktop_app::version()
}

pub mod get {
    use axum::{extract::Query, http::StatusCode};
    use serde::Serialize;
    use shared::{
        GetState,
        desktop_app::{Platform, installer_for},
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct App {
        enabled: bool,
        version: String,
        filename: String,
        size: u64,
        release_notes: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Download {
        platform: &'static str,
        filename: String,
        size: u64,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// The file for the asked platform (the Windows installer by default)
        app: App,
        /// Every file this panel offers, for the download page
        downloads: Vec<Download>,
    }

    #[utoipa::path(get, path = "/", params(super::PlatformParams), responses((status = OK, body = inline(Response))))]
    pub async fn route(
        _state: GetState,
        _user: GetUser,
        Query(params): Query<super::PlatformParams>,
    ) -> ApiResponseResult {
        let Some(platform) = Platform::parse(params.platform.as_deref()) else {
            return ApiResponse::error("unknown platform (windows, linux or linux-deb)")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        };
        let (enabled, filename, size) = match installer_for(platform) {
            Some(installer) => (true, installer.filename, installer.size),
            None => (false, String::new(), 0),
        };
        let downloads = Platform::ALL
            .iter()
            .filter_map(|platform| {
                installer_for(*platform).map(|installer| Download {
                    platform: platform.key(),
                    filename: installer.filename,
                    size: installer.size,
                })
            })
            .collect();
        ApiResponse::new_serialized(Response {
            app: App {
                enabled,
                version: super::version(),
                filename,
                size,
                release_notes: std::env::var("PLUGGED_APP_RELEASE_NOTES").ok(),
            },
            downloads,
        })
        .ok()
    }
}

pub mod download {
    use axum::{
        body::Body,
        extract::Query,
        http::{StatusCode, header},
    };
    use shared::{
        GetState,
        desktop_app::{Platform, installer_for},
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/download", params(super::PlatformParams), responses((status = OK)))]
    pub async fn route(
        _state: GetState,
        _user: GetUser,
        Query(params): Query<super::PlatformParams>,
    ) -> ApiResponseResult {
        let Some(platform) = Platform::parse(params.platform.as_deref()) else {
            return ApiResponse::error("unknown platform (windows, linux or linux-deb)")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        };
        let Some(installer) = installer_for(platform) else {
            return ApiResponse::error("the desktop app is not available on this panel")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };
        // streamed: the files are ~30 MB and every signed-in user may fetch them
        let file = match tokio::fs::File::open(&installer.path).await {
            Ok(file) => file,
            Err(err) => {
                tracing::error!(path = %installer.path.display(), "failed to read the app installer: {err}");
                return ApiResponse::error("the installer could not be read")
                    .with_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .ok();
            }
        };
        ApiResponse::new(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
            .with_header(header::CONTENT_LENGTH.as_str(), &installer.size.to_string())
            .with_header(header::CONTENT_TYPE.as_str(), platform.content_type())
            .with_header(
                header::CONTENT_DISPOSITION.as_str(),
                &format!("attachment; filename=\"{}\"", installer.filename),
            )
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(download::route))
        .with_state(state.clone())
}
