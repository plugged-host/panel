// plugged.host: what a user buys with tokens for the server: a keep-awake pass, a boost, extra
// slots, a card colour (shared/src/server_upgrades.rs, docs/features/server-upgrades.md).
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod color;

mod get {
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            server::GetServer,
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_upgrades::ApiUpgrades,
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        upgrades: ApiUpgrades,
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
        permissions: GetPermissionManager,
        user: GetUser,
        server: GetServer,
    ) -> ApiResponseResult {
        permissions.has_server_permission("settings.upgrades")?;

        ApiResponse::new_serialized(Response {
            upgrades: shared::server_upgrades::view(&state, &server, &user).await?,
        })
        .ok()
    }
}

mod post {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            ByUuid,
            server::{GetServer, GetServerActivityLogger, Server},
            user::{GetPermissionManager, GetUser},
        },
        response::{ApiResponse, ApiResponseResult},
        server_upgrades::{ApiUpgrades, Kind, PurchaseRequest, UpgradeError},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        kind: Kind,
        /// The extra port, database or backup slot to renew; a new one when left out.
        #[garde(skip)]
        renew: Option<uuid::Uuid>,
        /// The price the user saw; a price that moved buys nothing (409).
        #[garde(range(min = 0))]
        tokens: i64,
        /// The card colour, one of the palette.
        #[garde(inner(length(chars, max = 16)))]
        color: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        upgrades: ApiUpgrades,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, body = ApiError),
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
        user: GetUser,
        server: GetServer,
        activity_logger: GetServerActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_server_permission("settings.upgrades")?;

        let bought = match shared::server_upgrades::buy(
            &state,
            &server,
            &user,
            PurchaseRequest {
                kind: data.kind,
                renew: data.renew,
                tokens: data.tokens,
                color: data.color,
            },
        )
        .await
        {
            Ok(bought) => bought,
            Err(UpgradeError::Other(err)) => return Err(err.into()),
            Err(err) => {
                return ApiResponse::error(&err.message())
                    .with_status(
                        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::BAD_REQUEST),
                    )
                    .ok();
            }
        };

        activity_logger
            .log(
                if bought.renewed {
                    "server:upgrade.renew"
                } else {
                    "server:upgrade.buy"
                },
                serde_json::json!({
                    "kind": bought.kind,
                    "upgrade": bought.uuid,
                    "tokens": bought.tokens,
                    "until": bought.until.map(|until| until.and_utc()),
                }),
            )
            .await;

        let server = Server::by_uuid(&state.database, server.uuid).await?;

        ApiResponse::new_serialized(Response {
            upgrades: shared::server_upgrades::view(&state, &server, &user).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(post::route))
        .nest("/color", color::router(state))
        .with_state(state.clone())
}
