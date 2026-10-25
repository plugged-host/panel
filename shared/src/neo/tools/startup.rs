//! plugged.host: neo's tools for a server's startup settings and reinstalling it
//! (docs/features/neo.md › Tools). Same checks as the startup and reinstall routes.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, support};
use crate::models::{
    UpdatableModel,
    server::{ServerInstallOptions, UpdateServerOptions},
    server_variable::ServerVariable,
};
use serde_json::{Value, json};
use std::{borrow::Cow, collections::HashMap};

/// The server's variables with their rules as the routes show and validate them.
async fn variables(context: &ToolContext<'_>) -> Result<Vec<ServerVariable>, ToolError> {
    let server = context.server()?;
    let mut variables = ServerVariable::all_by_server_uuid_egg_uuid(
        &context.state.database,
        server.uuid,
        server.egg.uuid,
    )
    .await?;
    for variable in variables.iter_mut() {
        ServerVariable::run_rules_handlers(
            server,
            &variable.variable.env_variable,
            &mut variable.variable.rules,
        )
        .await?;
    }

    Ok(variables)
}

/// A variable's value as neo may see it: secrets never, anything else with secrets masked.
fn shown_value(variable: &ServerVariable) -> String {
    if variable.variable.secret {
        crate::redact::MASK.to_string()
    } else {
        support::redact(&variable.value)
    }
}

/// The image an argument names: one of the egg's images by label or by the image itself.
fn offered_image<'a>(
    images: &'a indexmap::IndexMap<compact_str::CompactString, compact_str::CompactString>,
    wanted: &str,
) -> Option<&'a compact_str::CompactString> {
    images
        .get(wanted)
        .or_else(|| images.values().find(|image| image.as_str() == wanted))
}

pub struct GetStartupTool;

#[async_trait::async_trait]
impl Tool for GetStartupTool {
    fn name(&self) -> &'static str {
        "get_startup"
    }

    fn description(&self) -> Cow<'static, str> {
        "The server's startup command, its docker image (and the images it may switch to), and the startup variables the user can see, with whether each is editable and its validation rules.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["startup.read"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Read startup settings".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let images = &server.egg.docker_images;
        let custom = !images.values().any(|image| *image == server.image);
        let overwrite = context
            .state
            .settings
            .get_as(|settings| settings.server.allow_overwriting_custom_docker_image)
            .await?;

        let mut lines = vec![
            format!("startup: {}", support::redact(&server.startup)),
            format!(
                "docker_image: {}{}",
                server.image,
                if custom && !overwrite {
                    " (set by an administrator, cannot be changed)"
                } else if custom {
                    " (custom)"
                } else {
                    ""
                }
            ),
            format!(
                "available_images: {}",
                images
                    .iter()
                    .map(|(label, image)| format!("{label} = {image}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "variables:".to_string(),
        ];

        for variable in variables(context)
            .await?
            .iter()
            .filter(|variable| variable.variable.user_viewable)
        {
            lines.push(format!(
                "- {} ({}) = \"{}\"{}{} rules: {}",
                variable.variable.env_variable,
                variable.variable.name,
                shown_value(variable),
                if variable.variable.user_editable {
                    ""
                } else {
                    " [read-only]"
                },
                if variable.variable.secret {
                    " [secret]"
                } else {
                    ""
                },
                variable.variable.rules.join("|")
            ));
        }

        Ok(ToolResult::ok(support::limit_output(&lines.join("\n")))
            .with_display("Read startup settings"))
    }
}

pub struct SetStartupVariableTool;

#[async_trait::async_trait]
impl Tool for SetStartupVariableTool {
    fn name(&self) -> &'static str {
        "set_startup_variable"
    }

    fn description(&self) -> Cow<'static, str> {
        "Change one editable startup variable (names from get_startup), e.g. a version or jar name. Applies on the next start.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": {"type": "string", "description": "The env variable name, e.g. SERVER_JARFILE."},
                "value": {"type": "string"},
            },
            "required": ["key", "value"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["startup.update"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let value: String = arguments
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(60)
            .collect();

        format!("Set {} = \"{value}\"", support::shown(arguments, "key"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        let key = support::string(arguments, "key", 255, true)?.trim();
        let value = match arguments.get("value") {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.to_string(),
            Some(Value::Bool(value)) => value.to_string(),
            _ => return Err(ToolError::invalid("\"value\" must be a string.")),
        };
        if value.chars().count() > 4096 {
            return Err(ToolError::invalid(
                "That value is too long (max 4096 characters).",
            ));
        }
        if support::masked(&value) {
            return Err(ToolError::invalid(
                "Masked •••• values cannot be written back by neo; ask the user to set this value themselves.",
            ));
        }

        let variables = variables(context).await?;
        let Some(variable) = variables.iter().find(|variable| {
            variable.variable.env_variable == key && variable.variable.user_viewable
        }) else {
            return Err(ToolError::invalid(format!(
                "There is no startup variable {key}."
            )));
        };
        if !variable.variable.user_editable {
            return Err(ToolError::invalid(format!("{key} is read-only.")));
        }

        // every variable, with the new value in, as the startup route validates them
        let checked: HashMap<&str, (&[compact_str::CompactString], &str)> = variables
            .iter()
            .map(|other| {
                (
                    other.variable.env_variable.as_str(),
                    (
                        other.variable.rules.as_slice(),
                        if other.variable.uuid == variable.variable.uuid {
                            value.as_str()
                        } else {
                            other.value.as_str()
                        },
                    ),
                )
            })
            .collect();
        let validation = rule_validator::Validator::new(checked)
            .map_err(|err| err.to_string())
            .and_then(|validator| validator.validate());
        if let Err(error) = validation {
            return Err(ToolError::invalid(format!(
                "Invalid value for {key}: {error}"
            )));
        }

        let old = shown_value(variable);
        ServerVariable::create(
            &context.state.database,
            server.uuid,
            variable.variable.uuid,
            &value,
        )
        .await?;
        server.clone().batch_sync(context.state).await;

        let new = if variable.variable.secret {
            crate::redact::MASK.to_string()
        } else {
            support::redact(&value)
        };
        context
            .log(
                "startup.variable",
                json!({"variable": key, "value": if variable.variable.secret { crate::redact::MASK } else { value.as_str() }}),
            )
            .await;

        Ok(ToolResult::ok(format!(
            "Set {key} from \"{old}\" to \"{new}\". It applies on the next start."
        ))
        .with_display(format!("Set {key}")))
    }
}

pub struct SetDockerImageTool;

#[async_trait::async_trait]
impl Tool for SetDockerImageTool {
    fn name(&self) -> &'static str {
        "set_docker_image"
    }

    fn description(&self) -> Cow<'static, str> {
        "Switch the server's docker image (e.g. a different Java or Node version) to one of the images listed by get_startup. Applies on the next start.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image": {"type": "string", "description": "Image name or its label from get_startup."},
            },
            "required": ["image"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["startup.docker-image"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Switch docker image to {}",
            support::short(support::shown(arguments, "image"), 80)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = context.server()?.clone();
        let wanted = support::string(arguments, "image", 255, true)?.trim();
        let Some(image) = offered_image(&server.egg.docker_images, wanted).cloned() else {
            return Err(ToolError::invalid(
                "That image is not offered for this server; use one from get_startup.",
            ));
        };

        let overwrite = context
            .state
            .settings
            .get_as(|settings| settings.server.allow_overwriting_custom_docker_image)
            .await?;
        if !overwrite
            && !server
                .egg
                .docker_images
                .values()
                .any(|offered| *offered == server.image)
        {
            return Err(ToolError::invalid(
                "This server's image was set by an administrator and cannot be changed.",
            ));
        }

        let old = server.image.clone();
        if old == image {
            return Ok(ToolResult::ok(format!("The server already uses {image}.")));
        }

        server
            .update(
                context.state,
                UpdateServerOptions {
                    image: Some(image.clone()),
                    ..Default::default()
                },
            )
            .await?;
        context
            .log("startup.image", json!({"old": old, "new": image}))
            .await;
        server.batch_sync(context.state).await;

        Ok(ToolResult::ok(format!(
            "Docker image changed from {old} to {image}. It applies on the next start."
        ))
        .with_display(format!("Image set to {image}")))
    }
}

pub struct ReinstallServerTool;

#[async_trait::async_trait]
impl Tool for ReinstallServerTool {
    fn name(&self) -> &'static str {
        "reinstall_server"
    }

    fn description(&self) -> Cow<'static, str> {
        "Stop the server and run its install script again (re-downloads the server software). Files are normally kept, but the install script may overwrite some. Suggest a backup first.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["settings.install"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Reinstall the server".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let server = context.server()?.clone();
        let state = context.state.clone();

        // spawned like the route: an install must not be cut off halfway when the chat goes away
        tokio::spawn(async move {
            server
                .install_with_options(
                    &state,
                    ServerInstallOptions {
                        truncate_directory: false,
                        installation_script: None,
                        start_on_completion: false,
                    },
                )
                .await
        })
        .await
        .map_err(anyhow::Error::from)??;
        context
            .log(
                "reinstall",
                json!({"truncate_directory": false, "start_on_completion": false}),
            )
            .await;

        Ok(ToolResult::ok(
            "The server is reinstalling now; it takes a few minutes. Other tools will report that it is installing until it finishes.",
        )
        .with_display("Reinstalling"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_images_by_label_or_value() {
        let mut images = indexmap::IndexMap::new();
        images.insert(
            compact_str::CompactString::from("Java 21"),
            compact_str::CompactString::from("ghcr.io/example/java:21"),
        );
        images.insert(
            compact_str::CompactString::from("Java 17"),
            compact_str::CompactString::from("ghcr.io/example/java:17"),
        );

        assert_eq!(
            offered_image(&images, "Java 17").map(|image| image.as_str()),
            Some("ghcr.io/example/java:17")
        );
        assert_eq!(
            offered_image(&images, "ghcr.io/example/java:21").map(|image| image.as_str()),
            Some("ghcr.io/example/java:21")
        );
        assert!(offered_image(&images, "ghcr.io/example/java:8").is_none());
    }

    #[test]
    fn summaries_are_short() {
        let long = "x".repeat(100);
        assert_eq!(
            SetStartupVariableTool.summarize(&json!({"key": "SERVER_JARFILE", "value": long})),
            format!("Set SERVER_JARFILE = \"{}\"", "x".repeat(60))
        );
        assert_eq!(
            ReinstallServerTool.summarize(&json!({})),
            "Reinstall the server"
        );
    }
}
