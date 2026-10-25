//! plugged.host: neo's tools, the actions it can take (docs/features/neo.md › Tools).
//!
//! A tool is offered in one place ([`ToolScope`]): on a server, in the account chat, or in both.
//! The model gets the tools the user could use there ([`available`]), and each call is checked
//! again right before it runs ([`permitted`]), because permissions can change between a proposal
//! and its approval. Tools always act for the chat's user, with that user's own permissions on the
//! server, never more. How risky a call is ([`ToolRisk`]) decides whether the user is asked first.

mod account;
mod backups;
pub mod console;
mod files;
mod mclogs;
mod minecraft;
mod servers;
mod startup;
pub mod support;

use crate::models::{
    server::{Server, ServerActivityLogger},
    user::{PermissionManager, User},
    user_activity::UserActivityLogger,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{borrow::Cow, sync::LazyLock};
use utoipa::ToSchema;

/// Where a tool is offered.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolScope {
    /// In a server's chat, for a user neo may act for on that server.
    Server,
    /// In the account chat only.
    App,
    /// Everywhere (account-level tools: an overview, creating a server).
    Any,
}

/// How much a call can change.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ToolRisk {
    /// Looks, never changes anything: always runs without asking.
    Read,
    /// Changes the server: asks first unless the user turned on making changes without asking.
    Write,
    /// Hard to undo (delete, kill): like Write, and the admin can make it always ask.
    Destructive,
    /// Irreversible or paid at account level (delete a server, spend tokens): always asks.
    Critical,
}

impl ToolRisk {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Destructive => "destructive",
            Self::Critical => "critical",
        }
    }

    /// Whether a call of this risk waits for the user's approval.
    pub fn asks(self, auto_approve: bool, always_confirm_destructive: bool) -> bool {
        match self {
            Self::Read => false,
            Self::Write => !auto_approve,
            Self::Destructive => !auto_approve || always_confirm_destructive,
            Self::Critical => true,
        }
    }
}

/// What a call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub ok: bool,
    /// What the model reads.
    pub content: String,
    /// A short line for the chat (the call's summary when `None`).
    pub display: Option<String>,
    /// The action was handed to another part of the panel (a start gate: the queue, the waiting
    /// room, hibernation) and the user is taken there.
    pub redirect: Option<String>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            ok: true,
            content: content.into(),
            display: None,
            redirect: None,
        }
    }

    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = Some(display.into());
        self
    }

    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();

        Self {
            ok: false,
            display: Some(message.clone()),
            content: message,
            redirect: None,
        }
    }

    pub fn redirect(content: impl Into<String>, url: impl Into<String>) -> Self {
        let content = content.into();

        Self {
            ok: true,
            display: Some(content.clone()),
            content,
            redirect: Some(url.into()),
        }
    }
}

/// Why a call did not run.
#[derive(Debug)]
pub enum ToolError {
    /// Bad arguments or a refusal the model should read and act on.
    Invalid(String),
    /// Something broke; logged, and the model is told only that the call failed.
    Failed(anyhow::Error),
}

impl ToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

impl<E: Into<anyhow::Error>> From<E> for ToolError {
    fn from(error: E) -> Self {
        Self::Failed(error.into())
    }
}

pub type ToolOutcome = Result<ToolResult, ToolError>;

/// Who a call acts for and on what. Always an explicit user: calls also run from approvals and
/// the CLI, long after the request that proposed them.
pub struct ToolContext<'a> {
    pub state: &'a crate::State,
    pub user: &'a User,
    /// The server of a server chat, loaded for the user (with their subuser permissions).
    pub server: Option<&'a Server>,
    /// The user's permissions, scoped to `server` when there is one.
    pub permissions: &'a PermissionManager,
    pub conversation: uuid::Uuid,
    /// The address of the request that started or approved the call, for the activity log.
    pub ip: std::net::IpAddr,
}

impl ToolContext<'_> {
    /// The chat's server, for a tool that needs one.
    pub fn server(&self) -> Result<&Server, ToolError> {
        self.server
            .ok_or_else(|| ToolError::invalid("This needs a server; ask in that server's chat."))
    }

    /// Whether the user holds a server permission on the chat's server.
    pub fn can(&self, permission: &str) -> bool {
        self.server.is_some() && self.permissions.has_server_permission(permission).is_ok()
    }

    /// Writes `server:neo.{event}` for the chat's server, attributed to the user.
    pub async fn log(&self, event: &str, mut data: Value) {
        let Some(server) = self.server else {
            return;
        };
        if let Some(data) = data.as_object_mut() {
            data.insert("conversation".into(), json!(self.conversation));
        }

        ServerActivityLogger {
            state: self.state.clone(),
            server_uuid: server.uuid,
            user_uuid: self.user.uuid,
            impersonator_uuid: None,
            user_admin: self.user.admin,
            user_owner: server.owner.uuid == self.user.uuid,
            user_subuser: server.subuser_permissions.is_some(),
            api_key_uuid: None,
            ip: self.ip,
        }
        .log(format!("server:neo.{event}"), data)
        .await;
    }

    /// Writes `user:neo.{event}` for the user.
    pub async fn log_user(&self, event: &str, mut data: Value) {
        if let Some(data) = data.as_object_mut() {
            data.insert("conversation".into(), json!(self.conversation));
        }

        UserActivityLogger {
            state: self.state.clone(),
            user_uuid: self.user.uuid,
            impersonator_uuid: None,
            api_key_uuid: None,
            ip: self.ip,
        }
        .log(format!("user:neo.{event}"), data)
        .await;
    }
}

/// One action neo can take.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Unique, `[A-Za-z0-9_-]{1,64}`.
    fn name(&self) -> &'static str;

    /// What the model is told the tool does. Plain English; the model reads it on every turn.
    fn description(&self) -> Cow<'static, str>;

    /// JSON Schema of the arguments (an object).
    fn parameters(&self) -> Value;

    fn scope(&self) -> ToolScope {
        ToolScope::Server
    }

    /// Whether the tool makes sense here at all (the Minecraft tools only on Minecraft servers).
    /// Checked after the scope.
    fn available_for(&self, _context: &ToolContext<'_>) -> bool {
        true
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Read
    }

    /// The server permissions of which the user needs at least one to be offered the tool.
    fn offered_with(&self) -> Vec<&'static str> {
        self.requires(&Value::Null)
    }

    /// The server permissions this exact call needs. Checked again right before it runs.
    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        Vec::new()
    }

    /// One line for the chat and the approval card ("Edit server.properties").
    fn summarize(&self, arguments: &Value) -> String;

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome;
}

/// Every tool neo knows, in the order the model is offered them.
static TOOLS: LazyLock<Vec<Box<dyn Tool>>> = LazyLock::new(|| {
    let tools: Vec<Box<dyn Tool>> = vec![
        // the chat's server
        Box::new(console::ServerStatusTool),
        Box::new(console::ConsoleReadTool),
        Box::new(files::ListFilesTool),
        Box::new(files::ReadFileTool),
        Box::new(files::SearchFilesTool),
        Box::new(console::SendCommandTool),
        Box::new(console::PowerTool),
        Box::new(files::EditFileTool),
        Box::new(files::WriteFileTool),
        Box::new(files::CreateDirectoryTool),
        Box::new(files::RenameFileTool),
        Box::new(files::CompressFilesTool),
        Box::new(files::DecompressFileTool),
        Box::new(files::DeleteFilesTool),
        Box::new(startup::GetStartupTool),
        Box::new(startup::SetStartupVariableTool),
        Box::new(startup::SetDockerImageTool),
        Box::new(startup::ReinstallServerTool),
        Box::new(backups::ListBackupsTool),
        Box::new(backups::CreateBackupTool),
        Box::new(backups::RestoreBackupTool),
        Box::new(backups::DeleteBackupTool),
        // the account chat, and everywhere
        Box::new(servers::ListServersTool),
        Box::new(mclogs::ReadMclogsTool),
        Box::new(account::AccountOverviewTool),
        Box::new(account::ListCreateOptionsTool),
        Box::new(account::CreateServerTool),
        Box::new(account::RenameServerTool),
        Box::new(account::ResizeServerTool),
        Box::new(account::DeleteServerTool),
        // Minecraft servers only
        Box::new(minecraft::MinecraftInfoTool),
        Box::new(minecraft::SearchContentTool),
        Box::new(minecraft::ContentVersionsTool),
        Box::new(minecraft::InstallContentTool),
        Box::new(minecraft::ListInstalledContentTool),
        Box::new(minecraft::RemoveContentTool),
        Box::new(minecraft::ToggleContentTool),
        Box::new(minecraft::InstallGeyserTool),
        Box::new(minecraft::InstallModpackTool),
        Box::new(minecraft::SwitchSoftwareTool),
    ];

    tools
});

pub fn all() -> &'static [Box<dyn Tool>] {
    &TOOLS
}

pub fn find(name: &str) -> Option<&'static dyn Tool> {
    TOOLS
        .iter()
        .find(|tool| tool.name() == name)
        .map(|tool| tool.as_ref())
}

/// Whether neo may act on the chat's server for the user: its owner, or a subuser holding
/// `neo.access`. Admins are not let in by being admins.
pub fn may_use_on(user: &User, server: &Server) -> bool {
    server.owner.uuid == user.uuid
        || server
            .subuser_permissions
            .as_ref()
            .is_some_and(|permissions| permissions.iter().any(|p| p == "neo.access"))
}

fn in_scope(tool: &dyn Tool, context: &ToolContext<'_>) -> bool {
    match tool.scope() {
        ToolScope::Any => true,
        ToolScope::App => context.server.is_none(),
        ToolScope::Server => context
            .server
            .is_some_and(|server| may_use_on(context.user, server)),
    }
}

/// The tools the model may be offered here: the right place, and the user holds at least one of
/// the permissions the tool can be used with.
pub fn available(context: &ToolContext<'_>) -> Vec<&'static dyn Tool> {
    TOOLS
        .iter()
        .map(|tool| tool.as_ref())
        .filter(|tool| in_scope(*tool, context) && tool.available_for(context))
        .filter(|tool| {
            let permissions = match tool.scope() {
                ToolScope::Server => tool.offered_with(),
                _ => Vec::new(),
            };

            permissions.is_empty() || permissions.iter().any(|p| context.can(p))
        })
        .collect()
}

/// Whether this exact call is allowed right now.
pub fn permitted(tool: &dyn Tool, context: &ToolContext<'_>, arguments: &Value) -> bool {
    if !in_scope(tool, context) {
        return false;
    }

    tool.scope() != ToolScope::Server || tool.requires(arguments).iter().all(|p| context.can(p))
}

/// The tools in the shape the model is given them.
pub fn definitions(tools: &[&dyn Tool]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": tool.parameters(),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_by_risk() {
        assert!(!ToolRisk::Read.asks(false, true));
        assert!(ToolRisk::Write.asks(false, false));
        assert!(!ToolRisk::Write.asks(true, true));
        assert!(ToolRisk::Destructive.asks(true, true));
        assert!(!ToolRisk::Destructive.asks(true, false));
        assert!(ToolRisk::Critical.asks(true, false));
        assert!(ToolRisk::Critical > ToolRisk::Write);
    }

    #[test]
    fn tool_names_are_unique_and_valid() {
        let mut names = std::collections::HashSet::new();
        for tool in all() {
            let name = tool.name();
            assert!(
                !name.is_empty()
                    && name.len() <= 64
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "bad tool name {name}"
            );
            assert!(names.insert(name), "duplicate tool {name}");
            assert!(tool.parameters().is_object(), "{name} parameters");
        }
    }

    #[test]
    fn every_tool_in_its_place() {
        let names: Vec<&str> = all().iter().map(|tool| tool.name()).collect();
        assert_eq!(
            names,
            [
                "server_status",
                "console_read",
                "list_files",
                "read_file",
                "search_files",
                "send_command",
                "power",
                "edit_file",
                "write_file",
                "create_directory",
                "rename_file",
                "compress_files",
                "decompress_file",
                "delete_files",
                "get_startup",
                "set_startup_variable",
                "set_docker_image",
                "reinstall_server",
                "list_backups",
                "create_backup",
                "restore_backup",
                "delete_backup",
                "list_servers",
                "read_mclogs",
                "account_overview",
                "list_create_options",
                "create_server",
                "rename_server",
                "resize_server",
                "delete_server",
                "minecraft_info",
                "search_content",
                "content_versions",
                "install_content",
                "list_installed_content",
                "remove_content",
                "toggle_content",
                "install_geyser",
                "install_modpack",
                "switch_server_software",
            ]
        );

        // a server tool without a permission would be open to every subuser neo may act for
        for tool in all() {
            if tool.scope() == ToolScope::Server
                && !matches!(tool.name(), "server_status" | "minecraft_info")
            {
                assert!(
                    !tool.requires(&Value::Null).is_empty(),
                    "{} needs a permission",
                    tool.name()
                );
            }
        }
        assert_eq!(
            find("list_servers").map(|tool| tool.scope()),
            Some(ToolScope::App)
        );
        assert_eq!(
            find("read_mclogs").map(|tool| tool.scope()),
            Some(ToolScope::Any)
        );
        assert_eq!(
            find("create_server").map(|tool| tool.scope()),
            Some(ToolScope::Any)
        );
        assert_eq!(
            find("delete_server").map(|tool| tool.risk(&Value::Null)),
            Some(ToolRisk::Critical)
        );
    }
}
