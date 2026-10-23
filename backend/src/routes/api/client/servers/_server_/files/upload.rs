use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use compact_str::ToCompactString;
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        jwt::BasePayload,
        models::{
            server::GetServer,
            user::{GetAuthMethod, GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

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
    ))]
    pub async fn route(
        state: GetState,
        request_host: shared::GetRequestHost,
        permissions: GetPermissionManager,
        auth: GetAuthMethod,
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("files.create")?;

        #[derive(Serialize)]
        struct FileUploadJwt<'a> {
            #[serde(flatten)]
            base: BasePayload,

            server_uuid: uuid::Uuid,
            user_uuid: uuid::Uuid,
            user_name: &'a str,
            unique_id: uuid::Uuid,
            credential: Option<uuid::Uuid>,

            ignored_files: &'a [compact_str::CompactString],
        }

        let node = server.node.fetch_cached(&state.database).await?;

        let token = node
            .create_jwt(
                &state.database,
                &state.jwt,
                &FileUploadJwt {
                    base: BasePayload {
                        scope: "file-upload".into(),
                        issuer: "panel".into(),
                        subject: Some(user.uuid.to_compact_string()),
                        audience: Vec::new(),
                        expiration_time: Some(chrono::Utc::now().timestamp() + 900),
                        not_before: None,
                        issued_at: Some(chrono::Utc::now().timestamp()),
                        jwt_id: uuid::Uuid::new_v4().to_compact_string(),
                    },
                    server_uuid: server.uuid,
                    user_uuid: user.uuid,
                    user_name: &user.username,
                    unique_id: uuid::Uuid::new_v4(),
                    credential: auth.api_key_uuid(),
                    ignored_files: server.subuser_ignored_files.as_deref().unwrap_or(&[]),
                },
            )
            .await?;

        let mut url = node
            .public_url(&state, request_host.as_deref(), "/upload/file")
            .await?;
        url.set_query(Some(&format!("token={}", urlencoding::encode(&token))));

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
