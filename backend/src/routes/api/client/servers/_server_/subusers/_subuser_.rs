use super::State;
use axum::{
    extract::{Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use shared::{
    GetState,
    models::{server::GetServer, server_subuser::ServerSubuser, user::GetPermissionManager},
    response::ApiResponse,
};
use utoipa_axum::{router::OpenApiRouter, routes};

pub type GetServerSubuser = shared::extract::ConsumingExtension<ServerSubuser>;

pub async fn auth(
    state: GetState,
    permissions: GetPermissionManager,
    server: GetServer,
    Path(subuser): Path<Vec<String>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let subuser = match subuser.get(1).map(|s| s.parse::<uuid::Uuid>()) {
        Some(Ok(id)) => id,
        _ => {
            return Ok(ApiResponse::error("invalid subuser uuid")
                .with_status(StatusCode::BAD_REQUEST)
                .into_response());
        }
    };

    if let Err(err) = permissions.has_server_permission("subusers.read") {
        return Ok(err.into_response());
    }

    let subuser =
        ServerSubuser::by_server_uuid_user_uuid(&state.database, server.uuid, subuser).await;
    let subuser = match subuser {
        Ok(Some(subuser)) => subuser,
        Ok(None) => {
            return Ok(ApiResponse::error("subuser not found")
                .with_status(StatusCode::NOT_FOUND)
                .into_response());
        }
        Err(err) => return Ok(ApiResponse::from(err).into_response()),
    };

    req.extensions_mut().insert(server.0);
    req.extensions_mut().insert(subuser);

    Ok(next.run(req).await)
}

mod delete {
    use crate::routes::api::client::servers::_server_::subusers::_subuser_::GetServerSubuser;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{DeletableModel, server::GetServerActivityLogger, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(delete, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "subuser" = uuid::Uuid,
            description = "The subuser ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetServerActivityLogger,
        subuser: GetServerSubuser,
    ) -> ApiResponseResult {
        permissions.has_server_permission("subusers.delete")?;

        subuser.delete(&state, ()).await?;

        activity_logger
            .log(
                "server:subuser.delete",
                serde_json::json!({
                    "username": subuser.user.username,
                    "email": subuser.user.email,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

mod patch {
    use crate::routes::api::client::servers::_server_::subusers::_subuser_::GetServerSubuser;
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            UpdatableModel,
            server::{GetServer, GetServerActivityLogger},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(inner(custom(shared::permissions::validate_server_permissions)))]
        permissions: Option<Vec<compact_str::CompactString>>,
        #[garde(inner(
            length(max = 1024),
            inner(length(chars, min = 1, max = 255)),
            custom(shared::utils::validate_ignored_files)
        ))]
        #[schema(max_items = 1024, min_length = 1, max_length = 255)]
        ignored_files: Option<Vec<compact_str::CompactString>>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(patch, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_REQUEST, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        (
            "subuser" = uuid::Uuid,
            description = "The subuser ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        mut subuser: GetServerSubuser,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        if permissions
            .missing_server_permission(data.permissions.iter().flatten())
            .is_some()
        {
            return ApiResponse::error("permissions: more permissions than self")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        if let Some(subuser_ignored_files) = &server.subuser_ignored_files
            && !data
                .ignored_files
                .as_ref()
                .unwrap_or(&subuser.ignored_files)
                .ends_with(subuser_ignored_files)
        {
            return ApiResponse::error("ignored_files: less restrictive than self")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("subusers.update")?;

        if subuser.user.uuid == user.uuid {
            return ApiResponse::error("cannot update permissions for self")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        subuser
            .update(
                &state,
                shared::models::server_subuser::UpdateServerSubuserOptions {
                    permissions: data.permissions,
                    ignored_files: data.ignored_files,
                },
            )
            .await?;

        activity_logger
            .log(
                "server:subuser.update",
                serde_json::json!({
                    "username": subuser.user.username,
                    "email": subuser.user.email,
                    "permissions": subuser.permissions,
                    "ignored_files": subuser.ignored_files,
                }),
            )
            .await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(delete::route))
        .routes(routes!(patch::route))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone())
}
