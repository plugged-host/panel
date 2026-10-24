//! plugged.host: the server.cfg txAdmin runs, as a form (docs/features/fivem.md).
//!
//! The file is the one txAdmin points at ([`super::txadmin::cfg_path`]), not a guess at
//! `/server.cfg`. Only what changed is written, each change an edit of the line holding it
//! ([`super::server_cfg`]), so comments, order and unknown commands survive a save. A save names
//! the file's hash as loaded and is refused when the file changed since (txAdmin's own editor, the
//! file manager, a framework install): the line numbers its rows carry would no longer be true.
//!
//! Endpoints and OneSync are shown, not edited: the panel owns the port and txAdmin owns OneSync
//! (it comments out any onesync line it finds).

use super::server_cfg::{Ace, Connection, Principal, Resource, ServerCfg, Style, tags};
use crate::{minecraft::files, models::server::Server};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const MAX_SIZE: u64 = 4 * 1024 * 1024;

#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Settings {
    #[garde(length(chars, max = 255))]
    pub hostname: String,
    #[garde(length(chars, max = 255))]
    pub project_name: String,
    #[garde(length(chars, max = 255))]
    pub project_description: String,
    #[garde(length(chars, max = 8))]
    pub max_clients: String,
    #[garde(length(chars, max = 32))]
    pub locale: String,
    #[garde(length(chars, max = 255))]
    pub icon: String,
    #[garde(length(chars, max = 16))]
    pub game_build: String,
    #[garde(length(chars, max = 255))]
    pub license_key: String,
    #[garde(length(chars, max = 255))]
    pub steam_key: String,
    #[garde(length(chars, max = 255))]
    pub tebex_secret: String,
    #[garde(length(max = 32), inner(length(chars, max = 64)))]
    pub tags: Vec<String>,
    #[garde(skip)]
    pub database: Connection,
}

/// The settings the form edits: the cfg name, how a new line is written, and whether a new value
/// is quoted (numbers read better bare).
const FIELDS: &[(&str, Style, bool)] = &[
    ("sv_hostname", Style::Command, true),
    ("sv_projectName", Style::Sets, true),
    ("sv_projectDesc", Style::Sets, true),
    ("sv_maxclients", Style::Command, false),
    ("locale", Style::Sets, true),
    ("load_server_icon", Style::Command, false),
    ("sv_enforceGameBuild", Style::Set, false),
    ("sv_licenseKey", Style::Command, true),
    ("steam_webApiKey", Style::Set, true),
    ("sv_tebexSecret", Style::Command, true),
];

impl Settings {
    fn values(&self) -> [&str; 10] {
        [
            &self.hostname,
            &self.project_name,
            &self.project_description,
            &self.max_clients,
            &self.locale,
            &self.icon,
            &self.game_build,
            &self.license_key,
            &self.steam_key,
            &self.tebex_secret,
        ]
    }

    pub fn read(cfg: &ServerCfg) -> Self {
        let get = |name: &str| cfg.get(name).unwrap_or_default();

        Self {
            hostname: get("sv_hostname"),
            project_name: get("sv_projectName"),
            project_description: get("sv_projectDesc"),
            max_clients: get("sv_maxclients"),
            locale: get("locale"),
            icon: get("load_server_icon"),
            game_build: get("sv_enforceGameBuild"),
            license_key: get("sv_licenseKey"),
            steam_key: get("steam_webApiKey"),
            tebex_secret: get("sv_tebexSecret"),
            tags: tags(cfg.get("tags").as_deref()),
            database: Connection::parse(&get("mysql_connection_string")),
        }
    }

    /// Why the values cannot be written, if they cannot.
    fn invalid(&self) -> Option<&'static str> {
        let max_clients = self.max_clients.trim();
        if !max_clients.is_empty()
            && !max_clients
                .parse::<u32>()
                .is_ok_and(|clients| (1..=2048).contains(&clients))
        {
            return Some("the player slots are a number from 1 to 2048");
        }
        let game_build = self.game_build.trim();
        if !game_build.is_empty() && !game_build.bytes().all(|b| b.is_ascii_digit()) {
            return Some("the game build is a number");
        }
        let port = self.database.port.trim();
        if !port.is_empty() && !port.parse::<u16>().is_ok_and(|port| port > 0) {
            return Some("the database port is a number from 1 to 65535");
        }

        None
    }
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct Config {
    /// server.cfg's path from the server's root; `null` while txAdmin is not set up (then
    /// nothing else is filled).
    pub path: Option<String>,
    /// What a save names to say which file it edited.
    pub hash: Option<String>,
    pub endpoints: Vec<String>,
    pub redm: bool,
    pub settings: Settings,
    pub resources: Vec<Resource>,
    pub principals: Vec<Principal>,
    pub aces: Vec<Ace>,
}

#[derive(ToSchema, Validate, Deserialize, Debug, Clone)]
pub struct Save {
    #[garde(length(chars, min = 1, max = 64))]
    pub hash: String,
    #[garde(dive)]
    pub settings: Settings,
    #[garde(length(max = 4096))]
    pub resources: Vec<Resource>,
    #[garde(length(max = 4096))]
    pub principals: Vec<Principal>,
    #[garde(length(max = 4096))]
    pub aces: Vec<Ace>,
}

#[derive(Debug)]
pub enum ConfigError {
    NotSetUp,
    Changed,
    Invalid(&'static str),
    InvalidRow(&'static str, usize),
    Wings(anyhow::Error),
}

impl ConfigError {
    pub fn response(self) -> crate::response::ApiResponse {
        use axum::http::StatusCode;

        let (message, status) = match self {
            Self::NotSetUp => (
                "txadmin has not set up a server yet: finish its setup in the console first".into(),
                StatusCode::CONFLICT,
            ),
            Self::Changed => (
                "server.cfg changed since the page loaded it: reload and make the change again"
                    .into(),
                StatusCode::CONFLICT,
            ),
            Self::Invalid(message) => (message.to_string(), StatusCode::BAD_REQUEST),
            Self::InvalidRow(list, row) => (
                format!("row {row} of {list} is not a valid name"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Wings(err) => {
                tracing::warn!("fivem server.cfg: {err:#}");
                (
                    "the server's files could not be reached, try again in a moment".into(),
                    StatusCode::BAD_GATEWAY,
                )
            }
        };

        crate::response::ApiResponse::error(message).with_status(status)
    }
}

fn hash(raw: &str) -> String {
    use sha2::Digest;

    let digest = sha2::Sha256::digest(raw.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn path(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<Option<String>, ConfigError> {
    let config = super::txadmin::config(client, server)
        .await
        .map_err(ConfigError::Wings)?;

    Ok(super::txadmin::cfg_path(config.as_ref()))
}

fn config(path: Option<String>, raw: Option<&str>) -> Config {
    let (Some(path), Some(raw)) = (path, raw) else {
        return Config {
            path: None,
            hash: None,
            endpoints: Vec::new(),
            redm: false,
            settings: Settings::default(),
            resources: Vec::new(),
            principals: Vec::new(),
            aces: Vec::new(),
        };
    };
    let cfg = ServerCfg::new(raw);

    Config {
        path: Some(path),
        hash: Some(hash(raw)),
        endpoints: cfg.endpoints(),
        redm: cfg.is_redm(),
        settings: Settings::read(&cfg),
        resources: cfg.resources(),
        principals: cfg.principals(),
        aces: cfg.aces(),
    }
}

pub async fn load(state: &crate::State, server: &Server) -> Result<Config, ConfigError> {
    let client = files::client(state, server)
        .await
        .map_err(ConfigError::Wings)?;
    let Some(path) = path(&client, server).await? else {
        return Ok(config(None, None));
    };
    let raw = files::read_text(&client, server, &path, MAX_SIZE)
        .await
        .map_err(ConfigError::Wings)?;

    Ok(config(Some(path), raw.as_deref()))
}

/// The file with the form's changes made, and the names of the settings and lists changed;
/// `None` when nothing changed.
pub fn apply(raw: &str, save: &Save) -> Result<Option<(String, Vec<&'static str>)>, ConfigError> {
    if let Some(message) = save.settings.invalid() {
        return Err(ConfigError::Invalid(message));
    }
    let lists: [(&'static str, Vec<(usize, bool)>); 3] = [
        (
            "resources",
            save.resources
                .iter()
                .enumerate()
                .map(|(row, r)| (row, super::server_cfg::valid_name(r.name.trim())))
                .collect(),
        ),
        (
            "group memberships",
            save.principals
                .iter()
                .enumerate()
                .map(|(row, p)| {
                    (
                        row,
                        super::server_cfg::valid_principal(p.child.trim())
                            && super::server_cfg::valid_principal(p.parent.trim()),
                    )
                })
                .collect(),
        ),
        (
            "access control",
            save.aces
                .iter()
                .enumerate()
                .map(|(row, a)| {
                    (
                        row,
                        super::server_cfg::valid_principal(a.principal.trim())
                            && super::server_cfg::valid_principal(a.object.trim()),
                    )
                })
                .collect(),
        ),
    ];
    for (list, rows) in &lists {
        if let Some((row, _)) = rows.iter().find(|(_, valid)| !valid) {
            return Err(ConfigError::InvalidRow(list, row + 1));
        }
    }

    let mut cfg = ServerCfg::new(raw);
    let loaded = Settings::read(&cfg);
    let mut changed = Vec::new();

    for ((name, style, quote), (now, before)) in FIELDS
        .iter()
        .zip(save.settings.values().into_iter().zip(loaded.values()))
    {
        if now.trim() != before.trim() {
            cfg.set(name, now.trim(), *style, *quote);
            changed.push(*name);
        }
    }
    let now_tags: Vec<String> = save
        .settings
        .tags
        .iter()
        .map(|tag| tag.trim().replace(',', " "))
        .filter(|tag| !tag.is_empty())
        .collect();
    if now_tags != loaded.tags {
        cfg.set("tags", &now_tags.join(", "), Style::Sets, true);
        changed.push("tags");
    }
    if save.settings.database != loaded.database {
        cfg.set(
            "mysql_connection_string",
            &save.settings.database.compose(),
            Style::Set,
            true,
        );
        changed.push("mysql_connection_string");
    }

    let same = |a: &[Resource], b: &[Resource]| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| a.key == b.key && a.name.trim() == b.name && a.enabled == b.enabled)
    };
    if !same(&save.resources, &cfg.resources()) {
        cfg.set_resources(&save.resources);
        changed.push("resources");
    }
    if save.principals != cfg.principals() {
        cfg.set_principals(&save.principals);
        changed.push("principals");
    }
    if save.aces != cfg.aces() {
        cfg.set_aces(&save.aces);
        changed.push("aces");
    }

    let rendered = cfg.render();
    if rendered == raw {
        return Ok(None);
    }

    Ok(Some((rendered, changed)))
}

/// Writes the form's changes; answers the file as now and what changed (empty: nothing did).
pub async fn save(
    state: &crate::State,
    server: &Server,
    save: &Save,
) -> Result<(Config, Vec<&'static str>), ConfigError> {
    let client = files::client(state, server)
        .await
        .map_err(ConfigError::Wings)?;
    let path = path(&client, server).await?.ok_or(ConfigError::NotSetUp)?;
    let raw = files::read_text(&client, server, &path, MAX_SIZE)
        .await
        .map_err(ConfigError::Wings)?
        .ok_or(ConfigError::NotSetUp)?;
    if hash(&raw) != save.hash {
        return Err(ConfigError::Changed);
    }

    let Some((rendered, changed)) = apply(&raw, save)? else {
        return Ok((config(Some(path), Some(&raw)), Vec::new()));
    };
    files::write_text(&client, server, &path, rendered.clone())
        .await
        .map_err(ConfigError::Wings)?;

    Ok((config(Some(path), Some(&rendered)), changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str =
        "sv_hostname \"Old\"\nsv_maxclients 32\nensure chat\nadd_ace group.admin command allow\n";

    fn save_of(raw: &str) -> Save {
        let config = config(Some("server.cfg".into()), Some(raw));
        Save {
            hash: config.hash.unwrap(),
            settings: config.settings,
            resources: config.resources,
            principals: config.principals,
            aces: config.aces,
        }
    }

    #[test]
    fn nothing_changed_writes_nothing() {
        assert!(apply(RAW, &save_of(RAW)).unwrap().is_none());
    }

    #[test]
    fn only_changed_values_are_written() {
        let mut save = save_of(RAW);
        save.settings.hostname = "New".into();
        save.settings.tags = vec!["roleplay".into(), "esx".into()];
        save.settings.database.host = "db.example.com".into();
        save.settings.database.name = "esx".into();
        let (out, changed) = apply(RAW, &save).unwrap().unwrap();
        assert_eq!(
            changed,
            vec!["sv_hostname", "tags", "mysql_connection_string"]
        );
        assert_eq!(
            out,
            "sv_hostname \"New\"\nsv_maxclients 32\nsets tags \"roleplay, esx\"\nset mysql_connection_string \"mysql://db.example.com/esx?charset=utf8mb4\"\nensure chat\nadd_ace group.admin command allow\n"
        );
    }

    #[test]
    fn bad_values_are_refused() {
        let mut save = save_of(RAW);
        save.settings.max_clients = "5000".into();
        assert!(matches!(apply(RAW, &save), Err(ConfigError::Invalid(_))));

        let mut save = save_of(RAW);
        save.resources.push(Resource {
            key: None,
            name: "bad name".into(),
            enabled: true,
        });
        assert!(matches!(
            apply(RAW, &save),
            Err(ConfigError::InvalidRow("resources", 2))
        ));
    }
}
