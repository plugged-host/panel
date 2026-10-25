//! plugged.host: neo's account tools: the account's balances and allowance, creating a server,
//! and renaming, resizing and deleting the chat's server (docs/features/neo.md › Tools). They go
//! through the same shared code as the create page and the server settings.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, ToolScope, support};
use crate::{
    allowance::{Allowance, Resources},
    deleted_servers::DeleteServerError,
    models::{UpdatableModel, plan::Plan, server::UpdateServerOptions},
    server_creation::{CreateError, CreateRequest},
    server_resize::ResizeError,
};
use serde_json::{Value, json};
use std::borrow::Cow;

/// What the economy settings call the location tiers and the memberships that open them.
struct Names {
    membership: bool,
    plus: String,
    plusplus: String,
}

async fn names(state: &crate::State) -> Result<Names, ToolError> {
    let settings = state.settings.get().await?;

    Ok(Names {
        membership: settings.economy.membership_enabled,
        plus: settings.economy.plus_name.to_string(),
        plusplus: settings.economy.plusplus_name.to_string(),
    })
}

impl Names {
    /// A location tier as players see it.
    fn tier(&self, tier: Option<&str>) -> String {
        match tier.filter(|tier| !tier.is_empty()) {
            None => "free".into(),
            Some(crate::economy::TIER_PLUS) => self.plus.clone(),
            Some(crate::economy::TIER_PREMIUM) => self.plusplus.clone(),
            Some(other) => other.to_string(),
        }
    }

    /// The membership a tier needs.
    fn needs(&self, tier: Option<&str>) -> String {
        match tier {
            Some(crate::economy::TIER_PREMIUM) => self.plusplus.clone(),
            _ => self.plus.clone(),
        }
    }
}

/// `12% used of 200%`, or of unlimited.
fn usage(used: i64, limit: i64, unit: &str) -> String {
    if limit > 0 {
        format!("{used}{unit} used of {limit}{unit}")
    } else {
        format!("{used}{unit} used of unlimited")
    }
}

/// The resources the account uses beyond its limits.
fn over_quota(allowance: &Allowance) -> Vec<&'static str> {
    let over = |limit: i64, used: i64| limit > 0 && used > limit;

    [
        (over(allowance.limit.cpu, allowance.used.cpu), "cpu"),
        (
            over(allowance.limit.memory, allowance.used.memory),
            "memory",
        ),
        (over(allowance.limit.disk, allowance.used.disk), "disk"),
    ]
    .into_iter()
    .filter_map(|(over, name)| over.then_some(name))
    .collect()
}

fn size(resources: Resources) -> String {
    format!(
        "{}% CPU / {} MB RAM / {} MB disk",
        resources.cpu, resources.memory, resources.disk
    )
}

fn deploy_allowed(context: &ToolContext<'_>) -> Result<(), ToolError> {
    context
        .permissions
        .has_user_permission("servers.deploy")
        .map_err(|_| ToolError::invalid("This account may not create servers."))
}

pub struct AccountOverviewTool;

#[async_trait::async_trait]
impl Tool for AccountOverviewTool {
    fn name(&self) -> &'static str {
        "account_overview"
    }

    fn description(&self) -> Cow<'static, str> {
        "The user's account: token and credit balance, resource allowance (CPU/memory/disk/server count) with what is used and left, and which memberships (and so which location tiers) the account has. Use before creating or resizing servers.".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn scope(&self) -> ToolScope {
        ToolScope::Any
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "Check account and allowance".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        let balance = crate::economy::balance(context.state, context.user).await?;
        let names = names(context.state).await?;
        let allowance = &balance.allowance;

        // never "tokens: …": the secret masking would hide the balance
        let mut lines = Vec::new();
        if let Some(tokens) = balance.tokens {
            lines.push(format!("token balance: {tokens}"));
        }
        if let Some(credits) = balance.credits {
            lines.push(format!("credit balance: {credits}"));
        }
        lines.push(if allowance.max_servers > 0 {
            format!(
                "servers: {} of {}",
                allowance.servers, allowance.max_servers
            )
        } else {
            format!("servers: {} (no limit)", allowance.servers)
        });
        // a pool shared with other noprofit.space services is shown whole, their part named
        let elsewhere = |amount: i64, unit: &str| {
            if amount > 0 {
                format!(
                    " (of which {amount}{unit} by the account's other noprofit.space services, e.g. switched.cloud VPSes)"
                )
            } else {
                String::new()
            }
        };
        lines.push(format!(
            "cpu: {}{}",
            usage(allowance.used.cpu, allowance.limit.cpu, "%"),
            elsewhere(allowance.elsewhere.cpu, "%")
        ));
        lines.push(format!(
            "memory: {}{}",
            usage(allowance.used.memory, allowance.limit.memory, " MB"),
            elsewhere(allowance.elsewhere.memory, " MB")
        ));
        lines.push(format!(
            "disk: {}{}",
            usage(allowance.used.disk, allowance.limit.disk, " MB"),
            elsewhere(allowance.elsewhere.disk, " MB")
        ));
        if !allowance.has_plan {
            lines.push(
                "plan: none, so the account cannot create servers or make them bigger".into(),
            );
        }

        if names.membership {
            let membership =
                crate::economy::membership(&context.state.database, context.user.uuid).await?;
            let now = chrono::Utc::now().naive_utc();

            lines.push(format!(
                "{}: {}",
                names.plus,
                if membership.has_plus() {
                    format!("yes ({} locations unlocked)", names.plus)
                } else {
                    "no".into()
                }
            ));
            lines.push(format!(
                "{}: {}",
                names.plusplus,
                match membership.plusplus_until {
                    Some(until) if membership.has_plusplus(now) => format!(
                        "yes, until {} UTC ({} locations unlocked)",
                        until.format("%Y-%m-%d %H:%M"),
                        names.plusplus
                    ),
                    _ => "no".into(),
                }
            ));
        }

        let over = over_quota(allowance);
        if !over.is_empty() {
            lines.push(format!(
                "over_quota: {} (existing servers keep running but cannot grow; shrink or delete something)",
                over.join(", ")
            ));
        }

        Ok(ToolResult::ok(lines.join("\n")).with_display("Checked account"))
    }
}

pub struct ListCreateOptionsTool;

#[async_trait::async_trait]
impl Tool for ListCreateOptionsTool {
    fn name(&self) -> &'static str {
        "list_create_options"
    }

    fn description(&self) -> Cow<'static, str> {
        "What the user can create: games (with ids, their software choices and whether they need a EULA accepted), size plans (with ids, and whether each fits the remaining allowance) and locations (ids, tier, whether unlocked, latency, and whether a node there takes new servers now).".into()
    }

    fn parameters(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    fn scope(&self) -> ToolScope {
        ToolScope::Any
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "List games, plans and locations".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, _arguments: &Value) -> ToolOutcome {
        deploy_allowed(context)?;
        let options = crate::server_creation::options(context.state, context.user, None).await?;
        if !options.enabled {
            return Ok(
                ToolResult::ok("Creating servers is turned off on this panel.")
                    .with_display("Creating servers is off"),
            );
        }
        let names = names(context.state).await?;

        let games: Vec<String> = options
            .games
            .iter()
            .map(|game| {
                let mut line = format!("- {} (game id {})", game.name, game.egg_uuid);
                if !game.variants.is_empty() {
                    line.push_str(&format!(
                        ", software: {} (default: the first)",
                        game.variants
                            .iter()
                            .map(|variant| format!("{} = {}", variant.value, variant.label))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if game.eula {
                    line.push_str(", needs its EULA accepted by the user");
                }

                line
            })
            .collect();

        let plans: Vec<String> = options
            .plans
            .iter()
            .map(|plan| {
                format!(
                    "- {} (plan id {}): {}% CPU, {} MB RAM, {} MB disk{}",
                    plan.name,
                    plan.uuid,
                    plan.cpu,
                    plan.memory,
                    plan.disk,
                    if plan.available {
                        ""
                    } else {
                        " [does not fit the remaining allowance]"
                    }
                )
            })
            .collect();

        let locations: Vec<String> = options
            .locations
            .iter()
            .map(|location| {
                let tier = location.tier.as_deref();
                let mut line = format!(
                    "- {} (location id {}, {}",
                    location.name,
                    location.uuid,
                    names.tier(tier)
                );
                if tier.is_some_and(|tier| !tier.is_empty()) {
                    if location.unlocked {
                        line.push_str(", unlocked");
                    } else {
                        line.push_str(&format!(
                            ", locked: needs {} (/account/membership)",
                            names.needs(tier)
                        ));
                    }
                }
                if let Some(latency) = location.latency {
                    line.push_str(&format!(", ~{latency} ms"));
                }
                if !location.available {
                    line.push_str(", no room right now");
                }
                line.push(')');

                line
            })
            .collect();

        let section = |items: Vec<String>| {
            if items.is_empty() {
                "- none".to_string()
            } else {
                items.join("\n")
            }
        };

        Ok(ToolResult::ok(support::limit_output(&format!(
            "Games:\n{}\n\nPlans:\n{}\n\nLocations:\n{}",
            section(games),
            section(plans),
            section(locations)
        )))
        .with_display("Listed create options"))
    }
}

pub struct CreateServerTool;

/// A model-given id.
fn id(arguments: &Value, key: &str, unknown: &str) -> Result<uuid::Uuid, ToolError> {
    support::string(arguments, key, 64, true)?
        .trim()
        .parse()
        .map_err(|_| ToolError::invalid(unknown.to_string()))
}

#[async_trait::async_trait]
impl Tool for CreateServerTool {
    fn name(&self) -> &'static str {
        "create_server"
    }

    fn description(&self) -> Cow<'static, str> {
        "Create a new server for the user from their allowance. Needs a game id, a location id and a plan id from list_create_options, and optionally the game's software. Games that need a EULA accepted need accept_eula: true, only after the user said they accept it. New servers install and then start by themselves; Minecraft servers first open a setup for software and version.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "minLength": 3, "maxLength": 255},
                "game": {"type": "string", "description": "Game id."},
                "location": {"type": "string", "description": "Location id."},
                "plan": {"type": "string", "description": "Plan id."},
                "software": {"type": "string", "description": "Software value from list_create_options (default: the game's first)."},
                "accept_eula": {"type": "boolean", "description": "The user accepted the game's EULA."},
            },
            "required": ["name", "game", "location", "plan"],
        })
    }

    fn scope(&self) -> ToolScope {
        ToolScope::Any
    }

    fn risk(&self, arguments: &Value) -> ToolRisk {
        // accepting a EULA for the user is always theirs to confirm
        if support::boolean(arguments, "accept_eula", false) {
            ToolRisk::Critical
        } else {
            ToolRisk::Write
        }
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Create server \"{}\" (plan {}){}",
            support::short(support::shown(arguments, "name"), 60),
            support::short(support::shown(arguments, "plan"), 40),
            if support::boolean(arguments, "accept_eula", false) {
                ", accepting the game's EULA"
            } else {
                ""
            }
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        deploy_allowed(context)?;
        let name = support::string(arguments, "name", 255, true)?.trim();
        if name.chars().count() < 3 {
            return Err(ToolError::invalid(
                "The name must be at least 3 characters.",
            ));
        }
        let egg_uuid = id(
            arguments,
            "game",
            "Unknown game id; use list_create_options for game ids.",
        )?;
        let location_uuid = id(
            arguments,
            "location",
            "Unknown location id; use list_create_options.",
        )?;
        let plan_uuid = id(
            arguments,
            "plan",
            "Unknown plan id; use list_create_options.",
        )?;
        let variant = support::string(arguments, "software", 64, false)?.trim();

        let server = match crate::server_creation::create(
            context.state,
            context.user,
            CreateRequest {
                name: name.into(),
                egg_uuid,
                location_uuid,
                plan_uuid: Some(plan_uuid),
                size: None,
                variant: (!variant.is_empty()).then(|| variant.into()),
                eula: support::boolean(arguments, "accept_eula", false),
            },
        )
        .await
        {
            Ok(server) => server,
            Err(CreateError::Other(err)) => return Err(ToolError::Failed(err)),
            Err(err) => {
                let names = names(context.state).await?;
                return Err(ToolError::invalid(create_refusal(&err, &names)));
            }
        };

        context
            .log_user(
                "server.create",
                json!({
                    "uuid": server.uuid,
                    "name": server.name,
                    "egg_uuid": egg_uuid,
                    "location_uuid": location_uuid,
                    "plan_uuid": plan_uuid,
                }),
            )
            .await;

        let page = format!("/server/{:08x}", server.uuid_short);
        Ok(ToolResult::ok(format!(
            "Created \"{}\" (id {:08x}): {}. It is installing now; {} Its page: {page}",
            server.name,
            server.uuid_short,
            size(crate::server_resize::current(&server)),
            if crate::minecraft::is_minecraft(&server.egg) {
                "then the user picks its software and version in the setup on its page, and it starts."
            } else {
                "it starts on its own when done."
            }
        ))
        .with_display(format!("Created {}", server.name)))
    }
}

/// A create refusal as the model hears it.
fn create_refusal(err: &CreateError, names: &Names) -> String {
    match err {
        CreateError::Disabled => "Creating servers is turned off on this panel.".into(),
        CreateError::AccountBlocked => "This account cannot create servers right now.".into(),
        CreateError::Busy => {
            "Another server of this account is being created; try again in a moment.".into()
        }
        CreateError::UnknownGame => {
            "Unknown or unavailable game; use list_create_options for game ids.".into()
        }
        CreateError::EulaRequired => "This game needs its EULA accepted. Ask the user whether they accept it, then pass accept_eula: true.".into(),
        CreateError::UnknownVariant => {
            "That software is not offered for this game; use list_create_options.".into()
        }
        CreateError::UnknownLocation => "Unknown location id; use list_create_options.".into(),
        CreateError::LocationLocked => format!(
            "That location is locked: {} locations need {}, {} locations {}; the user gets them on the membership page (/account/membership).",
            names.plus, names.plus, names.plusplus, names.plusplus
        ),
        CreateError::UnknownPlan => "Unknown plan id; use list_create_options.".into(),
        CreateError::InvalidName | CreateError::InvalidSize => format!("That was refused: {}.", err.message()),
        CreateError::Allowance(refusal) => format!(
            "That does not fit the account's allowance ({}); check account_overview and pick a smaller plan.",
            refusal.message()
        ),
        CreateError::NoNode(reason) => format!(
            "No node in that location can take the server right now ({reason}); try another location."
        ),
        CreateError::Other(_) => "The server could not be created right now.".into(),
    }
}

pub struct RenameServerTool;

#[async_trait::async_trait]
impl Tool for RenameServerTool {
    fn name(&self) -> &'static str {
        "rename_server"
    }

    fn description(&self) -> Cow<'static, str> {
        "Rename this server (the name shown in the panel).".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"name": {"type": "string", "maxLength": 255}},
            "required": ["name"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["settings.rename"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Rename to \"{}\"",
            support::short(support::shown(arguments, "name"), 60)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = context.server()?.clone();
        let name = support::string(arguments, "name", 255, true)?.trim();
        if name.is_empty() {
            return Err(ToolError::invalid("Give a new name."));
        }

        let old = server.name.clone();
        server
            .update(
                context.state,
                UpdateServerOptions {
                    name: Some(name.into()),
                    ..Default::default()
                },
            )
            .await?;
        context
            .log("rename", json!({"old": old, "new": name}))
            .await;

        Ok(ToolResult::ok(format!("Renamed \"{old}\" to \"{name}\"."))
            .with_display(format!("Renamed to {name}")))
    }
}

pub struct ResizeServerTool;

/// The size an argument asks for: a plan's, or the current size with the given values changed.
fn target(current: Resources, arguments: &Value, plans: &[Plan]) -> Result<Resources, ToolError> {
    if let Some(plan) = arguments
        .get("plan")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|plan| !plan.is_empty())
    {
        let plan = plan
            .parse::<uuid::Uuid>()
            .ok()
            .and_then(|uuid| plans.iter().find(|candidate| candidate.uuid == uuid))
            .ok_or_else(|| ToolError::invalid("Unknown plan id; use list_create_options."))?;

        return Ok(Resources::new(plan.cpu as i64, plan.memory, plan.disk));
    }

    let value =
        |key: &str, current: i64| support::integer(arguments, key, current, i64::MIN, i64::MAX);

    Ok(Resources::new(
        value("cpu", current.cpu),
        value("memory", current.memory),
        value("disk", current.disk),
    ))
}

#[async_trait::async_trait]
impl Tool for ResizeServerTool {
    fn name(&self) -> &'static str {
        "resize_server"
    }

    fn description(&self) -> Cow<'static, str> {
        "Change this server's CPU (%), memory (MB) and disk (MB) within the owner's allowance (the server's current size always counts as available), or set it to a plan's size. Omitted values stay as they are. The node applies it live.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "cpu": {"type": "integer", "minimum": 1},
                "memory": {"type": "integer", "minimum": 1},
                "disk": {"type": "integer", "minimum": 1},
                "plan": {"type": "string", "description": "Plan id instead of numbers."},
            },
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["settings.resize"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        if let Some(plan) = arguments.get("plan").and_then(Value::as_str) {
            return format!("Resize to plan {}", support::short(plan, 40));
        }

        let shown = |key: &str| {
            arguments
                .get(key)
                .filter(|value| !value.is_null())
                .map_or("same".to_string(), |value| {
                    support::short(&value.to_string().replace('"', ""), 12)
                })
        };

        format!(
            "Resize to {}% CPU / {} MB RAM / {} MB disk",
            shown("cpu"),
            shown("memory"),
            shown("disk")
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = context.server()?.clone();
        let from = crate::server_resize::current(&server);
        let plans = if arguments.get("plan").is_some_and(|plan| !plan.is_null()) {
            Plan::all_public(&context.state.database).await?
        } else {
            Vec::new()
        };
        let to = target(from, arguments, &plans)?;
        if to == from {
            return Ok(ToolResult::ok(format!(
                "The server already has {}.",
                size(from)
            )));
        }

        match crate::server_resize::resize(
            context.state,
            &mut server,
            to,
            context.permissions.is_admin(),
        )
        .await
        {
            Ok(()) => {}
            Err(ResizeError::Other(err)) => return Err(ToolError::Failed(err)),
            Err(err) => {
                let max = crate::server_resize::options(context.state, &server)
                    .await?
                    .max;
                let bound = |value: Option<i64>, unit: &str| {
                    value.map_or("unlimited".to_string(), |value| format!("{value}{unit}"))
                };

                return Err(ToolError::invalid(match err {
                    ResizeError::Disabled => "Resizing servers is turned off on this panel.".into(),
                    ResizeError::Busy => "Another change to the owner's servers is still running; try again in a moment.".into(),
                    ResizeError::Invalid => "CPU, memory and disk must each be at least 1.".into(),
                    err => format!(
                        "{} Maximums for this server: {} CPU, {} memory, {} disk.",
                        support::sentence(&err.message()),
                        bound(max.cpu, "%"),
                        bound(max.memory, " MB"),
                        bound(max.disk, " MB")
                    ),
                }));
            }
        }

        context.log("resize", json!({"from": from, "to": to})).await;

        Ok(ToolResult::ok(format!(
            "Resized from {} to {}. The node applies it live; a restart may still be needed for everything to pick it up.",
            size(from),
            size(to)
        ))
        .with_display(format!("Resized to {}", size(to))))
    }
}

pub struct DeleteServerTool;

#[async_trait::async_trait]
impl Tool for DeleteServerTool {
    fn name(&self) -> &'static str {
        "delete_server"
    }

    fn description(&self) -> Cow<'static, str> {
        "Permanently delete this server with all its files, databases and backups. Irreversible. Only when the user explicitly asks; pass the server's exact name to confirm. The user always approves this.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "confirm_name": {"type": "string", "description": "The server's exact current name."},
            },
            "required": ["confirm_name"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Critical
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["settings.delete"]
    }

    fn summarize(&self, _arguments: &Value) -> String {
        "DELETE this server permanently".into()
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let server = context.server()?;
        if !context.permissions.is_admin()
            && !context
                .state
                .settings
                .get_as(|settings| settings.games.allow_deleting)
                .await?
        {
            return Err(ToolError::invalid(
                "Deleting servers is turned off on this panel.",
            ));
        }
        let confirm = arguments
            .get("confirm_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if confirm != server.name.as_str() {
            return Err(ToolError::invalid(format!(
                "confirm_name must be the server's exact name (\"{}\").",
                server.name
            )));
        }

        match crate::deleted_servers::delete_server(context.state, server).await {
            Ok(()) => {}
            Err(DeleteServerError::Lookup(err)) => return Err(ToolError::Failed(err)),
            Err(DeleteServerError::Delete(err)) => {
                return Err(match crate::response::extract_readable_message(&err) {
                    Some((message, _)) => {
                        ToolError::invalid(format!("The server could not be deleted: {message}."))
                    }
                    None => ToolError::Failed(err),
                });
            }
        }

        let id = format!("{:08x}", server.uuid_short);
        context
            .log_user(
                "server.delete",
                json!({"uuid": server.uuid, "name": server.name, "server": id}),
            )
            .await;

        Ok(ToolResult::redirect(
            format!(
                "Deleted the server \"{}\" ({id}). The user is being taken back to their server list.",
                server.name
            ),
            "/",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowance(limit: Resources, used: Resources) -> Allowance {
        Allowance {
            source: crate::allowance::AllowanceSource::Settings,
            has_plan: true,
            limit,
            used,
            elsewhere: crate::allowance::Resources::default(),
            servers: 1,
            max_servers: 0,
        }
    }

    #[test]
    fn reports_usage_and_overage() {
        assert_eq!(usage(50, 200, "%"), "50% used of 200%");
        assert_eq!(usage(1024, 0, " MB"), "1024 MB used of unlimited");
        assert_eq!(
            over_quota(&allowance(
                Resources::new(100, 2048, 0),
                Resources::new(150, 1024, 99999)
            )),
            vec!["cpu"]
        );
        assert!(over_quota(&allowance(Resources::default(), Resources::new(1, 1, 1))).is_empty());
    }

    #[test]
    fn names_tiers() {
        let names = Names {
            membership: true,
            plus: "Plus".into(),
            plusplus: "PlusPlus".into(),
        };
        assert_eq!(names.tier(None), "free");
        assert_eq!(names.tier(Some("")), "free");
        assert_eq!(names.tier(Some("premium")), "PlusPlus");
        assert_eq!(names.needs(Some("plus")), "Plus");
        assert_eq!(names.needs(Some("premium")), "PlusPlus");
        assert!(
            create_refusal(&CreateError::LocationLocked, &names).contains("/account/membership")
        );
    }

    #[test]
    fn resize_targets() {
        let current = Resources::new(100, 2048, 4096);
        assert_eq!(
            target(current, &json!({"memory": 4096}), &[]).unwrap(),
            Resources::new(100, 4096, 4096)
        );
        assert_eq!(target(current, &json!({}), &[]).unwrap(), current);
        assert!(target(current, &json!({"plan": "not-a-plan"}), &[]).is_err());
        assert_eq!(
            ResizeServerTool.summarize(&json!({"cpu": 200})),
            "Resize to 200% CPU / same MB RAM / same MB disk"
        );
    }

    #[test]
    fn eula_acceptance_always_asks() {
        assert_eq!(
            CreateServerTool.risk(&json!({"accept_eula": true})),
            ToolRisk::Critical
        );
        assert_eq!(CreateServerTool.risk(&json!({})), ToolRisk::Write);
    }
}
