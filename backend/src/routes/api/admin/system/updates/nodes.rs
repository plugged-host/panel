use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{extract::Query, http::StatusCode};
    use futures_util::{StreamExt, TryStreamExt};
    use serde::Serialize;
    use shared::{
        ApiError, GetState,
        models::{
            ByUuid, IntoAdminApiObject, Pagination, PaginationParams, node::Node,
            user::GetPermissionManager,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ResponseNode {
        #[serde(flatten)]
        #[schema(inline)]
        status: shared::updates::UpgradeStatus,
        node: shared::models::node::AdminApiNode,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        outdated_nodes: Pagination<ResponseNode>,
        failed_nodes: usize,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ), params(
        (
            "page" = i64, Query,
            description = "The page number",
            example = "1",
        ),
        (
            "per_page" = i64, Query,
            description = "The number of items per page",
            example = "10",
        ),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        Query(params): Query<PaginationParams>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&params) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("nodes.read")?;

        let token = state
            .settings
            .get()
            .await?
            .updates
            .wings_release_token_for(&permissions);
        let cache_key = if token.is_some() {
            "nodes::versions::token"
        } else {
            "nodes::versions"
        };

        let (node_versions, failed_nodes) = state
            .cache
            .cached(cache_key, 30, || async {
                let mut versions = Vec::new();
                let mut failed_nodes = 0;

                let mut node_page = 1;
                loop {
                    let nodes =
                        Node::all_with_pagination(&state.database, node_page, 50, None).await?;
                    if nodes.data.is_empty() {
                        break;
                    }

                    let mut versions_futures = Vec::new();
                    for node in &nodes.data {
                        let token = token.clone();
                        let state = &state;
                        versions_futures.push(async move {
                            Ok::<_, anyhow::Error>((
                                node.uuid,
                                node.upgrade_status(state, token).await?,
                            ))
                        });
                    }

                    let mut futures_stream =
                        futures_util::stream::iter(versions_futures).buffer_unordered(10);

                    while let Some(result) = futures_stream.next().await {
                        match result {
                            Ok((node_uuid, status)) => {
                                let Some(created) = nodes
                                    .data
                                    .iter()
                                    .find(|n| n.uuid == node_uuid)
                                    .map(|n| n.created)
                                else {
                                    continue;
                                };

                                versions.push((node_uuid, created, status));
                            }
                            Err(err) => {
                                tracing::warn!(
                                    "failed to get system overview for a node: {:#?}",
                                    err
                                );
                                failed_nodes += 1;
                            }
                        }
                    }

                    node_page += 1;
                }

                versions.sort_unstable_by_key(|a| a.1);

                Ok::<_, anyhow::Error>((versions, failed_nodes))
            })
            .await?;

        let mut outdated_node_uuids = Vec::new();
        for (node_uuid, _, status) in node_versions {
            if status.update_available {
                outdated_node_uuids.push((node_uuid, status));
            }
        }

        let mut outdated_nodes = Pagination {
            total: outdated_node_uuids.len() as i64,
            per_page: params.per_page,
            page: params.page,
            data: Vec::new(),
        };

        let mut node_futures = Vec::new();

        for (node_uuid, _) in outdated_node_uuids
            .iter()
            .skip(((params.page - 1) * params.per_page) as usize)
            .take(params.per_page as usize)
        {
            node_futures.push(async {
                let node = Node::by_uuid_cached(&state.database, *node_uuid).await?;
                node.into_admin_api_object(&state, ()).await
            });
        }

        let mut futures_stream = futures_util::stream::iter(node_futures).buffered(10);

        while let Some(node) = futures_stream.try_next().await? {
            let status = match outdated_node_uuids
                .iter()
                .find(|(uuid, _)| *uuid == node.uuid)
            {
                Some((_, status)) => status.clone(),
                None => continue,
            };

            outdated_nodes.data.push(ResponseNode { status, node });
        }

        ApiResponse::new_serialized(Response {
            outdated_nodes,
            failed_nodes,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
