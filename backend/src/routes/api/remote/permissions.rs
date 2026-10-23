use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            ByUuid,
            node::GetNode,
            server::Server,
            user::{CredentialScope, PermissionManager, User},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use std::collections::HashMap;
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct PayloadEntry {
        server: uuid::Uuid,
        user: uuid::Uuid,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(length(max = 100))]
        #[schema(inline, max_items = 100)]
        entries: Vec<PayloadEntry>,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseEntry {
        server: uuid::Uuid,
        user: uuid::Uuid,

        permissions: Option<Vec<compact_str::CompactString>>,
        ignored_files: Vec<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        as_of: i64,
        #[schema(inline)]
        entries: Vec<ResponseEntry>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        node: GetNode,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let as_of = chrono::Utc::now().timestamp_millis();
        let settings = state.settings.get().await?;

        let mut users: HashMap<uuid::Uuid, Option<User>> = HashMap::new();
        let mut entries = Vec::with_capacity(data.entries.len());

        for entry in data.entries {
            let user = match users.get(&entry.user) {
                Some(user) => user,
                None => {
                    let user = User::by_uuid_optional_cached(&state.database, entry.user)
                        .await?
                        .filter(|user| user.wings_access_denial(&settings).is_none());

                    users.entry(entry.user).or_insert(user)
                }
            };

            let access = match user {
                Some(user) => Server::by_user_identifier(
                    &state.database,
                    &PermissionManager::new(user, CredentialScope::Session),
                    &entry.server.to_string(),
                )
                .await?
                .filter(|server| server.node.uuid == node.uuid)
                .map(|server| (user, server)),
                None => None,
            };

            entries.push(match access {
                Some((user, server)) => ResponseEntry {
                    server: entry.server,
                    user: entry.user,
                    permissions: Some(
                        server
                            .wings_permissions(&settings, user, &CredentialScope::Session)
                            .into_iter()
                            .map(compact_str::CompactString::from)
                            .collect(),
                    ),
                    ignored_files: server.subuser_ignored_files.unwrap_or_default(),
                },
                None => ResponseEntry {
                    server: entry.server,
                    user: entry.user,
                    permissions: None,
                    ignored_files: Vec::new(),
                },
            });
        }

        ApiResponse::new_serialized(Response { as_of, entries }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
