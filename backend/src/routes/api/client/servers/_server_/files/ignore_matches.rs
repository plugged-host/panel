use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(max = 32), inner(length(chars, min = 1, max = 1024)))]
        #[schema(max_items = 32, min_length = 1, max_length = 1024)]
        patterns: Vec<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        counts: Vec<Option<u64>>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = EXPECTATION_FAILED, body = ApiError),
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
        server: GetServer,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("files.read")?;

        let limit = state
            .settings
            .get()
            .await?
            .server
            .max_file_manager_search_results;

        let response = match server
            .0
            .node
            .fetch_cached(&state.database)
            .await?
            .api_client(&state.database)
            .await?
            .ignoring(server.0.subuser_ignored_files.unwrap_or_default())
            .post_servers_server_files_ignore_matches(
                server.0.uuid,
                &wings_api::servers_server_files_ignore_matches::post::RequestBody {
                    patterns: data.patterns,
                    limit,
                },
            )
            .await
        {
            Ok(data) => data,
            Err(wings_api::client::ApiHttpError::Http(StatusCode::NOT_FOUND, err)) => {
                return ApiResponse::new_serialized(ApiError::new_wings_value(err))
                    .with_status(StatusCode::NOT_FOUND)
                    .ok();
            }
            Err(wings_api::client::ApiHttpError::Http(StatusCode::EXPECTATION_FAILED, err)) => {
                return ApiResponse::new_serialized(ApiError::new_wings_value(err))
                    .with_status(StatusCode::EXPECTATION_FAILED)
                    .ok();
            }
            Err(wings_api::client::ApiHttpError::Http(StatusCode::BAD_REQUEST, err)) => {
                return ApiResponse::new_serialized(ApiError::new_wings_value(err))
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
            Err(err) => return Err(err.into()),
        };

        ApiResponse::new_serialized(Response {
            counts: response.counts,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
