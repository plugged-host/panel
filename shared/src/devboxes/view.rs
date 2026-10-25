//! plugged.host: what the server's VS Code page shows (docs/features/devboxes.md): the state of
//! the devbox its person works in, and, while it runs, how to connect.

use super::{Context, Devbox, Kind, SharedRights};
use crate::models::{ByUuid, node::Node, server::Server};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardState {
    /// Devboxes are off, or this person isn't a member who may have one.
    Hidden,
    /// Offered, but the person (or the owner, for a shared one) has no membership for it.
    Locked,
    None,
    Stopped,
    /// The first start of its box on the node: Wings pulls the devbox image, which can take a
    /// few minutes.
    Creating,
    Starting,
    Stopping,
    /// Started but never reported back.
    Failed,
    /// Running, but `/server` is not there: the node doesn't allow the mount.
    MountMissing,
    Ready,
}

#[derive(ToSchema, Serialize)]
pub struct ApiDevboxConnection {
    pub host: String,
    pub port: i32,
    pub username: String,
    /// The SSH password for this start.
    pub code: Option<String>,
    pub host_key: Option<String>,
    pub ssh_command: String,
    /// Opens the devbox's `/server` in VS Code with Remote - SSH.
    pub vscode_url: String,
    pub connections: i32,
    /// VS Code in the browser is set up.
    pub web: bool,
}

#[derive(ToSchema, Serialize)]
pub struct ApiDevboxCard {
    pub state: CardState,
    pub kind: Option<Kind>,
    pub is_owner: bool,
    /// The server's shared devbox is on.
    pub shared: bool,
    /// The owner's membership allows a devbox (so they may switch the shared one on).
    pub owner_allowed: bool,
    /// What a shared devbox may do on the game server; for the owner.
    pub shared_rights: Option<SharedRights>,
    /// A stop of this devbox is this person's to make (a shared one is the owner's).
    pub can_stop: bool,
    /// Wait a few seconds and ask again: something is on its way.
    pub polling: bool,
    pub idle_minutes: i32,
    pub connection: Option<ApiDevboxConnection>,
}

impl ApiDevboxCard {
    fn hidden(context: &Context) -> Self {
        Self {
            state: CardState::Hidden,
            kind: None,
            is_owner: false,
            shared: false,
            owner_allowed: false,
            shared_rights: None,
            can_stop: false,
            polling: false,
            idle_minutes: context.devboxes.idle_minutes,
            connection: None,
        }
    }
}

/// The address people connect to: the allocation's alias, its address when that is routable,
/// else the node's public address.
pub fn connect_host(node: &Node, server: &Server) -> Option<(String, i32)> {
    let allocation = &server.allocation.as_ref()?.allocation;
    let host_of = |url: &reqwest::Url| {
        url.host_str().map(|host| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .to_string()
        })
    };
    let ip = allocation.ip.ip();

    let host = allocation
        .ip_alias
        .as_deref()
        .map(str::trim)
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .or_else(|| (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string()))
        .or_else(|| node.public_url.as_ref().and_then(host_of))
        .or_else(|| host_of(&node.url))?;

    Some((host, allocation.port))
}

/// `vscode://` link that opens `/server` over Remote - SSH.
pub fn vscode_url(host: &str, username: &str, port: i32) -> String {
    let authority = serde_json::json!({
        "hostName": host,
        "user": username,
        "port": port,
    });

    format!(
        "vscode://vscode-remote/ssh-remote+{}/server",
        hex::encode(authority.to_string())
    )
}

fn ssh_command(host: &str, username: &str, port: i32) -> String {
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };

    format!("ssh -p {port} {username}@{host}")
}

/// Where a devbox with a server stands. A start that never reported back is `Creating` while its box
/// never came up on this server (no version or host key yet: Wings is still pulling the image),
/// and failed after [`super::BOOT_MINUTES`] either way.
fn state_of(devbox: &Devbox, now: chrono::NaiveDateTime) -> CardState {
    if devbox.is_stopping() {
        CardState::Stopping
    } else if devbox.token_hash.is_none() {
        CardState::Stopped
    } else if devbox.booted.is_none() {
        if devbox
            .started
            .is_some_and(|started| started < now - chrono::Duration::minutes(super::BOOT_MINUTES))
        {
            CardState::Failed
        } else if devbox.version.is_none() && devbox.host_key.is_none() {
            CardState::Creating
        } else {
            CardState::Starting
        }
    } else if devbox.boot_status.as_deref() == Some("mount_missing") {
        CardState::MountMissing
    } else {
        CardState::Ready
    }
}

/// The card for `user_uuid` (signed in as `username`) on `game`.
pub async fn card(
    state: &crate::State,
    context: &Context,
    user_uuid: uuid::Uuid,
    username: &str,
    game: &Server,
) -> Result<ApiDevboxCard, anyhow::Error> {
    if !context.ready() || !super::is_member(&state.database, user_uuid, game).await? {
        return Ok(ApiDevboxCard::hidden(context));
    }

    let is_owner = game.owner.uuid == user_uuid;
    let shared = Devbox::shared_of(&state.database, game.uuid).await?;
    let owner_allowed = super::tier(state, context, game.owner.uuid)
        .await?
        .is_some();
    let mut card = ApiDevboxCard {
        state: CardState::Locked,
        kind: None,
        is_owner,
        shared: shared.is_some(),
        owner_allowed,
        shared_rights: shared
            .as_ref()
            .filter(|_| is_owner)
            .map(|shared| shared.shared_rights),
        can_stop: false,
        polling: false,
        idle_minutes: context.devboxes.idle_minutes,
        connection: None,
    };

    let Some((kind, devbox)) = super::resolve(state, context, user_uuid, game).await? else {
        return Ok(card);
    };
    card.kind = Some(kind);
    card.can_stop = kind == Kind::Personal || is_owner;

    let server = match devbox.as_ref().and_then(|devbox| devbox.server_uuid) {
        Some(uuid) => Server::by_uuid_optional(&state.database, uuid).await?,
        None => None,
    };
    let (Some(devbox), Some(server)) = (devbox, server) else {
        card.state = CardState::None;
        return Ok(card);
    };

    card.state = state_of(&devbox, chrono::Utc::now().naive_utc());
    card.polling = matches!(
        card.state,
        CardState::Creating | CardState::Starting | CardState::Stopping
    );
    if card.state != CardState::Ready {
        return Ok(card);
    }

    let node = server.node.fetch_cached(&state.database).await?;
    let Some((host, port)) = connect_host(&node, &server) else {
        return Ok(card);
    };
    card.connection = Some(ApiDevboxConnection {
        code: super::codes::code_for(state, &devbox, user_uuid).await?,
        host_key: devbox.host_key.clone(),
        ssh_command: ssh_command(&host, username, port),
        vscode_url: vscode_url(&host, username, port),
        connections: devbox.connections,
        web: context.web_enabled(),
        username: username.to_string(),
        host,
        port,
    });

    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vscode_link_names_host_user_and_port() {
        let url = vscode_url("node.example.com", "alice", 25570);
        let encoded = url
            .strip_prefix("vscode://vscode-remote/ssh-remote+")
            .and_then(|rest| rest.strip_suffix("/server"))
            .unwrap();
        let authority: serde_json::Value =
            serde_json::from_slice(&hex::decode(encoded).unwrap()).unwrap();

        assert_eq!(
            authority,
            serde_json::json!({"hostName": "node.example.com", "user": "alice", "port": 25570})
        );
    }

    fn devbox() -> Devbox {
        let now = chrono::Utc::now().naive_utc();

        Devbox {
            uuid: uuid::Uuid::nil(),
            game_server_uuid: uuid::Uuid::nil(),
            user_uuid: None,
            server_uuid: Some(uuid::Uuid::nil()),
            node_uuid: None,
            mount_source: None,
            slug: "box".into(),
            token_hash: Some("hash".into()),
            code_secret: None,
            stopping: None,
            boot_status: None,
            boot_detail: None,
            host_key: None,
            version: None,
            connections: 0,
            shared_rights: SharedRights::default(),
            people: Vec::new(),
            started: Some(now),
            booted: None,
            last_seen: None,
            last_used: None,
            created: now,
        }
    }

    #[test]
    fn a_first_start_is_creating_until_the_box_reports_in() {
        let now = chrono::Utc::now().naive_utc();
        let first = devbox();
        assert_eq!(state_of(&first, now), CardState::Creating);

        let again = Devbox {
            version: Some("1.0.0".into()),
            host_key: Some("SHA256:key".into()),
            ..devbox()
        };
        assert_eq!(state_of(&again, now), CardState::Starting);

        let late = now + chrono::Duration::minutes(super::super::BOOT_MINUTES + 1);
        assert_eq!(state_of(&first, late), CardState::Failed);
        assert_eq!(state_of(&again, late), CardState::Failed);

        let booted = Devbox {
            booted: Some(now),
            ..again
        };
        assert_eq!(state_of(&booted, now), CardState::Ready);
        assert_eq!(
            state_of(
                &Devbox {
                    token_hash: None,
                    ..devbox()
                },
                now
            ),
            CardState::Stopped
        );
    }

    #[test]
    fn ssh_commands_bracket_ipv6() {
        assert_eq!(
            ssh_command("node.example.com", "alice", 2022),
            "ssh -p 2022 alice@node.example.com"
        );
        assert_eq!(
            ssh_command("2001:db8::1", "alice", 2022),
            "ssh -p 2022 alice@[2001:db8::1]"
        );
    }
}
