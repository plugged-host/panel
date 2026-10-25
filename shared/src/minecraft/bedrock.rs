//! plugged.host: Bedrock players on a Java server (docs/features/minecraft.md): Geyser translates
//! for them, Floodgate lets them in without a Java account, and the server gets one more port of
//! its node for Bedrock's UDP listener.
//!
//! Geyser comes from GeyserMC's own build API, which has a build for every platform offered here
//! (Paper and its forks take the Spigot build). Floodgate comes from there too for the Spigot
//! family and from Modrinth for Fabric and NeoForge, where GeyserMC's API has none.

use super::{Loader, content, files, modrinth};
use crate::models::{node_allocation::NodeAllocation, server::Server};
use serde::Serialize;
use utoipa::ToSchema;

const GEYSERMC: &str = "https://download.geysermc.org/v2/projects";
/// Marks the allocation a server holds for Bedrock, so installing again reuses it.
pub const NOTE: &str = "Bedrock (Geyser)";

/// GeyserMC's name for the platform a loader runs, `None` when Geyser has no build for it.
pub fn platform(loader: Option<Loader>) -> Option<&'static str> {
    match loader? {
        Loader::Paper => Some("spigot"),
        Loader::Fabric => Some("fabric"),
        Loader::NeoForge => Some("neoforge"),
        Loader::Quilt | Loader::Forge => None,
    }
}

/// Where Geyser reads its config on a platform (its data folder).
fn config_path(platform: &str) -> &'static str {
    match platform {
        "fabric" => "config/Geyser-Fabric/config.yml",
        "neoforge" => "config/Geyser-NeoForge/config.yml",
        _ => "plugins/Geyser-Spigot/config.yml",
    }
}

fn jar_name(platform: &str) -> String {
    let mut chars = platform.chars();
    let capitalised: String = chars
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default();

    format!("Geyser-{capitalised}.jar")
}

fn is_geyser(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("geyser") && name.ends_with(".jar")
}

fn is_floodgate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("floodgate") && name.ends_with(".jar")
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Bedrock {
    /// Whether the server's software has a Geyser build.
    pub available: bool,
    /// The Geyser jar on the server.
    pub installed: Option<String>,
    /// The port Bedrock players join on, once one is held.
    pub port: Option<i32>,
    /// The address Bedrock players join, once a port is held.
    pub address: Option<String>,
    pub running: bool,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Installed {
    /// A Geyser jar was there before and was replaced.
    pub updated: bool,
    pub floodgate: bool,
    pub port: i32,
    pub address: String,
    /// The server was running and was restarted to load it.
    pub restarted: bool,
}

#[derive(Debug)]
pub enum BedrockError {
    Unsupported,
    NoPort,
    Wings(anyhow::Error),
    Content(content::ContentError),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for BedrockError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<crate::database::DatabaseError> for BedrockError {
    fn from(err: crate::database::DatabaseError) -> Self {
        Self::Other(err.into())
    }
}

impl BedrockError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        match self {
            Self::Unsupported => crate::response::ApiResponse::error(
                "geyser has no build for this server's software (it runs on paper and its forks, fabric and neoforge)",
            )
            .with_status(StatusCode::BAD_REQUEST),
            Self::NoPort => crate::response::ApiResponse::error(
                "this node has no free port left for bedrock",
            )
            .with_status(StatusCode::CONFLICT),
            Self::Wings(err) => {
                tracing::warn!("minecraft bedrock: {err:#}");
                crate::response::ApiResponse::error(
                    "the server's files could not be reached, try again in a moment",
                )
                .with_status(StatusCode::BAD_GATEWAY)
            }
            Self::Content(err) => err.response(),
            Self::Other(err) => crate::response::ApiResponse::from(err),
        }
    }
}

/// The address players type for an allocation: its alias, else its IP.
fn address(allocation: &NodeAllocation) -> String {
    allocation
        .ip_alias
        .as_ref()
        .map(|alias| alias.to_string())
        .unwrap_or_else(|| allocation.ip.ip().to_string())
}

/// The allocation the server holds for Bedrock: the marked one, else (for a Geyser set up by
/// hand) a non-primary one on the port its config listens on.
async fn held_allocation(
    state: &crate::State,
    server: &Server,
    configured: Option<i32>,
) -> Result<Option<crate::models::server_allocation::ServerAllocation>, anyhow::Error> {
    let primary = server.allocation.as_ref().map(|allocation| allocation.uuid);
    let allocations = crate::models::server_allocation::ServerAllocation::all_by_server_uuid(
        &state.database,
        server.uuid,
    )
    .await?;
    let others = allocations
        .into_iter()
        .filter(|allocation| Some(allocation.uuid) != primary);

    let mut by_port = None;
    for allocation in others {
        if allocation.notes.as_deref() == Some(NOTE) {
            return Ok(Some(allocation));
        }
        if configured == Some(allocation.allocation.port) && by_port.is_none() {
            by_port = Some(allocation);
        }
    }

    Ok(by_port)
}

/// Takes a port for Bedrock the way a player adds one: the next free one above the primary inside
/// the deployment port ranges, made when missing (docs/features/allocations.md). When the ranges
/// are full, the lowest free port of the server's node (on its primary IP when it has one),
/// skipping ports something on the node already listens on.
async fn claim_allocation(state: &crate::State, server: &Server) -> Result<i32, BedrockError> {
    use crate::prelude::SqlxErrorExt;
    use sqlx::Row;

    match crate::allocations::add_port(state, server, Some(NOTE)).await {
        Ok(server_allocation) => {
            let port: i32 = sqlx::query_scalar(
                "SELECT node_allocations.port FROM server_allocations
                JOIN node_allocations ON node_allocations.uuid = server_allocations.allocation_uuid
                WHERE server_allocations.uuid = $1",
            )
            .bind(server_allocation)
            .fetch_one(state.database.write())
            .await
            .map_err(anyhow::Error::from)?;

            return Ok(port);
        }
        Err(crate::allocations::PortError::NoPort) => {}
        Err(crate::allocations::PortError::Unreachable(err)) => {
            return Err(BedrockError::Wings(err));
        }
        Err(crate::allocations::PortError::Other(err)) => return Err(BedrockError::Other(err)),
    }

    let node = server.node.fetch_cached(&state.database).await?;
    let ip = server
        .allocation
        .as_ref()
        .map(|allocation| allocation.allocation.ip);

    let ips: Vec<sqlx::types::ipnetwork::IpNetwork> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT node_allocations.ip
        FROM node_allocations
        WHERE node_allocations.node_uuid = $1 AND ($2 IS NULL OR node_allocations.ip = $2)
        "#,
    )
    .bind(node.uuid)
    .bind(ip)
    .fetch_all(state.database.read())
    .await
    .map_err(anyhow::Error::from)?;
    let exclude = NodeAllocation::used_by_node(
        state,
        &node,
        &ips.iter().map(|ip| ip.ip()).collect::<Vec<_>>(),
    )
    .await
    .map_err(BedrockError::Wings)?;

    let row = sqlx::query(
        r#"
        INSERT INTO server_allocations (server_uuid, allocation_uuid, notes)
        VALUES ($1, (
            SELECT node_allocations.uuid FROM node_allocations
            LEFT JOIN server_allocations ON server_allocations.allocation_uuid = node_allocations.uuid
            WHERE
                node_allocations.node_uuid = $2
                AND ($3 IS NULL OR node_allocations.ip = $3)
                AND server_allocations.uuid IS NULL
                AND NOT (node_allocations.uuid = ANY($4))
            ORDER BY node_allocations.port
            LIMIT 1
        ), $5)
        RETURNING allocation_uuid
        "#,
    )
    .bind(server.uuid)
    .bind(node.uuid)
    .bind(ip)
    .bind(&exclude)
    .bind(NOTE)
    .fetch_one(state.database.write())
    .await;

    let allocation_uuid: uuid::Uuid = match row {
        Ok(row) => row.get("allocation_uuid"),
        Err(err) if err.is_not_null_violation() => {
            return Err(BedrockError::NoPort);
        }
        Err(err) => return Err(BedrockError::Other(err.into())),
    };

    let port: i32 = sqlx::query_scalar("SELECT port FROM node_allocations WHERE uuid = $1")
        .bind(allocation_uuid)
        .fetch_one(state.database.read())
        .await
        .map_err(anyhow::Error::from)?;

    Ok(port)
}

async fn listing(
    client: &wings_api::client::WingsClient,
    server: &Server,
    directory: &str,
) -> Result<Vec<String>, anyhow::Error> {
    match client
        .get_servers_server_files_list(
            server.uuid,
            &wings_api::servers_server_files_list::get::Query {
                directory: Some(format!("/{directory}").into()),
                per_page: Some(1000),
                ..Default::default()
            },
        )
        .await
    {
        Ok(list) => Ok(list
            .entries
            .into_iter()
            .filter(|entry| entry.file)
            .map(|entry| entry.name.to_string())
            .collect()),
        Err(wings_api::client::ApiHttpError::Http(status, _))
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            Ok(Vec::new())
        }
        Err(err) => Err(err.into()),
    }
}

/// `bedrock.port` of an existing Geyser config.
fn configured_port(config: &str) -> Option<i32> {
    let mut in_bedrock = false;

    for line in config.lines() {
        if !line.starts_with([' ', '\t']) {
            in_bedrock = line.trim_end() == "bedrock:";
            continue;
        }
        if in_bedrock && let Some(port) = line.trim().strip_prefix("port:") {
            return port.split('#').next()?.trim().parse().ok();
        }
    }

    None
}

/// A short Geyser config: the Bedrock port, the Java server and how Bedrock players sign in.
/// Geyser fills in everything else with its defaults on start.
fn new_config(bedrock_port: i32, java_port: i32, auth_type: &str) -> String {
    format!(
        "bedrock:\n  port: {bedrock_port}\nremote:\n  address: auto\n  port: {java_port}\n  auth-type: {auth_type}\n"
    )
}

/// What Bedrock support looks like on the server now.
pub async fn status(state: &crate::State, server: &Server) -> Result<Bedrock, BedrockError> {
    let software = super::software(state, server).await?;
    let Some(platform) = platform(software.loader) else {
        return Ok(Bedrock {
            available: false,
            installed: None,
            port: None,
            address: None,
            running: false,
        });
    };
    let directory = software.loader.map(Loader::directory).unwrap_or("plugins");

    let client = files::client(state, server).await?;
    let installed = listing(&client, server, directory)
        .await
        .map_err(BedrockError::Wings)?
        .into_iter()
        .find(|name| is_geyser(name));
    let config = files::read_text(&client, server, config_path(platform), 256 * 1024)
        .await
        .ok()
        .flatten();
    let held = held_allocation(state, server, config.as_deref().and_then(configured_port)).await?;

    Ok(Bedrock {
        available: true,
        installed,
        port: held.as_ref().map(|held| held.allocation.port),
        address: held.as_ref().map(|held| address(&held.allocation)),
        running: !matches!(
            files::power_state(&client, server).await,
            Some(wings_api::ServerState::Offline)
        ),
    })
}

/// Installs or updates Geyser and Floodgate, holds a port for Bedrock and restarts a running
/// server so it loads them.
pub async fn install(state: &crate::State, server: &Server) -> Result<Installed, BedrockError> {
    let software = super::software(state, server).await?;
    let Some(platform) = platform(software.loader) else {
        return Err(BedrockError::Unsupported);
    };
    let loader = software.loader.ok_or(BedrockError::Unsupported)?;
    let directory = loader.directory();
    let client = files::client(state, server).await?;

    // the port first: without one, Bedrock players could not reach Geyser anyway
    let config = files::read_text(&client, server, config_path(platform), 256 * 1024)
        .await
        .ok()
        .flatten();
    let lock = state
        .cache
        .lock(
            format!("servers::{}::allocations", server.uuid),
            Some(30),
            Some(5000),
        )
        .await?;
    let port =
        match held_allocation(state, server, config.as_deref().and_then(configured_port)).await? {
            Some(held) => {
                if held.notes.as_deref() != Some(NOTE) {
                    sqlx::query("UPDATE server_allocations SET notes = $2 WHERE uuid = $1")
                        .bind(held.uuid)
                        .bind(NOTE)
                        .execute(state.database.write())
                        .await
                        .map_err(anyhow::Error::from)?;
                }
                held.allocation.port
            }
            None => claim_allocation(state, server).await?,
        };
    drop(lock);

    let held = held_allocation(state, server, Some(port))
        .await?
        .ok_or_else(|| anyhow::anyhow!("the bedrock allocation is gone"))?;

    // an update replaces the jars instead of leaving two of each
    let existing = listing(&client, server, directory)
        .await
        .map_err(BedrockError::Wings)?;
    let old: Vec<compact_str::CompactString> = existing
        .iter()
        .filter(|name| is_geyser(name) || is_floodgate(name))
        .map(|name| name.as_str().into())
        .collect();
    let updated = existing.iter().any(|name| is_geyser(name));
    if !old.is_empty() {
        client
            .post_servers_server_files_delete(
                server.uuid,
                &wings_api::servers_server_files_delete::post::RequestBody {
                    root: format!("/{directory}").into(),
                    files: old,
                },
            )
            .await
            .map_err(|err| BedrockError::Wings(err.into()))?;
    }

    content::pull(
        state,
        server,
        &format!("{GEYSERMC}/geyser/versions/latest/builds/latest/downloads/{platform}"),
        &jar_name(platform),
        directory,
    )
    .await
    .map_err(BedrockError::Content)?;

    let floodgate = if platform == "spigot" {
        Some((
            format!("{GEYSERMC}/floodgate/versions/latest/builds/latest/downloads/spigot"),
            "floodgate-spigot.jar".to_string(),
        ))
    } else {
        let game_version = software.game_version(state).await;
        modrinth::versions(
            state,
            "floodgate",
            loader.modrinth_loaders(),
            game_version.as_deref(),
        )
        .await
        .ok()
        .and_then(|versions| versions.into_iter().find_map(|version| version.file))
        .and_then(|file| {
            let url = file.url?;
            super::plain_file_name(&file.name).then(|| (url.to_string(), file.name.to_string()))
        })
    };
    if let Some((url, name)) = &floodgate {
        content::pull(state, server, url, name, directory)
            .await
            .map_err(BedrockError::Content)?;
    }

    let java_port = server
        .allocation
        .as_ref()
        .map(|allocation| allocation.allocation.port)
        .unwrap_or(25565);
    let path = config_path(platform);
    let auth_type = if floodgate.is_some() {
        "floodgate"
    } else {
        "online"
    };
    // newer Geyser configs call the Java section `java`, older ones `remote`
    let written = config
        .as_deref()
        .and_then(|config| {
            ["remote", "java"].iter().find_map(|section| {
                let port_key = format!("{section}.port");
                let auth_key = format!("{section}.auth-type");

                super::config::set_yaml(
                    config,
                    &[
                        ("bedrock.port", serde_json::json!(port)),
                        (port_key.as_str(), serde_json::json!(java_port)),
                        (auth_key.as_str(), serde_json::json!(auth_type)),
                    ],
                )
            })
        })
        .unwrap_or_else(|| new_config(port, java_port, auth_type));
    // Wings makes the folders a written file needs
    files::write_text(&client, server, path, written)
        .await
        .map_err(BedrockError::Wings)?;

    let restarted = !matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Offline) | None
    );
    if restarted {
        // the downloads run in the background; give them a moment before the jars are loaded
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        client
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Restart,
                    wait_seconds: None,
                },
            )
            .await
            .map_err(|err| BedrockError::Wings(err.into()))?;
    }

    Ok(Installed {
        updated,
        floodgate: floodgate.is_some(),
        port,
        address: address(&held.allocation),
        restarted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_and_files() {
        assert_eq!(platform(Some(Loader::Paper)), Some("spigot"));
        assert_eq!(platform(Some(Loader::Forge)), None);
        assert_eq!(platform(None), None);
        assert_eq!(jar_name("neoforge"), "Geyser-Neoforge.jar");
        assert!(is_geyser("Geyser-Spigot.jar"));
        assert!(!is_geyser("Geyser-Spigot.jar.disabled"));
        assert!(is_floodgate("floodgate-fabric-2.2.4.jar"));
    }

    #[test]
    fn the_configured_port_is_found() {
        let config = "# Geyser\nbedrock:\n  address: 0.0.0.0\n  port: 19133 # bedrock\nremote:\n  port: 25565\n";
        assert_eq!(configured_port(config), Some(19133));
        assert_eq!(configured_port("remote:\n  port: 25565\n"), None);
        assert_eq!(
            configured_port(&new_config(19140, 25565, "floodgate")),
            Some(19140)
        );
        assert!(new_config(19140, 25565, "floodgate").contains("auth-type: floodgate"));
    }
}
