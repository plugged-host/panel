//! plugged.host: neo's tools for a server's state, console and power (docs/features/neo.md › Tools).

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, support};
use crate::models::server::{Server, ServerStatus};
use serde_json::{Value, json};
use std::borrow::Cow;
use tokio::io::AsyncReadExt;

/// Wings keeps a bounded console buffer; more lines than this are rarely there.
pub const MAX_LINES: i64 = 100;
const DEFAULT_LINES: i64 = 80;
const CONSOLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const CONSOLE_MAX_BYTES: u64 = 1024 * 1024;

/// Why the chat's server cannot be acted on right now, `None` when it can: the rules the agent
/// applies before every server tool but `server_status` (through `agent::server_blocked`), which
/// `server_status` reports so the model knows why the other tools refuse.
pub async fn blocked(state: &crate::State, server: &Server) -> Option<&'static str> {
    if server.suspended {
        return Some("the server is suspended");
    }
    match server.status {
        Some(ServerStatus::Installing) => return Some("the server is still installing"),
        Some(ServerStatus::InstallFailed) => return Some("the server's installation failed"),
        Some(ServerStatus::RestoringBackup) => return Some("the server is restoring a backup"),
        Some(ServerStatus::BackupRestoreFailed) => {
            return Some("the server's last backup restore failed");
        }
        None => {}
    }
    if server.destination_node.is_some() {
        return Some("the server is being moved to another node");
    }
    if server
        .limbo
        .as_ref()
        .is_some_and(|limbo| !matches!(limbo, crate::limbo::LimboState::Stopped))
    {
        return Some("the server is hibernated; the owner restores it from its page");
    }

    match server.node.fetch_cached(&state.database).await {
        Ok(node) if node.maintenance_enabled => Some("the node is under maintenance"),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!(server = %server.uuid, "neo could not read the server's node: {err:#}");
            None
        }
    }
}

/// The console's last `lines` lines from Wings, without colour codes and with secrets masked.
pub async fn tail(
    state: &crate::State,
    server: &Server,
    lines: i64,
) -> Result<Vec<String>, &'static str> {
    let lines = lines.clamp(1, MAX_LINES) as usize;

    let read = tokio::time::timeout(CONSOLE_TIMEOUT, async {
        let reader = server
            .node
            .fetch_cached(&state.database)
            .await?
            .api_client(&state.database)
            .await?
            .get_servers_server_logs(
                server.uuid,
                &wings_api::servers_server_logs::get::Query {
                    lines: Some(lines as u64),
                    ..Default::default()
                },
            )
            .await?;

        let mut content = Vec::new();
        reader
            .take(CONSOLE_MAX_BYTES)
            .read_to_end(&mut content)
            .await?;

        Ok::<_, anyhow::Error>(content)
    })
    .await;

    let content = match read {
        Ok(Ok(content)) => content,
        Ok(Err(err)) => {
            tracing::debug!(server = %server.uuid, "neo could not read the console: {err:#}");
            return Err("the node could not be reached");
        }
        Err(_) => return Err("the node did not answer in time"),
    };

    Ok(clean_lines(&String::from_utf8_lossy(&content), lines))
}

/// Console text as lines: colour codes stripped, secrets masked, at most the last `keep`.
fn clean_lines(text: &str, keep: usize) -> Vec<String> {
    let text = crate::redact::redact(&crate::redact::strip_ansi(text));
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    let skip = lines.len().saturating_sub(keep);

    lines.into_iter().skip(skip).collect()
}

/// The server's live state on Wings, `None` when Wings does not answer.
async fn live(state: &crate::State, server: &Server) -> Option<wings_api::ResourceUsage> {
    let fetch = async {
        server
            .node
            .fetch_cached(&state.database)
            .await?
            .fetch_server_resources(&state.database)
            .await
    };

    match tokio::time::timeout(CONSOLE_TIMEOUT, fetch).await {
        Ok(Ok(mut resources)) => resources.remove(&server.uuid),
        _ => None,
    }
}

pub struct ServerStatusTool;

#[async_trait::async_trait]
impl Tool for ServerStatusTool {
    fn name(&self) -> &'static str {
        "server_status"
    }

    fn description(&self) -> Cow<'static, str> {
        "Live state of this server: running/stopped, CPU, memory and disk use against its limits, uptime, its address and game, and whether something (a suspension, an install, a move) blocks working on it. Use it first when something seems wrong.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Check server status".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let node = server.node.fetch_cached(&context.state.database).await?;
        let usage = live(context.state, server).await;

        let state = usage
            .as_ref()
            .map_or("unknown (the node did not answer)", |usage| {
                support::state_name(usage.state)
            });
        let address = server.allocation.as_ref().map_or_else(
            || "none".to_string(),
            |allocation| {
                let host = allocation.allocation.ip_alias.as_ref().map_or_else(
                    || allocation.allocation.ip.ip().to_string(),
                    |alias| alias.to_string(),
                );
                format!("{host}:{}", allocation.allocation.port)
            },
        );
        let limit = |value: i64, unit: &str| {
            if value > 0 {
                format!("{value}{unit}")
            } else {
                "unlimited".to_string()
            }
        };

        let lines = [
            format!("name: {}", server.name),
            format!("state: {state}"),
            format!(
                "blocked: {}",
                blocked(context.state, server).await.unwrap_or("no")
            ),
            format!("game: {}", server.egg.name),
            format!("node: {}", node.name),
            format!("address: {address}"),
            format!(
                "cpu: {}% of {}",
                usage
                    .as_ref()
                    .map_or(0.0, |usage| (usage.cpu_absolute * 10.0).round() / 10.0),
                limit(server.cpu as i64, "%")
            ),
            format!(
                "memory: {} of {}",
                support::human(usage.as_ref().map_or(0, |usage| usage.memory_bytes)),
                limit(server.memory, " MB")
            ),
            format!(
                "disk: {} of {}",
                support::human(usage.as_ref().map_or(0, |usage| usage.disk_bytes)),
                limit(server.disk, " MB")
            ),
            format!(
                "uptime_seconds: {}",
                usage.as_ref().map_or(0, |usage| usage.uptime / 1000)
            ),
        ];

        Ok(ToolResult::ok(lines.join("\n")).with_display(format!("Server is {state}")))
    }
}

pub struct ConsoleReadTool;

#[async_trait::async_trait]
impl Tool for ConsoleReadTool {
    fn name(&self) -> &'static str {
        "console_read"
    }

    fn description(&self) -> Cow<'static, str> {
        "Read the most recent lines of the server console (up to 100). Use it to diagnose crashes, errors and startup problems. Secrets such as passwords and tokens are masked as ••••.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "lines": {"type": "integer", "minimum": 1, "maximum": MAX_LINES, "description": "How many recent lines to read (default 80)."},
            },
        })
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["control.read-console"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Read the console".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let lines = support::integer(arguments, "lines", DEFAULT_LINES, 1, MAX_LINES);

        let lines = match tail(context.state, server, lines).await {
            Ok(lines) => lines,
            Err(reason) => {
                return Ok(ToolResult::error(format!(
                    "Could not read the console: {reason}."
                )));
            }
        };

        if lines.iter().all(|line| line.trim().is_empty()) {
            return Ok(ToolResult::ok(
                "The console is empty (the server may not have been started since the node restarted).",
            )
            .with_display("Console is empty"));
        }

        Ok(ToolResult::ok(support::limit_tail(&lines.join("\n")))
            .with_display(format!("Read {} console lines", lines.len())))
    }
}

pub struct SendCommandTool;

#[async_trait::async_trait]
impl Tool for SendCommandTool {
    fn name(&self) -> &'static str {
        "send_command"
    }

    fn description(&self) -> Cow<'static, str> {
        "Run one command in the server console (without a leading slash for Minecraft), then return the console lines that followed. The server must be running.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "A single console command, e.g. \"list\" or \"say Restarting in 5 minutes\"."},
            },
            "required": ["command"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["control.console"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Run: {}",
            support::short(support::shown(arguments, "command"), 80)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let command = support::string(arguments, "command", 1024, true)?.trim();
        if command.is_empty() {
            return Err(ToolError::invalid(
                "\"command\" must be a non-empty string.",
            ));
        }
        if command.chars().any(char::is_control) {
            return Err(ToolError::invalid(
                "Send one command at a time, without line breaks.",
            ));
        }

        let client = support::wings(context).await?;
        match crate::minecraft::files::power_state(&client, server).await {
            Some(wings_api::ServerState::Running | wings_api::ServerState::Starting) => {}
            Some(state) => {
                return Ok(ToolResult::error(format!(
                    "The server is {}; start it before sending console commands.",
                    support::state_name(state)
                )));
            }
            None => {
                return Ok(ToolResult::error(
                    "The node did not answer; the command was not sent.",
                ));
            }
        }

        client
            .post_servers_server_commands(
                server.uuid,
                &wings_api::servers_server_commands::post::RequestBody {
                    commands: vec![command.into()],
                },
            )
            .await
            .map_err(support::wings_error)?;
        context.log("command", json!({"command": command})).await;

        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let output = match tail(context.state, server, 30).await {
            Ok(lines) => lines.join("\n"),
            Err(_) => "(console output unavailable)".to_string(),
        };

        Ok(ToolResult::ok(support::limit_output(&format!(
            "Sent: {command}\nConsole afterwards:\n{output}"
        )))
        .with_display(format!("Ran: {}", support::short(command, 60))))
    }
}

pub struct PowerTool;

const ACTIONS: [&str; 4] = ["start", "stop", "restart", "kill"];

fn action(value: &str) -> Option<wings_api::ServerPowerAction> {
    match value {
        "start" => Some(wings_api::ServerPowerAction::Start),
        "stop" => Some(wings_api::ServerPowerAction::Stop),
        "restart" => Some(wings_api::ServerPowerAction::Restart),
        "kill" => Some(wings_api::ServerPowerAction::Kill),
        _ => None,
    }
}

#[async_trait::async_trait]
impl Tool for PowerTool {
    fn name(&self) -> &'static str {
        "power"
    }

    fn description(&self) -> Cow<'static, str> {
        "Start, stop, restart or kill the server. Prefer stop over kill; kill forces the process down and can lose unsaved data. A start may be routed through the start queue, the waiting room or a hibernation restore; then the user is taken to that page.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ACTIONS},
            },
            "required": ["action"],
        })
    }

    fn risk(&self, arguments: &Value) -> ToolRisk {
        if support::shown(arguments, "action") == "kill" {
            ToolRisk::Destructive
        } else {
            ToolRisk::Write
        }
    }

    fn offered_with(&self) -> Vec<&'static str> {
        vec!["control.start", "control.stop", "control.restart"]
    }

    fn requires(&self, arguments: &Value) -> Vec<&'static str> {
        match support::shown(arguments, "action") {
            "start" => vec!["control.start"],
            "restart" => vec!["control.restart"],
            _ => vec!["control.stop"],
        }
    }

    fn summarize(&self, arguments: &Value) -> String {
        match support::shown(arguments, "action") {
            "start" => "Start the server".into(),
            "stop" => "Stop the server".into(),
            "restart" => "Restart the server".into(),
            "kill" => "Kill the server".into(),
            _ => "Power the server".into(),
        }
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let name = support::shown(arguments, "action");
        let Some(power) = action(name) else {
            return Err(ToolError::invalid(
                "\"action\" must be one of: start, stop, restart, kill.",
            ));
        };

        if let Some(held) =
            crate::start_gates::run(context.state, server, context.user, power).await
        {
            context
                .log("power", json!({"action": name, "gate": held.gate}))
                .await;

            return Ok(ToolResult::redirect(
                format!(
                    "The {name} was handed to the panel's start flow ({}). The user is being taken to that page now; tell them it continues there.",
                    held.gate
                ),
                held.url,
            ));
        }

        support::wings(context)
            .await?
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: power,
                    wait_seconds: None,
                },
            )
            .await
            .map_err(support::wings_error)?;
        context.log("power", json!({"action": name})).await;

        Ok(ToolResult::ok(format!(
            "Sent \"{name}\" to the server. It can take a moment; check the console or the status to confirm."
        ))
        .with_display(format!("Sent {name}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_lines_are_cleaned() {
        let text = "\x1b[32m[INFO]\x1b[0m Done\r\nrcon.password=hunter2\nlast  \n";
        assert_eq!(
            clean_lines(text, 10),
            vec!["[INFO] Done", "rcon.password=••••", "last"]
        );
        assert_eq!(clean_lines("a\nb\nc", 2), vec!["b", "c"]);
    }

    #[test]
    fn power_by_action() {
        let tool = PowerTool;
        assert_eq!(tool.risk(&json!({"action": "kill"})), ToolRisk::Destructive);
        assert_eq!(tool.risk(&json!({"action": "stop"})), ToolRisk::Write);
        assert_eq!(
            tool.requires(&json!({"action": "start"})),
            vec!["control.start"]
        );
        assert_eq!(
            tool.requires(&json!({"action": "kill"})),
            vec!["control.stop"]
        );
        assert_eq!(
            tool.requires(&json!({"action": "restart"})),
            vec!["control.restart"]
        );
        assert!(action("reboot").is_none());
        assert_eq!(
            tool.summarize(&json!({"action": "kill"})),
            "Kill the server"
        );
    }

    #[test]
    fn commands_are_summarised_short() {
        let summary = SendCommandTool.summarize(&json!({"command": "say ".repeat(40)}));
        assert!(summary.starts_with("Run: say"));
        assert!(summary.chars().count() <= 85);
    }
}
