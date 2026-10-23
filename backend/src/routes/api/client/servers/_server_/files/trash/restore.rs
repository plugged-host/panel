use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        #[schema(inline)]
        entries: Vec<wings_api::servers_server_files_trash_restore::post::RequestBodyEntries>,

        #[serde(default)]
        overwrite: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        restored: Vec<wings_api::TrashEntry>,
        #[schema(inline)]
        conflicts: Vec<wings_api::servers_server_files_trash_restore::post::Response200Conflicts>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        mut server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("files.delete")?;

        let request_body = wings_api::servers_server_files_trash_restore::post::RequestBody {
            entries: data
                .entries
                .into_iter()
                .filter(|e| {
                    e.path
                        .as_ref()
                        .is_none_or(|path| !server.is_ignored_either(path))
                })
                .collect(),
            overwrite: data.overwrite,
        };

        tokio::spawn(async move {
            let data = match server
                .0
                .node
                .fetch_cached(&state.database)
                .await?
                .api_client(&state.database)
                .await?
                .ignoring(server.0.subuser_ignored_files.unwrap_or_default())
                .post_servers_server_files_trash_restore(server.0.uuid, &request_body)
                .await
            {
                Ok(data) => data,
                Err(wings_api::client::ApiHttpError::Http(StatusCode::NOT_FOUND, err)) => {
                    return ApiResponse::new_serialized(ApiError::new_wings_value(err))
                        .with_status(StatusCode::NOT_FOUND)
                        .ok();
                }
                Err(err) => return Err(err.into()),
            };

            if !data.restored.is_empty() {
                activity_logger
                    .log(
                        "server:file.trash.restore",
                        serde_json::json!({
                            "files": data.restored.iter().map(|e| &e.path).collect::<Vec<_>>(),
                        }),
                    )
                    .await;
            }

            ApiResponse::new_serialized(Response {
                restored: data.restored,
                conflicts: data.conflicts,
            })
            .ok()
        })
        .await?
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
