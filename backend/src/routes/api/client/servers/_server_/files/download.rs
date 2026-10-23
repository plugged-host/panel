use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use compact_str::ToCompactString;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        jwt::BasePayload,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::{GetAuthMethod, GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use std::path::{Path, PathBuf};
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        #[serde(default)]
        root: String,
        files: Vec<String>,

        #[serde(default)]
        directory: bool,
        #[serde(default)]
        archive_format: wings_api::StreamableArchiveFormat,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(format = "uri")]
        url: String,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "root" = String, Query,
            description = "The root directory to download from",
            example = "/path/to/root",
        ),
        (
            "files" = Vec<String>, Query,
            description = "The file(s) to download",
            example = "/path/to/file.txt",
        ),
        (
            "directory" = bool, Query,
            description = "Whether the file is a directory",
            example = "false",
        ),
        (
            "archive_format" = wings_api::StreamableArchiveFormat, Query,
            description = "The format of the archive to download",
            example = "tar_gz",
        ),
    ))]
    #[allow(clippy::too_many_arguments)]
    pub async fn route(
        state: GetState,
        request_host: shared::GetRequestHost,
        permissions: GetPermissionManager,
        auth: GetAuthMethod,
        user: GetUser,
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("files.read-content")?;

        for file in &params.files {
            let path = Path::new(&params.root).join(file);
            let ignored = if params.directory {
                server.is_ignored_subtree(&path)
            } else {
                server.is_ignored(&path, false)
            };

            if ignored {
                return ApiResponse::new_serialized(ApiError::new_value(&["file not found"]))
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            }
        }

        let node = server.node.fetch_cached(&state.database).await?;

        let url = if params.files.len() == 1 {
            #[derive(Serialize)]
            struct FileDownloadJwt<'a> {
                #[serde(flatten)]
                base: BasePayload,

                file_path: PathBuf,
                ignored_files: &'a [compact_str::CompactString],
                server_uuid: uuid::Uuid,
                unique_id: uuid::Uuid,
                credential: Option<uuid::Uuid>,
            }

            let token = node
                .create_jwt(
                    &state.database,
                    &state.jwt,
                    &FileDownloadJwt {
                        base: BasePayload {
                            scope: "file-download".into(),
                            issuer: "panel".into(),
                            subject: Some(user.uuid.to_compact_string()),
                            audience: Vec::new(),
                            expiration_time: Some(chrono::Utc::now().timestamp() + 900),
                            not_before: None,
                            issued_at: Some(chrono::Utc::now().timestamp()),
                            jwt_id: uuid::Uuid::new_v4().to_compact_string(),
                        },
                        file_path: Path::new(&params.root).join(&params.files[0]),
                        ignored_files: server.subuser_ignored_files.as_deref().unwrap_or(&[]),
                        server_uuid: server.uuid,
                        unique_id: uuid::Uuid::new_v4(),
                        credential: auth.api_key_uuid(),
                    },
                )
                .await?;

            let mut url = node
                .public_url(
                    &state,
                    request_host.as_deref(),
                    if params.directory {
                        "/download/directory"
                    } else {
                        "/download/file"
                    },
                )
                .await?;
            url.set_query(Some(&format!(
                "token={}&archive_format={}",
                urlencoding::encode(&token),
                params.archive_format
            )));

            url
        } else {
            #[derive(Serialize)]
            struct FilesDownloadJwt<'a> {
                #[serde(flatten)]
                base: BasePayload,

                file_path: &'a str,
                file_paths: &'a [String],
                ignored_files: &'a [compact_str::CompactString],
                server_uuid: uuid::Uuid,
                unique_id: uuid::Uuid,
                credential: Option<uuid::Uuid>,
            }

            let token = node
                .create_jwt(
                    &state.database,
                    &state.jwt,
                    &FilesDownloadJwt {
                        base: BasePayload {
                            scope: "file-download".into(),
                            issuer: "panel".into(),
                            subject: Some(user.uuid.to_compact_string()),
                            audience: Vec::new(),
                            expiration_time: Some(chrono::Utc::now().timestamp() + 900),
                            not_before: None,
                            issued_at: Some(chrono::Utc::now().timestamp()),
                            jwt_id: uuid::Uuid::new_v4().to_compact_string(),
                        },
                        file_path: &params.root,
                        file_paths: &params.files,
                        ignored_files: server.subuser_ignored_files.as_deref().unwrap_or(&[]),
                        server_uuid: server.uuid,
                        unique_id: uuid::Uuid::new_v4(),
                        credential: auth.api_key_uuid(),
                    },
                )
                .await?;

            let mut url = node
                .public_url(&state, request_host.as_deref(), "/download/files")
                .await?;
            url.set_query(Some(&format!(
                "token={}&archive_format={}",
                urlencoding::encode(&token),
                params.archive_format
            )));

            url
        };

        activity_logger
            .log(
                "server:file.download",
                serde_json::json!({
                    "directory": params.root,
                    "files": params.files,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {
            url: url.to_string(),
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
