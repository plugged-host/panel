//! plugged.host: neo's `list_servers`, the one tool of the account chat that looks at servers
//! (docs/features/neo.md › Tools). Working on a server happens in that server's own chat.

use super::{Tool, ToolContext, ToolOutcome, ToolResult, ToolScope, support};
use crate::models::server::Server;
use serde_json::{Value, json};
use std::{borrow::Cow, collections::HashMap};

const LISTED: i64 = 25;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub struct ListServersTool;

/// The live states of the servers' nodes, one request per node; a node that does not answer
/// leaves its servers without a state.
async fn states(
    state: &crate::State,
    servers: &[Server],
) -> HashMap<uuid::Uuid, (String, Option<wings_api::ServerState>)> {
    let mut nodes: HashMap<uuid::Uuid, Vec<&Server>> = HashMap::new();
    for server in servers {
        nodes.entry(server.node.uuid).or_default().push(server);
    }

    let mut states = HashMap::new();
    for servers in nodes.into_values() {
        let node = match servers[0].node.fetch_cached(&state.database).await {
            Ok(node) => node,
            Err(err) => {
                tracing::debug!("neo could not load a node: {err:#}");
                continue;
            }
        };
        let resources = tokio::time::timeout(TIMEOUT, node.fetch_server_resources(&state.database))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();

        for server in servers {
            states.insert(
                server.uuid,
                (
                    node.name.to_string(),
                    resources.get(&server.uuid).map(|usage| usage.state),
                ),
            );
        }
    }

    states
}

#[async_trait::async_trait]
impl Tool for ListServersTool {
    fn name(&self) -> &'static str {
        "list_servers"
    }

    fn description(&self) -> Cow<'static, str> {
        "List the servers this user owns or has access to, with their game, node and whether they are running. To work on a server, the user has to open neo from that server's pages.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn scope(&self) -> ToolScope {
        ToolScope::App
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "List your servers".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let servers = Server::by_user_uuid_with_pagination(
            &context.state.database,
            context.user.uuid,
            1,
            LISTED,
            None,
        )
        .await?;
        if servers.data.is_empty() {
            return Ok(ToolResult::ok("This user has no servers yet.").with_display("No servers"));
        }

        let states = states(context.state, &servers.data).await;
        let mut lines: Vec<String> = servers
            .data
            .iter()
            .map(|server| {
                let mut role = if server.owner.uuid == context.user.uuid {
                    "owner".to_string()
                } else {
                    "subuser".to_string()
                };
                if !super::may_use_on(context.user, server) {
                    role.push_str(", neo not enabled for you here");
                }
                let (node, state) = states
                    .get(&server.uuid)
                    .map(|(node, state)| {
                        (node.as_str(), state.map_or("unknown", support::state_name))
                    })
                    .unwrap_or(("?", "unknown"));

                format!(
                    "- {} (id {:08x}, {role}): {} on {node}, {state}{}",
                    server.name,
                    server.uuid_short,
                    server.egg.name,
                    if server.suspended { ", suspended" } else { "" }
                )
            })
            .collect();
        if servers.total > servers.data.len() as i64 {
            lines.push(format!(
                "…and {} more servers",
                servers.total - servers.data.len() as i64
            ));
        }

        Ok(ToolResult::ok(lines.join("\n"))
            .with_display(format!("Listed {} servers", servers.data.len())))
    }
}
