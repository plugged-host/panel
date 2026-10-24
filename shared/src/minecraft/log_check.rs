//! plugged.host: the console's automatic log check (docs/features/minecraft.md), the way Aternos
//! does it: the server's `logs/latest.log` goes to mclo.gs and the problems it recognises show
//! above the console.
//!
//! Secrets are masked before the log leaves the panel; mclo.gs strips IP addresses itself. One
//! result per server, shared by everyone watching its console. A log is uploaded only when it
//! changed, a running server's at most every two minutes and a stopped server's right away
//! (that is the log with the crash in it).

use super::files;
use crate::models::server::Server;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const API: &str = "https://api.mclo.gs/1";
const SITE: &str = "https://mclo.gs/";
const LATEST_LOG: &str = "logs/latest.log";
/// Bigger logs are not checked (mclo.gs takes 10 MB; a log this long is days of a session).
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;
/// Seconds between uploads of a running server's log.
const UPLOAD_EVERY: i64 = 120;
const KEEP: u64 = 24 * 60 * 60;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// Problems shown, and how long their texts may be.
const MAX_PROBLEMS: usize = 10;
const MAX_TEXT: usize = 300;

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Problem {
    pub message: String,
    /// mclo.gs's first suggested fix.
    pub solution: Option<String>,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct LogCheck {
    pub id: String,
    pub url: String,
    /// Error lines in the log.
    pub errors: u64,
    pub problems: Vec<Problem>,
    pub checked: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    result: LogCheck,
    hash: String,
}

/// Whether logs are checked at all.
pub async fn enabled(state: &crate::State) -> bool {
    state
        .settings
        .get()
        .await
        .map(|settings| settings.games.minecraft_log_check)
        .unwrap_or(false)
}

fn key(server: &Server) -> String {
    format!("minecraft::log_check::{}", server.uuid)
}

async fn last(state: &crate::State, server: &Server) -> Option<Stored> {
    state.cache.get::<Stored>(&key(server)).await.ok().flatten()
}

/// Checks the log when it is due and changed; answers the latest result either way.
pub async fn check(
    state: &crate::State,
    server: &Server,
) -> Result<Option<LogCheck>, anyhow::Error> {
    let last = last(state, server).await;
    if !enabled(state).await {
        return Ok(last.map(|stored| stored.result));
    }

    let client = files::client(state, server).await?;
    let running = !matches!(
        files::power_state(&client, server).await,
        Some(wings_api::ServerState::Offline)
    );
    if running
        && let Some(last) = &last
        && (chrono::Utc::now() - last.result.checked).num_seconds() < UPLOAD_EVERY
    {
        return Ok(Some(last.result.clone()));
    }

    // one upload at a time per server, however many consoles are open
    let Ok(_lock) = state
        .cache
        .lock(
            format!("minecraft::log_check::{}::lock", server.uuid),
            Some(30),
            Some(0),
        )
        .await
    else {
        return Ok(last.map(|stored| stored.result));
    };

    let log = match files::read_text(&client, server, LATEST_LOG, MAX_LOG_BYTES).await {
        Ok(Some(log)) => crate::redact::redact(&crate::redact::strip_ansi(&log)),
        Ok(None) => return Ok(last.map(|stored| stored.result)),
        Err(err) => {
            tracing::debug!(server = %server.uuid, "log check could not read the log: {err:#}");
            return Ok(last.map(|stored| stored.result));
        }
    };
    if log.trim().is_empty() {
        return Ok(last.map(|stored| stored.result));
    }

    let hash = format!("{:x}", md5::compute(log.as_bytes()));
    if let Some(mut last) = last
        && last.hash == hash
    {
        last.result.checked = chrono::Utc::now();
        state.cache.set(&key(server), KEEP, &last).await.ok();

        return Ok(Some(last.result));
    }

    let (id, mut errors) = upload(state, &log).await?;
    let problems = match insights(state, &id).await {
        Ok((problems, counted)) => {
            errors = errors.max(counted);
            problems
        }
        // the analysis is a bonus; the upload and its error count still stand
        Err(err) => {
            tracing::debug!("mclo.gs insights failed: {err:#}");
            Vec::new()
        }
    };

    let stored = Stored {
        result: LogCheck {
            url: format!("{SITE}{id}"),
            id,
            errors,
            problems,
            checked: chrono::Utc::now(),
        },
        hash,
    };
    state.cache.set(&key(server), KEEP, &stored).await.ok();

    Ok(Some(stored.result))
}

/// A paste id as mclo.gs gives it out.
fn valid_id(id: &str) -> bool {
    (3..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

async fn upload(state: &crate::State, log: &str) -> Result<(String, u64), anyhow::Error> {
    let response = state
        .client
        .post(format!("{API}/log"))
        .timeout(TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .header("Accept", "application/json")
        .form(&[("content", log)])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();

    match body["id"].as_str() {
        Some(id) if body["success"] == true && status.is_success() && valid_id(id) => {
            Ok((id.to_string(), body["errors"].as_u64().unwrap_or(0)))
        }
        _ => anyhow::bail!(
            "mclo.gs refused the log ({status}): {}",
            body["error"].as_str().unwrap_or("no reason given")
        ),
    }
}

async fn insights(state: &crate::State, id: &str) -> Result<(Vec<Problem>, u64), anyhow::Error> {
    let body: serde_json::Value = state
        .client
        .get(format!("{API}/log/{id}?insights=1"))
        .timeout(TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok((
        problems(&body["content"]["insights"]["analysis"]["problems"]),
        body["errors"].as_u64().unwrap_or(0),
    ))
}

fn shorten(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_TEXT {
        return text.to_string();
    }

    let mut short: String = text.chars().take(MAX_TEXT - 1).collect();
    short.push('…');
    short
}

/// mclo.gs's problems as the banner shows them: what went wrong and its first suggested fix.
fn problems(value: &serde_json::Value) -> Vec<Problem> {
    value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|problem| {
            let message = problem["message"]
                .as_str()
                .filter(|m| !m.trim().is_empty())?;
            let solution = problem["solutions"]
                .as_array()
                .and_then(|solutions| {
                    solutions
                        .iter()
                        .filter_map(|solution| solution["message"].as_str())
                        .find(|message| !message.trim().is_empty())
                })
                .map(shorten);

            Some(Problem {
                message: shorten(message),
                solution,
            })
        })
        .take(MAX_PROBLEMS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problems_take_the_first_fix() {
        let value = serde_json::json!([
            {
                "message": "The mod 'create' needs Minecraft 1.20.1.",
                "solutions": [{ "message": "" }, { "message": "Install Minecraft 1.20.1." }]
            },
            { "message": "  " },
            { "message": "Out of memory.", "solutions": [] },
        ]);

        assert_eq!(
            problems(&value),
            vec![
                Problem {
                    message: "The mod 'create' needs Minecraft 1.20.1.".into(),
                    solution: Some("Install Minecraft 1.20.1.".into()),
                },
                Problem {
                    message: "Out of memory.".into(),
                    solution: None,
                },
            ]
        );
        assert!(problems(&serde_json::Value::Null).is_empty());
        assert_eq!(shorten(&"a".repeat(400)).chars().count(), MAX_TEXT);
    }

    #[test]
    fn paste_ids() {
        assert!(valid_id("8FlTowW"));
        assert!(!valid_id("../x"));
        assert!(!valid_id("ab"));
    }
}
