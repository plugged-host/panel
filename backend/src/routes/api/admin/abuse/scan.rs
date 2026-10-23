//! plugged.host: an admin running the full abuse check on one server now.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{
            ByUuid, admin_activity::GetAdminActivityLogger, server::Server,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    /// How long the request waits for the scan; a slower scan finishes in the background.
    const WAIT: std::time::Duration = std::time::Duration::from_secs(45);

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        server_uuid: uuid::Uuid,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseFinding {
        rule: String,
        category: String,
        severity: shared::abuse::Severity,
        summary: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        /// False when the scan took too long to wait for; it still records what it finds.
        finished: bool,
        /// Whether the server's node answered (its files and console were checked).
        reachable: bool,
        findings: Vec<ResponseFinding>,
    }

    /// Records what it finds like the scheduled scan, without suspending: the admin decides.
    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = NOT_FOUND, body = shared::ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        permissions.has_admin_permission("abuse.manage")?;

        let Some(server) = Server::by_uuid_optional(&state.database, data.server_uuid).await?
        else {
            return super::super::not_found("server not found");
        };

        activity_logger
            .log(
                "abuse:scan",
                serde_json::json!({ "server_uuid": server.uuid }),
            )
            .await;

        let scan = tokio::spawn({
            let state = state.0.clone();
            async move { shared::abuse::scan_server(&state, &server, false, false).await }
        });

        let response = match tokio::time::timeout(WAIT, scan).await {
            Ok(result) => {
                let result = result??;
                Response {
                    finished: true,
                    reachable: result.reachable,
                    findings: result
                        .findings
                        .into_iter()
                        .map(|finding| ResponseFinding {
                            rule: finding.rule,
                            category: finding.category,
                            severity: finding.severity,
                            summary: finding.summary,
                        })
                        .collect(),
                }
            }
            Err(_) => Response {
                finished: false,
                reachable: true,
                findings: Vec::new(),
            },
        };

        ApiResponse::new_serialized(response).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
