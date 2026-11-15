//! plugged.host: the Windows app's download and update check (docs/features/app.md).
//!
//! The installer is a file on the box, not an uploaded asset. `PLUGGED_APP_INSTALLER` names it;
//! unset turns the feature off. Every signed-in user may read and download it.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

fn installer_path() -> Option<std::path::PathBuf> {
    std::env::var("PLUGGED_APP_INSTALLER")
        .ok()
        .map(|path| path.trim().to_owned())
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
}

fn version() -> String {
    std::env::var("PLUGGED_APP_VERSION")
        .ok()
        .map(|version| version.trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "3.1.0".into())
}

fn installer() -> Option<(std::path::PathBuf, u64, String)> {
    let path = installer_path()?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Plugged-Setup.exe")
        .to_owned();
    Some((path, metadata.len(), filename))
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
