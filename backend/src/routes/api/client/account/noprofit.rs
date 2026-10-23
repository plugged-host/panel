//! plugged.host: the noprofit.space account the signed-in user came from (`shared::noprofit`).

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetUser,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ResponseAccount {
        account_id: i64,
        email: String,
        username: String,
        is_admin: bool,
        credits: i64,
        #[schema(value_type = Option<Object>)]
        plan: Option<serde_json::Value>,
        #[schema(value_type = Option<Object>)]
        discord: Option<serde_json::Value>,
        synced: chrono::NaiveDateTime,
    }

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        accounts_url: Option<&'a str>,
        #[schema(inline)]
        account: Option<ResponseAccount>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, user: GetUser) -> ApiResponseResult {
        let Some(noprofit) = state.settings.get().await?.noprofit.config() else {
            return ApiResponse::new_serialized(Response {
                accounts_url: None,
                account: None,
            })
            .ok();
        };

        let account = shared::noprofit::Link::by_user_uuid(&state.database, user.uuid)
            .await?
            .map(|link| ResponseAccount {
                account_id: link.account_id,
                email: link.email,
                username: link.username,
                is_admin: link.is_admin,
                credits: link.credits,
                plan: link.plan,
                discord: link.discord,
                synced: link.synced,
            });

        ApiResponse::new_serialized(Response {
            accounts_url: Some(noprofit.accounts_url.as_str()),
            account,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
