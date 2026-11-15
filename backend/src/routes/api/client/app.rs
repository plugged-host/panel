//! plugged.host: the Windows app's download and update check (docs/features/app.md).
//!
//! The installer is a file on the box, not an uploaded asset. `PLUGGED_APP_INSTALLER` names it;
//! unset turns the feature off. Every signed-in user may read and download it.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

fn installer() -> Option<(std::path::PathBuf, u64, String)> {
    shared::desktop_app::installer()
        .map(|installer| (installer.path, installer.size, installer.filename))
}

fn version() -> String {
    shared::desktop_app::version()
}

pub mod get {
    use serde::Serialize;
    use shared::{
        GetState,
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
    struct Response {
        app: App,
    }

    #[utoipa::path(get, path = "/", responses((status = OK, body = inline(Response))))]
    pub async fn route(_state: GetState, _user: GetUser) -> ApiResponseResult {
        let (enabled, filename, size) = match super::installer() {
            Some((_path, size, filename)) => (true, filename, size),
            None => (false, String::new(), 0),
        };
        ApiResponse::new_serialized(Response {
            app: App {
                enabled,
                version: super::version(),
                filename,
                size,
                release_notes: std::env::var("PLUGGED_APP_RELEASE_NOTES").ok(),
            },
        })
        .ok()
    }
}

pub mod download {
    use axum::{
        body::Body,
        http::{StatusCode, header},
    };
    use shared::{
        GetState,
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/download", responses((status = OK)))]
    pub async fn route(_state: GetState, _user: GetUser) -> ApiResponseResult {
        let Some((path, size, filename)) = super::installer() else {
            return ApiResponse::error("the desktop app is not available on this panel")
                .with_status(StatusCode::NOT_FOUND)
                .ok();
        };
        // streamed: the installer is ~30 MB and every signed-in user may fetch it
        let file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(err) => {
                tracing::error!(path = %path.display(), "failed to read the app installer: {err}");
                return ApiResponse::error("the installer could not be read")
                    .with_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .ok();
            }
        };
        ApiResponse::new(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
            .with_header(header::CONTENT_LENGTH.as_str(), &size.to_string())
            .with_header(
                header::CONTENT_TYPE.as_str(),
                "application/vnd.microsoft.portable-executable",
            )
            .with_header(
                header::CONTENT_DISPOSITION.as_str(),
                &format!("attachment; filename=\"{filename}\""),
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
