//! plugged.host: txAdmin's view of a FiveM server: where its server data (and so server.cfg)
//! lives, and who its master admin is (docs/features/fivem.md).
//!
//! The egg runs FXServer in txAdmin's manager mode, so there is no server.cfg at the root: txAdmin
//! deploys a recipe into `txData/<Name>_<HEX>.base/` (or is pointed at any folder) and records it
//! as `server.dataPath` in `txData/default/config.json`. That file had three shapes: version 1
//! keeps `fxRunner.serverDataPath` / `fxRunner.cfgPath`, versions 2 and 3 `server.dataPath` /
//! `cfgPath`. Its paths are container paths (`/home/container/...`); the file API wants them
//! relative to the server's root.

use crate::{minecraft::files, models::server::Server};

pub const CONFIG: &str = "txData/default/config.json";
pub const ADMINS: &str = "txData/admins.json";
const ROOT: &str = "/home/container";
const MAX_SIZE: u64 = 1024 * 1024;

/// Collapses `.` and `..` without ever leaving the root.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }

    parts.join("/")
}

/// A container path as a path relative to the server's root.
pub fn relative(path: &str) -> String {
    let path = path.trim().replace('\\', "/");

    normalize(path.strip_prefix(ROOT).unwrap_or(&path))
}

/// txAdmin's config, `None` until txAdmin wrote one.
pub async fn config(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<Option<serde_json::Value>, anyhow::Error> {
    Ok(files::read_text(client, server, CONFIG, MAX_SIZE)
        .await?
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .filter(serde_json::Value::is_object))
}

/// The server data folder relative to the root (`txData/ESXLegacy_1A2B3C.base`), `None` while
/// txAdmin is not set up.
pub fn data_path(config: Option<&serde_json::Value>) -> Option<String> {
    let config = config?;
    let path = config
        .pointer("/server/dataPath")
        .or_else(|| config.pointer("/fxRunner/serverDataPath"))?
        .as_str()?;

    (!path.is_empty()).then(|| relative(path))
}

/// server.cfg's path relative to the root, `None` while txAdmin is not set up.
pub fn cfg_path(config: Option<&serde_json::Value>) -> Option<String> {
    let data = data_path(config)?;
    let cfg = config
        .and_then(|config| {
            config
                .pointer("/server/cfgPath")
                .or_else(|| config.pointer("/fxRunner/cfgPath"))
        })
        .and_then(serde_json::Value::as_str)
        .filter(|cfg| !cfg.is_empty())
        .unwrap_or("server.cfg");

    // absolute is a container path; anything else is relative to the data folder
    let path = if cfg.starts_with('/') {
        relative(cfg)
    } else {
        normalize(&format!("{data}/{cfg}"))
    };

    (!path.is_empty()).then_some(path)
}

/// txAdmin's config pointed at a new server data folder, every other setting kept.
pub fn pointed_at(
    config: Option<serde_json::Value>,
    data_path: &str,
    onesync: Option<&str>,
) -> serde_json::Value {
    let mut config = config
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({ "version": 2 }));
    let object = config.as_object_mut().expect("an object");
    object.entry("version").or_insert(2.into());
    let folder = format!("{ROOT}/{}/", data_path.trim_matches('/'));

    if object.contains_key("fxRunner") && !object.contains_key("server") {
        // version 1: txAdmin migrates it on boot, so write where it reads
        if let Some(runner) = object["fxRunner"].as_object_mut() {
            runner.insert("serverDataPath".into(), folder.into());
            runner.insert("cfgPath".into(), "server.cfg".into());
        }
    } else {
        if !object
            .get("server")
            .is_some_and(serde_json::Value::is_object)
        {
            object.insert("server".into(), serde_json::json!({}));
        }
        let server = object["server"].as_object_mut().expect("an object");
        server.insert("dataPath".into(), folder.into());
        server.remove("cfgPath");
        if let Some(onesync) = onesync {
            server.insert("onesync".into(), onesync.into());
        }
    }

    config
}

/// `add_principal` lines for txAdmin's master admin, the way txAdmin's deployer writes
/// `{{addPrincipalsMaster}}`: one per linked identity (FiveM account, Discord).
pub fn master_principals(admins: &serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    for admin in admins.as_array().into_iter().flatten() {
        if admin.get("master").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        let name: String = admin
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
            .collect();

        for provider in ["citizenfx", "discord"] {
            let Some(identifier) = admin
                .pointer(&format!("/providers/{provider}/identifier"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let valid = identifier.split_once(':').is_some_and(|(kind, id)| {
                !kind.is_empty()
                    && kind.bytes().all(|b| b.is_ascii_lowercase())
                    && !id.is_empty()
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            });
            if valid {
                lines.push(format!(
                    "add_principal identifier.{identifier} group.admin #{name}"
                ));
            }
        }
    }

    lines
}

/// txAdmin's admins file, `null` when it has none yet.
pub async fn admins(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<serde_json::Value, anyhow::Error> {
    Ok(files::read_text(client, server, ADMINS, MAX_SIZE)
        .await?
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_root() {
        assert_eq!(
            relative("/home/container/txData/ESX_1.base/"),
            "txData/ESX_1.base"
        );
        assert_eq!(normalize("a/./b/../../../c"), "c");
        assert_eq!(normalize("\\x\\y"), "x/y");
    }

    #[test]
    fn cfg_path_reads_every_shape() {
        let v2 = serde_json::json!({"version": 2, "server": {"dataPath": "/home/container/txData/QBox_AB12CD.base/"}});
        assert_eq!(
            cfg_path(Some(&v2)).as_deref(),
            Some("txData/QBox_AB12CD.base/server.cfg")
        );
        let v1 = serde_json::json!({"fxRunner": {"serverDataPath": "/home/container/data", "cfgPath": "/home/container/data/custom.cfg"}});
        assert_eq!(cfg_path(Some(&v1)).as_deref(), Some("data/custom.cfg"));
        assert_eq!(cfg_path(Some(&serde_json::json!({}))), None);
        assert_eq!(cfg_path(None), None);
    }

    #[test]
    fn pointing_keeps_other_settings() {
        let v2 = serde_json::json!({"version": 3, "server": {"dataPath": "/old/", "cfgPath": "x.cfg", "quiet": true}, "general": {"serverName": "s"}});
        let pointed = pointed_at(Some(v2), "txData/New_1.base", Some("on"));
        assert_eq!(
            pointed["server"]["dataPath"],
            "/home/container/txData/New_1.base/"
        );
        assert!(pointed["server"].get("cfgPath").is_none());
        assert_eq!(pointed["server"]["quiet"], true);
        assert_eq!(pointed["server"]["onesync"], "on");
        assert_eq!(pointed["general"]["serverName"], "s");

        let fresh = pointed_at(None, "txData/A.base", None);
        assert_eq!(fresh["version"], 2);
        let v1 = pointed_at(
            Some(serde_json::json!({"fxRunner": {"cfgPath": "a.cfg"}})),
            "d",
            None,
        );
        assert_eq!(v1["fxRunner"]["serverDataPath"], "/home/container/d/");
        assert_eq!(v1["fxRunner"]["cfgPath"], "server.cfg");
    }

    #[test]
    fn master_principals_per_identity() {
        let admins = serde_json::json!([
            {"name": "Owner Name", "master": true, "providers": {
                "citizenfx": {"identifier": "fivem:271"},
                "discord": {"identifier": "discord:1234"}
            }},
            {"name": "other", "master": false, "providers": {"citizenfx": {"identifier": "fivem:9"}}},
            {"name": "bad", "master": true, "providers": {"citizenfx": {"identifier": "fivem:1 group.admin"}}}
        ]);
        assert_eq!(
            master_principals(&admins),
            vec![
                "add_principal identifier.fivem:271 group.admin #OwnerName".to_string(),
                "add_principal identifier.discord:1234 group.admin #OwnerName".to_string(),
            ]
        );
    }
}
