// plugged.host: installing one plugin or mod (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        minecraft::Source,
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        source: Source,
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        project: compact_str::CompactString,
        /// Left out: the newest release that fits.
        #[garde(inner(length(chars, min = 1, max = 64)))]
        #[schema(min_length = 1, max_length = 64)]
        version: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        installed: shared::minecraft::content::Installed,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
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
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("minecraft.content")?;
        shared::minecraft::require(&server)?;

        let ratelimit = state
            .settings
            .get_as(|s| s.ratelimits.client_servers_files_pull)
            .await?;
        state
            .cache
            .ratelimit(
                "client/servers/files/pull",
                ratelimit.hits,
                ratelimit.window_seconds,
                server.uuid.to_string(),
            )
            .await?;

        let software = shared::minecraft::software(&state, &server).await?;
        let game_version = software.game_version(&state).await;

        let installed = match shared::minecraft::content::install(
            &state,
            &server,
            &software,
            game_version.as_deref(),
            data.source,
            &data.project,
            data.version.as_deref(),
        )
        .await
        {
            Ok(installed) => installed,
            Err(err) => return err.response().ok(),
        };

        activity_logger
            .log(
                "server:minecraft.content.install",
                serde_json::json!({
                    "source": data.source,
                    "project": data.project,
                    "version": installed.version.id,
                    "file": installed.file,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response { installed })
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
