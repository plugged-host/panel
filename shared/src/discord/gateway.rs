//! plugged.host: the Discord gateway (docs/features/discord.md): the bots' connections.
//!
//! The `discord_gateway` task runs on the primary instance only (`APP_PRIMARY`) and calls
//! [`supervise`] every few seconds: it works out which bots Admin › Settings › Accounts › Discord
//! turns on, connects one gateway session per bot token (bots sharing a token share a session,
//! their intents added up) and closes the sessions nobody wants any more. A bot that is off
//! connects nothing.
//!
//! A bot is a [`Handler`]: the intents it needs, its slash commands (each created or updated on
//! the server by name, so commands other software made on the same application stay) and what it
//! does with the events. The invite and boost bot is built in ([`super::rewards_bot`]); the
//! assistant and the moderation bot plug in with [`register`] and run on their own tokens. Each
//! session hands its events to one worker that runs them in order, so a slow handler never holds
//! up the heartbeat.
//!
//! The session speaks the gateway protocol itself (tokio-tungstenite, JSON, no compression):
//! HELLO, IDENTIFY or RESUME, heartbeats with their ACKs (a missing ACK reconnects), RECONNECT,
//! INVALID_SESSION. A refused token or intents stop the bot until its settings change; a refused
//! privileged intent (Server Members) reconnects without it, so joins and leaves are then missed.

use super::rest;
use crate::settings::discord::AppSettingsDiscord;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};
use utoipa::ToSchema;

pub const INTENT_GUILDS: u64 = 1 << 0;
/// Privileged: Server Members Intent in the Developer Portal.
pub const INTENT_GUILD_MEMBERS: u64 = 1 << 1;
pub const INTENT_GUILD_INVITES: u64 = 1 << 6;

const GATEWAY: &str = "wss://gateway.discord.gg";
const QUERY: &str = "?v=10&encoding=json";
/// How long a bot whose token or intents Discord refused waits before it tries again (settings
/// changes retry at once).
const REFUSED_RETRY: Duration = Duration::from_secs(60 * 60);
/// The cache key the bots' state is kept under, for Admin › Settings (any instance).
const STATUS_KEY: &str = "discord::gateway::status";

/// The bots the panel can run.
#[derive(
    ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum Bot {
    /// Invites and boosts (`/invites`, `/topinvites`, `/setboost`), on the bot token.
    Rewards,
    /// The assistant, on its own token.
    Assistant,
    /// Moderation, on its own token.
    Moderation,
}

impl Bot {
    pub const ALL: [Self; 3] = [Self::Rewards, Self::Assistant, Self::Moderation];

    /// The token the bot connects with, while it is on.
    fn token(self, settings: &AppSettingsDiscord) -> Option<&str> {
        let (enabled, token) = match self {
            Self::Rewards => (settings.rewards_bot_enabled, &settings.bot_token),
            Self::Assistant => (
                settings.assistant_bot_enabled,
                &settings.assistant_bot_token,
            ),
            Self::Moderation => (
                settings.moderation_bot_enabled,
                &settings.moderation_bot_token,
            ),
        };

        token
            .as_deref()
            .filter(|token| enabled && !token.is_empty())
    }

    fn handler(self) -> Option<Arc<dyn Handler>> {
        match self {
            Self::Rewards => Some(Arc::new(super::rewards_bot::RewardsBot::default())),
            bot => HANDLERS.read().get(&bot).cloned(),
        }
    }
}

/// What a bot's handler gets with every event.
#[derive(Clone)]
pub struct Session {
    pub rest: rest::Client,
    /// The bot's application, from READY.
    pub application_id: String,
    /// The bot's own user id, from READY.
    pub user_id: String,
    pub guild_id: String,
    /// The intents the session connected with (without Server Members after Discord refused it).
    pub intents: u64,
}

/// A bot: what it needs from the gateway and what it does with it.
#[async_trait::async_trait]
pub trait Handler: Send + Sync + 'static {
    /// The gateway intents it needs.
    fn intents(&self) -> u64;

    /// Its slash commands (Discord's application command objects), created or updated on the
    /// server after each READY.
    fn commands(&self) -> Vec<serde_json::Value> {
        Vec::new()
    }

    /// A dispatch event (`READY`, `GUILD_CREATE`, `INTERACTION_CREATE`, ...), in the order the
    /// gateway sent them. Events of other servers are not filtered out.
    async fn event(
        &self,
        state: &crate::State,
        session: &Session,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error>;
}

static HANDLERS: LazyLock<parking_lot::RwLock<HashMap<Bot, Arc<dyn Handler>>>> =
    LazyLock::new(Default::default);

/// Plugs a bot's handler in (the assistant's or the moderation bot's); it connects once its
/// switch and token are set.
pub fn register(bot: Bot, handler: Arc<dyn Handler>) {
    HANDLERS.write().insert(bot, handler);
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Connecting,
    Ready,
    /// Discord refused the token or the intents; tried again when the settings change.
    Refused,
}

/// Why Discord refused a bot, or what it runs without.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The token is wrong or was reset.
    Token,
    /// The intents are not allowed.
    Intents,
    /// Discord closed the connection for good (sharding, the API version).
    Closed,
    /// Connected without Server Members (off in the Developer Portal): joins and leaves are
    /// missed.
    MembersIntent,
}

impl Reason {
    fn describe(self) -> &'static str {
        match self {
            Self::Token => "Discord did not accept the token",
            Self::Intents => "Discord did not accept the intents",
            Self::Closed => "Discord closed the connection for good",
            Self::MembersIntent => "the Server Members intent is off",
        }
    }
}

/// A bot's state, for Admin › Settings.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct BotStatus {
    pub bot: Bot,
    pub phase: Phase,
    /// The bot's Discord name, once connected.
    pub username: Option<String>,
    pub reason: Option<Reason>,
    pub since: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
struct Shared {
    phase: Phase,
    username: Option<String>,
    reason: Option<Reason>,
    since: chrono::DateTime<chrono::Utc>,
}

impl Shared {
    fn new() -> Self {
        Self {
            phase: Phase::Connecting,
            username: None,
            reason: None,
            since: chrono::Utc::now(),
        }
    }
}

/// A session wanted by the settings: one token, its bots, the server.
#[derive(Clone, PartialEq, Eq)]
struct Wanted {
    token: String,
    guild_id: String,
    bots: Vec<Bot>,
}

struct Running {
    wanted: Wanted,
    shared: Arc<parking_lot::Mutex<Shared>>,
    task: tokio::task::JoinHandle<()>,
    /// When a refused session may try again.
    retry_at: Option<std::time::Instant>,
}

static SESSIONS: LazyLock<tokio::sync::Mutex<Vec<Running>>> = LazyLock::new(Default::default);

/// The sessions the settings ask for.
fn wanted(settings: &AppSettingsDiscord) -> Vec<Wanted> {
    if !settings.active() {
        return Vec::new();
    }

    let mut sessions: Vec<Wanted> = Vec::new();
    for bot in Bot::ALL {
        let Some(token) = bot.token(settings) else {
            continue;
        };
        if bot.handler().is_none() {
            continue;
        }

        match sessions.iter_mut().find(|session| session.token == token) {
            Some(session) => session.bots.push(bot),
            None => sessions.push(Wanted {
                token: token.to_string(),
                guild_id: settings.guild_id.to_string(),
                bots: vec![bot],
            }),
        }
    }

    sessions
}

/// The `discord_gateway` task: starts and stops the sessions the settings ask for, and keeps the
/// bots' state in the cache.
pub async fn supervise(state: &crate::State) -> Result<(), anyhow::Error> {
    let settings = state.settings.get().await?.discord.clone();
    let wanted = wanted(&settings);
    let mut sessions = SESSIONS.lock().await;

    // closed: no longer wanted (a bot turned off, a new token, another server)
    sessions.retain(|running| {
        let keep = wanted.contains(&running.wanted);
        if !keep {
            running.task.abort();
            tracing::info!(bots = ?running.wanted.bots, "Discord gateway session closed");
        }
        keep
    });

    let now = std::time::Instant::now();
    for running in sessions.iter_mut() {
        if running.task.is_finished() && running.retry_at.is_none() {
            running.retry_at = Some(now + REFUSED_RETRY);
        }
        if running.retry_at.is_some_and(|at| at <= now) {
            running.shared = Arc::new(parking_lot::Mutex::new(Shared::new()));
            running.task = spawn(state, running.wanted.clone(), running.shared.clone());
            running.retry_at = None;
        }
    }

    for wanted in wanted {
        if sessions.iter().any(|running| running.wanted == wanted) {
            continue;
        }

        let shared = Arc::new(parking_lot::Mutex::new(Shared::new()));
        tracing::info!(bots = ?wanted.bots, "Discord gateway session starting");
        sessions.push(Running {
            task: spawn(state, wanted.clone(), shared.clone()),
            wanted,
            shared,
            retry_at: None,
        });
    }

    let statuses: Vec<BotStatus> = sessions
        .iter()
        .flat_map(|running| {
            let shared = running.shared.lock().clone();
            running.wanted.bots.iter().map(move |bot| BotStatus {
                bot: *bot,
                phase: shared.phase,
                username: shared.username.clone(),
                reason: shared.reason,
                since: shared.since,
            })
        })
        .collect();
    drop(sessions);

    if let Err(err) = state.cache.set(STATUS_KEY, 90, &statuses).await {
        tracing::debug!("failed to store the Discord bots' state: {err:#}");
    }

    Ok(())
}

/// The bots' state as the primary instance last stored it; `None` when it did not (no bot on, or
/// no primary instance running).
pub async fn status(state: &crate::State) -> Option<Vec<BotStatus>> {
    state
        .cache
        .get::<Vec<BotStatus>>(STATUS_KEY)
        .await
        .ok()
        .flatten()
}

fn spawn(
    state: &crate::State,
    wanted: Wanted,
    shared: Arc<parking_lot::Mutex<Shared>>,
) -> tokio::task::JoinHandle<()> {
    let state = state.clone();

    tokio::spawn(async move { run(state, wanted, shared).await })
}

/// What the worker gets from the session.
enum Event {
    Ready(Session),
    Dispatch(String, serde_json::Value),
}

/// How a connection ended.
enum End {
    /// Reconnect; `resume` keeps the session.
    Reconnect { resume: bool },
    /// Discord refused the token or the intents.
    Refused(Reason),
    /// Discord refused the privileged intents: reconnect without them.
    Privileged,
}

#[derive(Clone)]
struct Resume {
    session_id: String,
    url: String,
}

#[derive(Deserialize)]
struct Payload {
    op: u8,
    #[serde(default)]
    d: serde_json::Value,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
}

/// One session: connects, reconnects (with a growing pause after failures) until it is aborted
/// or refused.
async fn run(state: crate::State, wanted: Wanted, shared: Arc<parking_lot::Mutex<Shared>>) {
    let handlers: Vec<Arc<dyn Handler>> =
        wanted.bots.iter().filter_map(|bot| bot.handler()).collect();
    let mut intents = handlers
        .iter()
        .fold(0, |intents, handler| intents | handler.intents());
    let rest = rest::Client::new(&state.client, &wanted.token);

    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(worker(state.clone(), handlers, rest.clone(), receiver));

    let mut resume: Option<Resume> = None;
    let mut sequence: Option<u64> = None;
    let mut pause = Duration::from_secs(1);
    let mut privileged_refused = false;

    loop {
        let started = std::time::Instant::now();
        let end = connect(
            &wanted,
            intents,
            &rest,
            &shared,
            &sender,
            &mut resume,
            &mut sequence,
        )
        .await;

        match end {
            Ok(End::Reconnect { resume: keep }) => {
                if !keep {
                    resume = None;
                    sequence = None;
                }
            }
            Ok(End::Privileged) => {
                tracing::warn!(
                    bots = ?wanted.bots,
                    "Discord refused the Server Members intent: connecting without it (turn it on in the Developer Portal)"
                );
                intents &= !INTENT_GUILD_MEMBERS;
                privileged_refused = true;
                resume = None;
                sequence = None;
            }
            Ok(End::Refused(reason)) => {
                tracing::error!(bots = ?wanted.bots, "Discord refused the bot: {}", reason.describe());
                let mut shared = shared.lock();
                shared.phase = Phase::Refused;
                shared.reason = Some(reason);
                shared.since = chrono::Utc::now();
                return;
            }
            Err(err) => {
                tracing::warn!(bots = ?wanted.bots, "Discord gateway connection failed: {err:#}");
            }
        }

        {
            let mut shared = shared.lock();
            if shared.phase != Phase::Connecting {
                shared.phase = Phase::Connecting;
                shared.since = chrono::Utc::now();
            }
            if privileged_refused {
                shared.reason = Some(Reason::MembersIntent);
            }
        }

        // a connection that held a while starts the pause over
        if started.elapsed() > Duration::from_secs(60) {
            pause = Duration::from_secs(1);
        }
        tokio::time::sleep(pause).await;
        pause = (pause * 2).min(Duration::from_secs(60));
    }
}

fn identify(token: &str, intents: u64) -> serde_json::Value {
    serde_json::json!({
        "op": 2,
        "d": {
            "token": token,
            "intents": intents,
            "properties": { "os": std::env::consts::OS, "browser": "calagopus", "device": "calagopus" },
        },
    })
}

/// A close code: whether to resume, to give up or to drop the privileged intents.
fn closed(code: u16) -> End {
    match code {
        4004 => End::Refused(Reason::Token),
        4010 | 4011 | 4012 => End::Refused(Reason::Closed),
        4013 => End::Refused(Reason::Intents),
        4014 => End::Privileged,
        // the session cannot be resumed
        4007 | 4009 | 1000 | 1001 => End::Reconnect { resume: false },
        _ => End::Reconnect { resume: true },
    }
}

/// One connection, until it ends.
async fn connect(
    wanted: &Wanted,
    intents: u64,
    rest: &rest::Client,
    shared: &Arc<parking_lot::Mutex<Shared>>,
    events: &tokio::sync::mpsc::UnboundedSender<Event>,
    resume: &mut Option<Resume>,
    sequence: &mut Option<u64>,
) -> Result<End, anyhow::Error> {
    let url = match resume.as_ref() {
        Some(resume) => format!("{}/{QUERY}", resume.url.trim_end_matches('/')),
        None => format!("{GATEWAY}/{QUERY}"),
    };
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(30),
        tokio_tungstenite::connect_async(url),
    )
    .await??;
    let (mut sink, mut stream) = socket.split();

    // HELLO first
    let interval = loop {
        let message = tokio::time::timeout(Duration::from_secs(30), stream.next())
            .await?
            .ok_or_else(|| anyhow::anyhow!("closed before HELLO"))??;
        match message {
            Message::Text(text) => {
                let payload: Payload = serde_json::from_str(text.as_str())?;
                if payload.op == 10 {
                    break payload.d["heartbeat_interval"]
                        .as_u64()
                        .filter(|interval| *interval > 0)
                        .unwrap_or(41_250);
                }
            }
            Message::Close(frame) => {
                return Ok(closed(frame.map_or(1000, |frame| u16::from(frame.code))));
            }
            _ => {}
        }
    };

    let hello = match (resume.as_ref(), *sequence) {
        (Some(resume), Some(sequence)) => serde_json::json!({
            "op": 6,
            "d": { "token": wanted.token, "session_id": resume.session_id, "seq": sequence },
        }),
        _ => identify(&wanted.token, intents),
    };
    sink.send(Message::Text(hello.to_string().into())).await?;

    // the first beat at a random point of the interval, then every interval
    let jitter = chrono::Utc::now().timestamp_subsec_nanos() as u64 % interval;
    let mut heartbeat = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(jitter),
        Duration::from_millis(interval),
    );
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut acknowledged = true;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if !acknowledged {
                    // a zombie connection: Discord stopped answering
                    let _ = sink.send(Message::Close(Some(CloseFrame {
                        code: CloseCode::from(4000),
                        reason: "heartbeat not acknowledged".into(),
                    }))).await;
                    return Ok(End::Reconnect { resume: true });
                }
                acknowledged = false;
                let beat = serde_json::json!({ "op": 1, "d": *sequence });
                sink.send(Message::Text(beat.to_string().into())).await?;
            }
            message = stream.next() => {
                let Some(message) = message else {
                    return Ok(End::Reconnect { resume: true });
                };
                let text = match message? {
                    Message::Text(text) => text,
                    Message::Close(frame) => {
                        return Ok(closed(frame.map_or(1000, |frame| u16::from(frame.code))));
                    }
                    _ => continue,
                };
                let payload: Payload = match serde_json::from_str(text.as_str()) {
                    Ok(payload) => payload,
                    Err(err) => {
                        tracing::debug!("unreadable gateway payload: {err:#}");
                        continue;
                    }
                };
                if let Some(s) = payload.s {
                    *sequence = Some(s);
                }

                match payload.op {
                    0 => {
                        let name = payload.t.unwrap_or_default();
                        if name == "READY" {
                            let data = &payload.d;
                            let session_id = data["session_id"].as_str().unwrap_or_default();
                            let url = data["resume_gateway_url"].as_str().unwrap_or(GATEWAY);
                            *resume = Some(Resume {
                                session_id: session_id.to_string(),
                                url: url.to_string(),
                            });

                            let username = data["user"]["username"].as_str().map(str::to_string);
                            {
                                let mut shared = shared.lock();
                                shared.phase = Phase::Ready;
                                shared.username = username.clone();
                                shared.since = chrono::Utc::now();
                                if intents & INTENT_GUILD_MEMBERS != 0 {
                                    shared.reason = None;
                                }
                            }
                            tracing::info!(bots = ?wanted.bots, bot = ?username, "Discord gateway ready");

                            let _ = events.send(Event::Ready(Session {
                                rest: rest.clone(),
                                application_id: data["application"]["id"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                user_id: data["user"]["id"].as_str().unwrap_or_default().to_string(),
                                guild_id: wanted.guild_id.clone(),
                                intents,
                            }));
                        } else if name == "RESUMED" {
                            let mut shared = shared.lock();
                            shared.phase = Phase::Ready;
                            shared.since = chrono::Utc::now();
                        }
                        let _ = events.send(Event::Dispatch(name, payload.d));
                    }
                    // Discord asks for a beat now
                    1 => {
                        let beat = serde_json::json!({ "op": 1, "d": *sequence });
                        sink.send(Message::Text(beat.to_string().into())).await?;
                    }
                    7 => return Ok(End::Reconnect { resume: true }),
                    9 => {
                        let resumable = payload.d.as_bool().unwrap_or(false);
                        // Discord asks for a pause of 1 to 5 seconds before identifying again
                        tokio::time::sleep(Duration::from_millis(1000 + jitter % 4000)).await;
                        return Ok(End::Reconnect { resume: resumable });
                    }
                    11 => acknowledged = true,
                    _ => {}
                }
            }
        }
    }
}

/// Runs a session's events through its handlers, in order. Ends with the session.
async fn worker(
    state: crate::State,
    handlers: Vec<Arc<dyn Handler>>,
    rest: rest::Client,
    mut events: tokio::sync::mpsc::UnboundedReceiver<Event>,
) {
    let mut session: Option<Session> = None;

    while let Some(event) = events.recv().await {
        match event {
            Event::Ready(ready) => {
                if !ready.application_id.is_empty() {
                    for handler in &handlers {
                        for command in handler.commands() {
                            if let Err(err) = rest
                                .upsert_command(&ready.application_id, &ready.guild_id, &command)
                                .await
                            {
                                tracing::warn!(
                                    command = ?command.get("name"),
                                    "failed to register a Discord command: {err:#}"
                                );
                            }
                        }
                    }
                }
                session = Some(ready);
            }
            Event::Dispatch(name, data) => {
                let Some(session) = &session else {
                    continue;
                };
                for handler in &handlers {
                    if let Err(err) = handler.event(&state, session, &name, &data).await {
                        tracing::warn!(event = %name, "a Discord bot failed on an event: {err:#}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bots_on_one_token_share_a_session() {
        let settings = AppSettingsDiscord {
            enabled: true,
            guild_id: "123456789012345678".into(),
            bot_token: Some("one".into()),
            rewards_bot_enabled: true,
            ..Default::default()
        };
        let sessions = wanted(&settings);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].bots, vec![Bot::Rewards]);

        // off, or without a server: nothing connects
        let off = AppSettingsDiscord {
            rewards_bot_enabled: false,
            ..settings.clone()
        };
        assert!(wanted(&off).is_empty());
        let no_server = AppSettingsDiscord {
            guild_id: "".into(),
            ..settings.clone()
        };
        assert!(wanted(&no_server).is_empty());

        // a bot without a handler is not connected
        let assistant = AppSettingsDiscord {
            assistant_bot_enabled: true,
            assistant_bot_token: Some("two".into()),
            ..settings
        };
        if !HANDLERS.read().contains_key(&Bot::Assistant) {
            assert_eq!(wanted(&assistant).len(), 1);
        }
    }

    #[test]
    fn close_codes() {
        assert!(matches!(closed(4004), End::Refused(Reason::Token)));
        assert!(matches!(closed(4014), End::Privileged));
        assert!(matches!(closed(4009), End::Reconnect { resume: false }));
        assert!(matches!(closed(4000), End::Reconnect { resume: true }));
    }
}
