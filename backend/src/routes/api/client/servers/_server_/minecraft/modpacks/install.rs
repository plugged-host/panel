// plugged.host: installing a modpack, which reinstalls the server for the pack's loader and
// version (docs/features/minecraft.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        minecraft::{Source, modpacks::Install},
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
        #[garde(length(chars, min = 1, max = 64))]
        #[schema(min_length = 1, max_length = 64)]
        version: compact_str::CompactString,
        /// Remove the mods installed now (into the trash when the server has one).
        #[garde(skip)]
        #[serde(default)]
        clean: bool,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = Install),
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
        // a pack brings its own loader and version, which reinstalls the server
        permissions.has_server_permission("settings.software")?;
        shared::minecraft::require(&server)?;

        let start = match shared::minecraft::modpacks::start(
            &state,
            &server,
            data.source,
            &data.project,
            &data.version,
            data.clean,
        )
        .await
        {
            Ok(start) => start,
            Err(err) => return err.response().ok(),
        };
        let started = start.install.clone();

        // A reinstall plus hundreds of downloads outlast a request: the page polls the state.
        let server = server.0;
        tokio::spawn(async move {
            let install = shared::minecraft::modpacks::install(&state, server, start).await;

            activity_logger
                .log(
                    "server:minecraft.modpack.install",
                    serde_json::json!({
                        "source": install.source,
                        "project": install.project,
                        "version": install.version,
                        "name": install.name,
                        "step": install.step,
                        "files": install.files,
                        "skipped": install.skipped.len(),
                        "error": install.error,
                    }),
                )
                .await;
        });

        ApiResponse::new_serialized(started)
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
