//! plugged.host: everything rux can do, as tools the model calls.
//!
//! A tool is a [`Tool`]: its name, what it is for, who may use it ([`Level`]: admins get every
//! tool; members of a rux moderator role only the moderator ones, never offered nor run the
//! others), whether it is offered for this order at all, the JSON schema of its arguments and
//! what it does. Arguments arrive as the model wrote them: a tool checks its own and answers
//! [`ToolError::Invalid`] with a message the model can act on.
//!
//! The tools live in their groups ([`moderation`], [`giveaways`], [`rules`], [`credits`],
//! [`servers`]); each group's `tools()` lists its own, and the [`Registry`] collects them. The
//! only tools that may give anyone anything are the credit grant and the giveaways, and what they
//! give is credits: a tool that hands out something else breaks the rule that rux only ever gives
//! credits.

pub mod credits;
pub mod giveaways;
pub mod moderation;
pub mod rules;
pub mod servers;

use super::{access::Level, context::Context};
use serde_json::{Value, json};
use std::sync::LazyLock;

/// What a tool tells the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub ok: bool,
    pub content: String,
    /// A DM's order acts on this server from here on (`switch_server`); the agent moves the
    /// order there and rebuilds its prompt.
    pub switch_guild: Option<String>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            ok: true,
            content: content.into(),
            switch_guild: None,
        }
    }

    /// JSON for the model (slashes and unicode as they are).
    pub fn json(content: &Value) -> Self {
        Self::ok(serde_json::to_string(content).unwrap_or_default())
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            content: message.into(),
            switch_guild: None,
        }
    }

    pub fn with_guild(mut self, guild_id: impl Into<String>) -> Self {
        self.switch_guild = Some(guild_id.into());
        self
    }
}

/// Why a tool did not do what it was asked.
#[derive(Debug)]
pub enum ToolError {
    /// The arguments, or what they point at, do not work: said so the model can act on it.
    Invalid(String),
    /// Discord refused (its message says why and what usually causes it).
    Discord(super::discord::DiscordError),
    /// noprofit.space could not answer.
    Unavailable(super::noprofit::Unavailable),
    /// Anything else: logged; the model only hears that it failed.
    Failed(anyhow::Error),
}

impl ToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    /// What the model is told.
    pub fn for_model(&self) -> String {
        match self {
            Self::Invalid(message) => message.clone(),
            Self::Discord(err) => err.message.clone(),
            Self::Unavailable(err) => err.0.clone(),
            Self::Failed(_) => {
                "rux failed while doing this (an internal error). Tell the admin it did not work."
                    .into()
            }
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(err) => write!(f, "{err:#}"),
            other => f.write_str(&other.for_model()),
        }
    }
}

impl From<super::discord::DiscordError> for ToolError {
    fn from(err: super::discord::DiscordError) -> Self {
        Self::Discord(err)
    }
}

impl From<super::noprofit::Unavailable> for ToolError {
    fn from(err: super::noprofit::Unavailable) -> Self {
        Self::Unavailable(err)
    }
}

impl From<anyhow::Error> for ToolError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

impl From<sqlx::Error> for ToolError {
    fn from(err: sqlx::Error) -> Self {
        Self::Failed(err.into())
    }
}

/// One thing rux can do.
#[async_trait::async_trait]
pub trait Tool: Send + Sync + 'static {
    /// The name the model calls it by (`snake_case`, unique).
    fn name(&self) -> &'static str;

    /// What it does and when to use it, for the model.
    fn description(&self, context: &Context) -> String;

    /// Who may use it: admins (the default), or moderators too.
    fn level(&self) -> Level {
        Level::Admin
    }

    /// Whether the model is offered it for this order at all (on top of [`Self::level`]).
    fn offered(&self, _context: &Context) -> bool {
        true
    }

    /// JSON schema of its arguments ([`schema`]).
    fn parameters(&self) -> Value;

    async fn execute(&self, context: &Context, arguments: &Value) -> Result<ToolResult, ToolError>;
}

/// An arguments schema: an object of these properties, these required, nothing else.
pub fn schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": if properties.as_object().is_some_and(|p| !p.is_empty()) { properties } else { json!({}) },
        "required": required,
        "additionalProperties": false,
    })
}

/// A text argument, trimmed; numbers count as text. `None` when missing or empty.
pub fn string(arguments: &Value, key: &str) -> Option<String> {
    match &arguments[key] {
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A text argument that must be there.
pub fn required(arguments: &Value, key: &str) -> Result<String, ToolError> {
    string(arguments, key).ok_or_else(|| ToolError::Invalid(format!("{key} is required.")))
}

/// A whole number, also when written as text.
pub fn int(arguments: &Value, key: &str) -> Option<i64> {
    match &arguments[key] {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|n| n as i64)),
        Value::String(text) => {
            let text = text.trim();
            text.parse::<i64>()
                .ok()
                .or_else(|| text.parse::<f64>().ok().map(|n| n as i64))
        }
        _ => None,
    }
}

/// A yes/no, also when written as text (`true`, `1`, `yes`).
pub fn boolean(arguments: &Value, key: &str, default: bool) -> bool {
    match &arguments[key] {
        Value::Bool(value) => *value,
        Value::String(text) => matches!(text.trim().to_lowercase().as_str(), "true" | "1" | "yes"),
        Value::Number(number) => number.as_i64() == Some(1),
        _ => default,
    }
}

/// A list of texts, also when written as one comma separated text.
pub fn strings(arguments: &Value, key: &str) -> Vec<String> {
    match &arguments[key] {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.trim().to_string()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect(),
        Value::String(text) => text
            .split(',')
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every tool, by name.
pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    /// The tools of every group; a name listed twice keeps the first.
    pub fn new(groups: Vec<Vec<Box<dyn Tool>>>) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        for tool in groups.into_iter().flatten() {
            if tools.iter().any(|t| t.name() == tool.name()) {
                tracing::warn!(
                    "rux: two tools are called {}; the second is left out",
                    tool.name()
                );
                continue;
            }
            tools.push(tool);
        }

        Self { tools }
    }

    pub fn find(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| tool.as_ref())
    }

    /// Whether a tool of this name exists (whoever may use it).
    pub fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|tool| tool.name()).collect()
    }

    /// The tools for this level that are offered for this order.
    pub fn available(&self, level: Level, context: &Context) -> Vec<&dyn Tool> {
        self.tools
            .iter()
            .map(|tool| tool.as_ref())
            .filter(|tool| level == Level::Admin || tool.level() == Level::Moderator)
            .filter(|tool| tool.offered(context))
            .collect()
    }

    /// The tools for this level and order, as the model gets them.
    pub fn definitions(&self, level: Level, context: &Context) -> Vec<Value> {
        self.available(level, context)
            .into_iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(context),
                        "parameters": tool.parameters(),
                    },
                })
            })
            .collect()
    }

    /// Runs one call for this order: refused when it is not available to it, errors made into
    /// what the model is told.
    pub async fn execute(&self, context: &Context, name: &str, arguments: &Value) -> ToolResult {
        let Some(tool) = self
            .available(context.level, context)
            .into_iter()
            .find(|tool| tool.name() == name)
        else {
            return ToolResult::error(if self.has(name) {
                format!(
                    "{name} is for {} admins only; this person is a moderator.",
                    context.config.portal
                )
            } else {
                format!("There is no tool called {name}.")
            });
        };

        match tool.execute(context, arguments).await {
            Ok(result) => result,
            Err(err) => {
                if let ToolError::Failed(err) = &err {
                    tracing::error!(tool = name, "rux: tool failed: {err:#}");
                }
                ToolResult::error(err.for_model())
            }
        }
    }
}

static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    Registry::new(vec![
        giveaways::tools(),
        credits::tools(),
        moderation::tools(),
        rules::tools(),
        servers::tools(),
    ])
});

/// Every tool rux has.
pub fn registry() -> &'static Registry {
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_read_as_the_model_writes_them() {
        let arguments = json!({
            "text": "  hi ",
            "empty": " ",
            "number": 12,
            "numeric_text": "7",
            "float": 2.9,
            "yes": "Yes",
            "list": ["a", " ", 3],
            "csv": "a, b,,c"
        });

        assert_eq!(string(&arguments, "text").as_deref(), Some("hi"));
        assert_eq!(string(&arguments, "empty"), None);
        assert_eq!(string(&arguments, "number").as_deref(), Some("12"));
        assert_eq!(string(&arguments, "missing"), None);
        assert!(
            matches!(required(&arguments, "missing"), Err(ToolError::Invalid(m)) if m == "missing is required.")
        );
        assert_eq!(int(&arguments, "number"), Some(12));
        assert_eq!(int(&arguments, "numeric_text"), Some(7));
        assert_eq!(int(&arguments, "float"), Some(2));
        assert_eq!(int(&arguments, "text"), None);
        assert!(boolean(&arguments, "yes", false));
        assert!(boolean(&arguments, "missing", true));
        assert_eq!(strings(&arguments, "list"), vec!["a", "3"]);
        assert_eq!(strings(&arguments, "csv"), vec!["a", "b", "c"]);
    }

    #[test]
    fn schemas_close_the_object() {
        let schema = schema(json!({}), &[]);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"], json!({}));
        assert_eq!(schema["required"], json!([]));
    }

    struct Named(&'static str);

    #[async_trait::async_trait]
    impl Tool for Named {
        fn name(&self) -> &'static str {
            self.0
        }

        fn description(&self, _context: &Context) -> String {
            String::new()
        }

        fn parameters(&self) -> Value {
            schema(json!({}), &[])
        }

        async fn execute(
            &self,
            _context: &Context,
            _arguments: &Value,
        ) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::ok("ok"))
        }
    }

    #[test]
    fn a_name_is_taken_once() {
        let registry = Registry::new(vec![
            vec![Box::new(Named("a")), Box::new(Named("b"))],
            vec![Box::new(Named("a"))],
        ]);
        assert_eq!(registry.names(), vec!["a", "b"]);
        assert!(registry.has("b") && !registry.has("c"));
    }

    #[test]
    fn results() {
        let result = ToolResult::json(&json!({"url": "https://example.com/a", "name": "é"}));
        assert!(result.ok);
        assert_eq!(
            result.content,
            r#"{"url":"https://example.com/a","name":"é"}"#
        );
        assert_eq!(
            ToolResult::ok("x").with_guild("1").switch_guild.as_deref(),
            Some("1")
        );
        assert!(!ToolResult::error("no").ok);
        assert_eq!(
            ToolError::Failed(anyhow::anyhow!("secret detail")).for_model(),
            "rux failed while doing this (an internal error). Tell the admin it did not work."
        );
    }
}
