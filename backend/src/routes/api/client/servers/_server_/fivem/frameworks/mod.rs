// plugged.host: one-click frameworks, txAdmin's recipes installed in the background
// (shared/src/fivem/frameworks.rs, docs/features/fivem.md#frameworks).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod install;

mod get {
    use shared::{
        ApiError, GetState,
        fivem::frameworks::{Access, Frameworks},
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = Frameworks),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("fivem.frameworks")?;
        shared::fivem::require(&server)?;

        let access = Access {
            read_databases: permissions.has_server_permission("databases.read").is_ok(),
            create_databases: permissions
                .has_server_permission("databases.create")
                .is_ok(),
            read_license: permissions.has_server_permission("fivem.config").is_ok(),
        };

        ApiResponse::new_serialized(
            shared::fivem::frameworks::overview(&state, &server, access).await?,
        )
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        fivem::frameworks::{DatabaseChoice, Install, Request},
        models::{
            server::{GetServer, GetServerActivityLogger},
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        /// A recipe id from the list.
        #[garde(length(chars, min = 1, max = 64))]
        recipe: String,
        #[garde(length(chars, min = 1, max = 64))]
        server_name: String,
        #[garde(length(chars, min = 1, max = 128))]
        license: String,
        /// For a recipe that needs a database.
        #[garde(skip)]
        database: Option<DatabaseChoice>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        install: Install,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = ACCEPTED, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
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
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("fivem.frameworks")?;
        shared::fivem::require(&server)?;
        if matches!(data.database, Some(DatabaseChoice::Existing { .. })) {
            permissions.has_server_permission("databases.read")?;
        }
        let may_create_database = permissions
            .has_server_permission("databases.create")
            .is_ok();

        let server = server.0;
        let request = Request {
            recipe: data.recipe,
            server_name: data.server_name,
            license: data.license,
            database: data.database,
        };
        let start =
            match shared::fivem::frameworks::start(&state, &server, request, may_create_database)
                .await
            {
                Ok(start) => start,
                Err(err) => return err.response().ok(),
            };
        let install = start.install.clone();

        let logger = activity_logger.0;
        logger
            .log(
                "server:fivem.framework.start",
                serde_json::json!({ "recipe": install.recipe }),
            )
            .await;
        tokio::spawn(shared::fivem::frameworks::install(
            state.0.clone(),
            server,
            start,
            logger,
        ));

        ApiResponse::new_serialized(Response { install })
            .with_status(StatusCode::ACCEPTED)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route, post::route))
        .nest("/install", install::router(state))
        .with_state(state.clone())
}
