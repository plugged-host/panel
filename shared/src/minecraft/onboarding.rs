//! plugged.host: the setup a player's new Minecraft server starts with (docs/features/minecraft.md):
//! software and version or a modpack, Bedrock, who may join, and who runs it, asked in plain words
//! while the server installs, then applied once the install is over, after which the server
//! starts.
//!
//! A `server_onboardings` row is made when a player creates a Minecraft server; servers an admin
//! creates have none and never see the setup. The software (or modpack) is put in place as soon as
//! it is picked ([`choose`], stored on the row's `answers` while it is pending), the rest once all
//! the answers are submitted ([`complete`], `completed`). One worker per server ([`kick`], under
//! the server's software lock) does both after the requests; the page polls its progress.

use super::{Loader, Source, bedrock, files, modpacks, players};
use crate::models::{ByUuid, server::Server};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const RUNNING_TTL: u64 = 3 * 60 * 60;
const DONE_TTL: u64 = 60 * 60;
/// Names per list, as many as a first setup reasonably holds.
const MAX_NAMES: usize = 20;
/// How often a running pick looks whether the player picked something else.
const WATCH: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModpackChoice {
    #[garde(skip)]
    pub source: Source,
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub project: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub version: compact_str::CompactString,
}

/// What the server runs, as the setup picked it: a software choice and Minecraft version, or a
/// modpack (which brings its own). Empty: what the server was created with.
#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SoftwarePick {
    /// The software choice; `null` keeps what the server was created with.
    #[garde(inner(length(chars, min = 1, max = 64)))]
    #[schema(min_length = 1, max_length = 64)]
    #[serde(default)]
    pub variant: Option<compact_str::CompactString>,
    /// The Minecraft version (`latest` or a version); `null` keeps the server's.
    #[garde(inner(length(chars, min = 1, max = 32)))]
    #[schema(min_length = 1, max_length = 32)]
    #[serde(default)]
    pub version: Option<compact_str::CompactString>,
    #[garde(dive)]
    #[serde(default)]
    pub modpack: Option<ModpackChoice>,
}

impl SoftwarePick {
    /// A modpack brings its own software and version, so a pick with one is the pack alone.
    fn normalized(self) -> Self {
        match self.modpack {
            Some(modpack) => Self {
                variant: None,
                version: None,
                modpack: Some(modpack),
            },
            None => self,
        }
    }

    /// The pick stored on a setup row: the pick while it is pending, the answers' once submitted
    /// (both hold these fields).
    fn of(answers: Option<&serde_json::Value>) -> Self {
        answers
            .and_then(|answers| serde_json::from_value::<Self>(answers.clone()).ok())
            .unwrap_or_default()
            .normalized()
    }

    pub fn reinstalls(&self) -> bool {
        self.modpack.is_some() || self.variant.is_some() || self.version.is_some()
    }
}

#[derive(ToSchema, Validate, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Answers {
    /// The software choice; `null` keeps what the server was created with.
    #[garde(inner(length(chars, min = 1, max = 64)))]
    #[schema(min_length = 1, max_length = 64)]
    #[serde(default)]
    pub variant: Option<compact_str::CompactString>,
    /// The Minecraft version (`latest` or a version); `null` keeps the server's.
    #[garde(inner(length(chars, min = 1, max = 32)))]
    #[schema(min_length = 1, max_length = 32)]
    #[serde(default)]
    pub version: Option<compact_str::CompactString>,
    /// A modpack, which brings its own software and version.
    #[garde(dive)]
    #[serde(default)]
    pub modpack: Option<ModpackChoice>,
    #[garde(skip)]
    #[serde(default)]
    pub bedrock: bool,
    /// `online-mode`: players need a bought Minecraft account.
    #[garde(skip)]
    pub online_mode: bool,
    /// Only the players on the whitelist join.
    #[garde(skip)]
    pub private: bool,
    #[garde(length(max = MAX_NAMES), inner(length(chars, min = 1, max = 16)))]
    #[schema(max_items = 20)]
    #[serde(default)]
    pub players: Vec<compact_str::CompactString>,
    #[garde(length(max = MAX_NAMES), inner(length(chars, min = 1, max = 16)))]
    #[schema(max_items = 20)]
    #[serde(default)]
    pub operators: Vec<compact_str::CompactString>,
}

impl Answers {
    /// Whether the answers change what the server runs (software, version or a modpack).
    pub fn reinstalls(&self) -> bool {
        self.modpack.is_some() || self.variant.is_some() || self.version.is_some()
    }

    /// The names to write: valid Minecraft names, each once (case-insensitively).
    fn names(names: &[compact_str::CompactString]) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for name in names.iter().map(|name| name.trim()) {
            if players::valid_name(name)
                && !seen.iter().any(|known| known.eq_ignore_ascii_case(name))
            {
                seen.push(name.to_string());
            }
        }

        seen
    }
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// The server's first install is still running.
    Waiting,
    Software,
    Modpack,
    /// The picked software is in place; the rest waits for the answers.
    Ready,
    /// server.properties, the whitelist and the operators.
    Settings,
    Bedrock,
    Starting,
    Done,
    Failed,
}

#[derive(ToSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Progress {
    pub step: Step,
    /// Why the setup stopped, or what part of it did not work.
    pub error: Option<String>,
    /// Names that could not be added (no such Minecraft account).
    pub skipped: Vec<String>,
    pub started: chrono::DateTime<chrono::Utc>,
    /// The software being put in place, or put in place when `software_done`.
    #[serde(default)]
    pub software: SoftwarePick,
    #[serde(default = "in_place")]
    pub software_done: bool,
    /// What did not work while putting it in place, reported with the rest.
    #[serde(default)]
    pub problems: Vec<String>,
    /// Why the rest cannot follow it (the server's install failed).
    #[serde(default)]
    pub fatal: Option<String>,
}

/// A progress stored before picks were put in place at once had nothing in place but the
/// server's own software.
fn in_place() -> bool {
    true
}

impl Progress {
    fn new() -> Self {
        Self {
            step: Step::Waiting,
            error: None,
            skipped: Vec::new(),
            started: chrono::Utc::now(),
            software: SoftwarePick::default(),
            software_done: true,
            problems: Vec::new(),
            fatal: None,
        }
    }

    fn ended(&self) -> bool {
        matches!(self.step, Step::Done | Step::Failed)
    }
}

#[derive(ToSchema, Serialize, Debug)]
pub struct Onboarding {
    /// The server still waits for its setup.
    pub pending: bool,
    pub completed: Option<chrono::DateTime<chrono::Utc>>,
    pub progress: Option<Progress>,
    /// The software choices Bedrock players can join, by value.
    pub bedrock_variants: Vec<compact_str::CompactString>,
    /// The loaders Bedrock players can join, for a modpack's loader.
    pub bedrock_loaders: Vec<Loader>,
    /// Whether the software the server runs now lets Bedrock players join.
    pub bedrock_now: bool,
    /// The software choices that take modpacks, by value, with the loader their packs are for
    /// (the setup's modpack step; Paper-family software takes plugins instead).
    pub modpack_loaders: std::collections::BTreeMap<compact_str::CompactString, Loader>,
    /// The picked modpack's install, while the setup puts one in place.
    pub modpack: Option<modpacks::Install>,
}

#[derive(Debug)]
pub enum OnboardingError {
    /// The server has no setup, or it was already done.
    NotPending,
    Other(anyhow::Error),
}

impl From<sqlx::Error> for OnboardingError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

impl OnboardingError {
    pub fn response(self) -> crate::response::ApiResponse {
        match self {
            Self::NotPending => crate::response::ApiResponse::error("this server's setup is done")
                .with_status(axum::http::StatusCode::CONFLICT),
            Self::Other(err) => crate::response::ApiResponse::from(err),
        }
    }
}

fn key(server_uuid: uuid::Uuid) -> String {
    format!("minecraft::onboarding::{server_uuid}")
}

async fn store(state: &crate::State, server_uuid: uuid::Uuid, progress: &Progress) {
    let ttl = match progress.step {
        Step::Done | Step::Failed => DONE_TTL,
        _ => RUNNING_TTL,
    };
    state.cache.set(&key(server_uuid), ttl, progress).await.ok();
}

/// Marks a player's new Minecraft server for setup.
pub async fn require(state: &crate::State, server_uuid: uuid::Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO server_onboardings (server_uuid) VALUES ($1) ON CONFLICT (server_uuid) DO NOTHING",
    )
    .bind(server_uuid)
    .execute(state.database.write())
    .await?;

    Ok(())
}

/// The server's setup, `None` when it has none.
pub async fn get(
    state: &crate::State,
    server: &Server,
) -> Result<Option<Onboarding>, anyhow::Error> {
    let Some(row) = Row::load(state, server.uuid).await? else {
        return Ok(None);
    };

    let settings = state.settings.get().await.map_err(anyhow::Error::from)?;
    let game = settings.games.game(server.egg.uuid);
    let bedrock_variants = game
        .map(|game| {
            game.variants
                .iter()
                .filter(|variant| bedrock::platform(Loader::from_value(&variant.value)).is_some())
                .map(|variant| variant.value.clone())
                .collect()
        })
        .unwrap_or_default();
    let modpack_loaders = modpack_loaders(
        game.iter()
            .flat_map(|game| game.variants.iter())
            .map(|variant| variant.value.as_str()),
    );
    drop(settings);

    let progress = load(state, server.uuid).await;
    // a worker that died with its panel instance (a restart) is taken up again by the next look
    if work_left(&row, progress.as_ref()) {
        kick(state, server.uuid);
    }
    let modpack = match &progress {
        Some(progress) if progress.software.modpack.is_some() => {
            modpacks::current(state, server).await
        }
        _ => None,
    };

    Ok(Some(Onboarding {
        pending: row.completed.is_none(),
        completed: row.completed.map(|completed| completed.and_utc()),
        progress,
        bedrock_variants,
        bedrock_loaders: bedrock_loaders(),
        bedrock_now: bedrock::platform(super::software(state, server).await?.loader).is_some(),
        modpack_loaders,
        modpack,
    }))
}

/// Stores the software the player picked for a pending setup and starts putting it in place
/// (after the server's first install, when that still runs). A pick replaces the one before: one
/// that has not changed the server yet is dropped, one that has is finished or stopped between
/// files first, never two reinstalls at once.
pub async fn choose(
    state: &crate::State,
    server: &Server,
    pick: &SoftwarePick,
) -> Result<(), OnboardingError> {
    let updated = sqlx::query(
        "UPDATE server_onboardings SET answers = $2 WHERE server_uuid = $1 AND completed IS NULL",
    )
    .bind(server.uuid)
    .bind(
        serde_json::to_value(pick.clone().normalized())
            .map_err(|err| OnboardingError::Other(err.into()))?,
    )
    .execute(state.database.write())
    .await?;
    if updated.rows_affected() == 0 {
        return Err(OnboardingError::NotPending);
    }

    kick(state, server.uuid);

    Ok(())
}

/// Stores the answers as the server's setup (once); the worker then puts the software in place
/// (when the answers pick other software than what was picked before) and applies the rest.
pub async fn complete(
    state: &crate::State,
    server: &Server,
    answers: &Answers,
) -> Result<(), OnboardingError> {
    let updated = sqlx::query(
        "UPDATE server_onboardings SET completed = NOW(), answers = $2
         WHERE server_uuid = $1 AND completed IS NULL",
    )
    .bind(server.uuid)
    .bind(serde_json::to_value(answers).map_err(|err| OnboardingError::Other(err.into()))?)
    .execute(state.database.write())
    .await?;
    if updated.rows_affected() == 0 {
        return Err(OnboardingError::NotPending);
    }

    // the page shows the setup running from the first look on
    if load(state, server.uuid).await.is_none() {
        store(state, server.uuid, &Progress::new()).await;
    }

    Ok(())
}

/// Puts the setup in place after the request: starts its worker, unless one runs already (it
/// sees the new pick or answers itself).
pub fn kick(state: &crate::State, server_uuid: uuid::Uuid) {
    let state = state.clone();
    tokio::spawn(async move { drive(&state, server_uuid).await });
}

/// [`kick`], waiting until the setup has ended (its answers applied, or it failed): `None` when it
/// did not within `timeout` or there is no setup.
pub async fn run_until_ended(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    timeout: std::time::Duration,
) -> Option<Progress> {
    drive(state, server_uuid).await;

    // another instance's worker may be the one applying it
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if let Some(progress) = load(state, server_uuid).await
            && progress.ended()
        {
            return Some(progress);
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    None
}

/// The setup row: what was picked (or answered) and when it was submitted.
struct Row {
    answers: Option<serde_json::Value>,
    completed: Option<chrono::NaiveDateTime>,
}

impl Row {
    async fn load(
        state: &crate::State,
        server_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let row: Option<(Option<serde_json::Value>, Option<chrono::NaiveDateTime>)> =
            sqlx::query_as(
                "SELECT answers, completed FROM server_onboardings WHERE server_uuid = $1",
            )
            .bind(server_uuid)
            .fetch_optional(state.database.write())
            .await?;

        Ok(row.map(|(answers, completed)| Self { answers, completed }))
    }

    fn pick(&self) -> SoftwarePick {
        SoftwarePick::of(self.answers.as_ref())
    }
}

async fn load(state: &crate::State, server_uuid: uuid::Uuid) -> Option<Progress> {
    state
        .cache
        .get::<Progress>(&key(server_uuid))
        .await
        .ok()
        .flatten()
}

/// What the worker still has to do: a pick that is not in place, or answers not applied. A setup
/// that ended, or was submitted so long ago that its progress is gone (a submit stores one first),
/// has none: it is never applied twice.
fn work_left(row: &Row, progress: Option<&Progress>) -> bool {
    match progress {
        None => row.completed.is_none() && row.pick().reinstalls(),
        Some(progress) if progress.ended() => false,
        Some(progress) => {
            !(progress.software_done && progress.software == row.pick()) || row.completed.is_some()
        }
    }
}

/// The worker: holds the server's software lock while there is work, and looks again once it
/// let it go (a pick or the answers may have come in just then; whoever holds the lock next
/// sees them).
async fn drive(state: &crate::State, server_uuid: uuid::Uuid) {
    let mut wait = std::time::Duration::ZERO;
    // a round that cannot get anywhere (the database gone) must not spin
    for _ in 0..20 {
        let Some(lock) = crate::server_software::lock(state, server_uuid, wait).await else {
            return;
        };
        if let Err(err) = work(state, server_uuid).await {
            tracing::warn!(server = %server_uuid, "minecraft setup: {err:#}");
        }
        drop(lock);

        let left = match Row::load(state, server_uuid).await {
            Ok(Some(row)) => work_left(&row, load(state, server_uuid).await.as_ref()),
            _ => false,
        };
        if !left {
            return;
        }
        // its own lock is let go in the background
        wait = std::time::Duration::from_secs(5);
    }
}

async fn work(state: &crate::State, server_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let mut progress = load(state, server_uuid).await.unwrap_or_else(Progress::new);
    if progress.ended() {
        return Ok(());
    }

    loop {
        let Some(row) = Row::load(state, server_uuid).await? else {
            return Ok(());
        };
        let pick = row.pick();

        if !(progress.software_done && progress.software == pick) {
            put_in_place(state, server_uuid, pick, &mut progress).await?;
            continue;
        }

        if row.completed.is_some() {
            finish(state, server_uuid, &row, &mut progress).await;
            return Ok(());
        }

        // the software is in place, the rest of the answers are still being given
        if pick.reinstalls() && progress.step != Step::Ready {
            progress.step = Step::Ready;
            store(state, server_uuid, &progress).await;
        }
        return Ok(());
    }
}

/// Watches the row and cancels once the pick differs from `pick`; never ends, the work it
/// runs beside decides when to stop.
async fn watch(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    pick: &SoftwarePick,
    cancel: &tokio_util::sync::CancellationToken,
) {
    loop {
        tokio::time::sleep(WATCH).await;
        if let Ok(Some(row)) = Row::load(state, server_uuid).await
            && row.pick() != *pick
        {
            cancel.cancel();
            std::future::pending::<()>().await;
        }
    }
}

/// Puts `pick` in place: switches the software and version or installs the modpack, once the
/// server's first install is over (a pack is downloaded and read meanwhile). Stopped when the
/// player picks something else (`software_done` stays false), except during a reinstall, which
/// is waited for.
async fn put_in_place(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    pick: SoftwarePick,
    progress: &mut Progress,
) -> Result<(), anyhow::Error> {
    progress.software = pick.clone();
    progress.software_done = false;
    progress.problems.clear();
    progress.fatal = None;
    progress.step = if pick.modpack.is_some() {
        Step::Modpack
    } else {
        Step::Waiting
    };
    store(state, server_uuid, progress).await;

    let server = Server::by_uuid(&state.database, server_uuid).await?;
    remove_pack_files(state, &server).await;

    let cancel = tokio_util::sync::CancellationToken::new();
    let first_install_failed = std::sync::atomic::AtomicBool::new(false);
    let ready = async {
        match crate::server_software::wait_for_install(
            state,
            server_uuid,
            crate::server_software::INSTALL_WAIT,
        )
        .await
        {
            Ok(true) => Ok(()),
            Ok(false) => {
                first_install_failed.store(true, std::sync::atomic::Ordering::Relaxed);
                Err(modpacks::ModpackError::InstallFailed)
            }
            Err(err) => Err(modpacks::ModpackError::from(err)),
        }
    };

    // the watch only cancels: what runs stops where it safely can (never during a reinstall)
    let result: Result<Option<String>, PutError> = tokio::select! {
        result = async {
            match &pick.modpack {
                Some(modpack) => put_modpack(state, server, modpack, ready, &cancel).await,
                None => put_software(state, server, &pick, ready, &cancel, progress).await,
            }
        } => result,
        _ = watch(state, server_uuid, &pick, &cancel) => unreachable!("the watch never ends"),
    };

    match result {
        Ok(problem) => {
            progress.problems.extend(problem);
            progress.software_done = true;
        }
        Err(PutError::Cancelled) => return Ok(()),
        Err(PutError::Fatal(message)) => {
            progress.fatal = Some(message);
            progress.software_done = true;
        }
    }
    if first_install_failed.load(std::sync::atomic::Ordering::Relaxed) {
        progress.fatal =
            Some("the server's install did not finish; reinstall it from Settings".into());
    }
    store(state, server_uuid, progress).await;

    Ok(())
}

enum PutError {
    /// The player picked something else meanwhile.
    Cancelled,
    /// The rest of the setup cannot follow.
    Fatal(String),
}

async fn put_modpack(
    state: &crate::State,
    server: Server,
    modpack: &ModpackChoice,
    ready: impl Future<Output = Result<(), modpacks::ModpackError>>,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<Option<String>, PutError> {
    let start = match modpacks::start(
        state,
        &server,
        modpack.source,
        &modpack.project,
        &modpack.version,
        false,
    )
    .await
    {
        Ok(start) => start,
        Err(err) => {
            return Ok(Some(format!(
                "the modpack did not install: {}",
                err.message()
            )));
        }
    };

    let server_uuid = server.uuid;
    let outcome = modpacks::install(state, server, start, ready, cancel).await;
    if !outcome.written.is_empty() {
        let mut written = pack_files(state, server_uuid).await;
        written.extend(outcome.written);
        state
            .cache
            .set(&pack_files_key(server_uuid), RUNNING_TTL, &written)
            .await
            .ok();
    }
    if cancel.is_cancelled() {
        return Err(PutError::Cancelled);
    }

    Ok(outcome
        .install
        .error
        .map(|error| format!("the modpack did not install: {error}")))
}

async fn put_software(
    state: &crate::State,
    server: Server,
    pick: &SoftwarePick,
    ready: impl Future<Output = Result<(), modpacks::ModpackError>>,
    cancel: &tokio_util::sync::CancellationToken,
    progress: &mut Progress,
) -> Result<Option<String>, PutError> {
    let failed = |err: &dyn std::fmt::Display| {
        tracing::warn!(server = %server.uuid, "minecraft setup: {err}");
        PutError::Fatal("the setup could not be applied, change it on the server's pages".into())
    };

    let ready = tokio::select! {
        ready = ready => ready,
        _ = cancel.cancelled() => return Err(PutError::Cancelled),
    };
    if ready.is_err() {
        return Err(PutError::Fatal(
            "the server's install did not finish; reinstall it from Settings".into(),
        ));
    }
    let mut server = Server::by_uuid(&state.database, server.uuid)
        .await
        .map_err(|err| failed(&err))?;

    let software = super::software(state, &server)
        .await
        .map_err(|err| failed(&err))?;
    let variant = pick
        .variant
        .as_deref()
        .filter(|variant| software.value.as_deref() != Some(*variant));
    let version = pick
        .version
        .as_deref()
        .filter(|version| !software.version.eq_ignore_ascii_case(version));
    if variant.is_none() && version.is_none() {
        return Ok(None);
    }
    if cancel.is_cancelled() {
        return Err(PutError::Cancelled);
    }

    progress.step = Step::Software;
    store(state, server.uuid, progress).await;
    if let Err(err) = crate::server_software::switch(state, &mut server, variant, version).await {
        return Ok(Some(format!(
            "the software was not changed: {}",
            err.message()
        )));
    }

    // a reinstall that began is waited for, whatever is picked meanwhile
    match crate::server_software::wait_for_install(
        state,
        server.uuid,
        crate::server_software::INSTALL_WAIT,
    )
    .await
    {
        Ok(true) => Ok(None),
        Ok(false) => Err(PutError::Fatal(
            "reinstalling with the chosen software failed; reinstall it from Settings".into(),
        )),
        Err(err) => Err(failed(&err)),
    }
}

fn pack_files_key(server_uuid: uuid::Uuid) -> String {
    format!("minecraft::onboarding::files::{server_uuid}")
}

/// The files the setup's modpacks downloaded so far (`directory/name`), kept beside the progress
/// for whichever worker takes the next pick.
async fn pack_files(state: &crate::State, server_uuid: uuid::Uuid) -> Vec<String> {
    state
        .cache
        .get::<Vec<String>>(&pack_files_key(server_uuid))
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Removes the files an earlier pick's modpack downloaded (the setup runs on a new server, so they
/// are the pack's own), before what was picked instead goes in.
async fn remove_pack_files(state: &crate::State, server: &Server) {
    let written = pack_files(state, server.uuid).await;
    if written.is_empty() {
        return;
    }
    let Ok(client) = files::client(state, server).await else {
        return;
    };
    state
        .cache
        .invalidate(&pack_files_key(server.uuid))
        .await
        .ok();

    let mut by_directory: std::collections::BTreeMap<String, Vec<compact_str::CompactString>> =
        std::collections::BTreeMap::new();
    for path in written {
        let (directory, name) = path.rsplit_once('/').unwrap_or(("", path.as_str()));
        by_directory
            .entry(directory.to_string())
            .or_default()
            .push(name.into());
    }
    for (directory, names) in by_directory {
        if let Err(err) = client
            .post_servers_server_files_delete(
                server.uuid,
                &wings_api::servers_server_files_delete::post::RequestBody {
                    root: format!("/{directory}").into(),
                    files: names,
                },
            )
            .await
        {
            tracing::warn!(server = %server.uuid, "minecraft setup: removing a replaced pick's files: {err:?}");
        }
    }
}

/// Applies the rest of the answers once the software is in place, then starts the server. A part
/// that fails is reported and the rest still applied: a pack that would not install should not
/// also cost the player their whitelist.
async fn finish(state: &crate::State, server_uuid: uuid::Uuid, row: &Row, progress: &mut Progress) {
    let answers = row
        .answers
        .clone()
        .and_then(|answers| serde_json::from_value::<Answers>(answers).ok());

    let result = match (&progress.fatal, answers) {
        (Some(fatal), _) => Err(fatal.clone()),
        (None, None) => {
            tracing::warn!(server = %server_uuid, "minecraft setup: the stored answers do not read");
            Err("the setup could not be applied, change it on the server's pages".into())
        }
        (None, Some(answers)) => apply_rest(state, server_uuid, &answers, progress).await,
    };

    match result {
        Ok(()) => progress.step = Step::Done,
        Err(message) => {
            progress.step = Step::Failed;
            progress.error = Some(message);
        }
    }
    store(state, server_uuid, progress).await;
}

async fn apply_rest(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    answers: &Answers,
    progress: &mut Progress,
) -> Result<(), String> {
    let failed = |err: &dyn std::fmt::Display| {
        tracing::warn!(server = %server_uuid, "minecraft setup: {err}");
        "the setup could not be applied, change it on the server's pages".to_string()
    };

    // nothing was picked: the first install may still run
    if !crate::server_software::wait_for_install(
        state,
        server_uuid,
        crate::server_software::INSTALL_WAIT,
    )
    .await
    .map_err(|err| failed(&err))?
    {
        return Err("the server's install did not finish; reinstall it from Settings".into());
    }
    let server = Server::by_uuid(&state.database, server_uuid)
        .await
        .map_err(|err| failed(&err))?;

    let mut problems = progress.problems.clone();

    progress.step = Step::Settings;
    store(state, server.uuid, progress).await;
    write_settings(state, &server, answers, progress)
        .await
        .map_err(|err| failed(&err))?;

    if answers.bedrock {
        progress.step = Step::Bedrock;
        store(state, server.uuid, progress).await;

        let software = super::software(state, &server)
            .await
            .map_err(|err| failed(&err))?;
        let result = match bedrock::platform(software.loader) {
            Some(_) => bedrock::install(state, &server).await.map(|_| ()),
            None => Err(bedrock::BedrockError::Unsupported),
        };
        if let Err(err) = result {
            problems.push(format!(
                "bedrock support was not set up: {}",
                bedrock_message(err)
            ));
        }
    }

    progress.step = Step::Starting;
    store(state, server.uuid, progress).await;
    let client = files::client(state, &server)
        .await
        .map_err(|err| failed(&err))?;
    if matches!(
        files::power_state(&client, &server).await,
        Some(wings_api::ServerState::Offline)
    ) {
        client
            .post_servers_server_power(
                server.uuid,
                &wings_api::servers_server_power::post::RequestBody {
                    action: wings_api::ServerPowerAction::Start,
                    wait_seconds: None,
                },
            )
            .await
            .map_err(|err| failed(&format!("{err:?}")))?;
    }

    if !problems.is_empty() {
        progress.error = Some(problems.join("; "));
    }

    Ok(())
}

fn bedrock_message(err: bedrock::BedrockError) -> String {
    match err {
        bedrock::BedrockError::Unsupported => {
            "geyser runs on paper and its forks, fabric and neoforge".into()
        }
        bedrock::BedrockError::NoPort => "the node has no free port left".into(),
        bedrock::BedrockError::Content(err) => err.message(),
        bedrock::BedrockError::Wings(err) | bedrock::BedrockError::Other(err) => {
            tracing::warn!("minecraft setup bedrock: {err:#}");
            "try it again from Plugins & Mods".into()
        }
    }
}

/// `online-mode` first (it decides which UUID a name stands for), then the whitelist, then the
/// operators: a player has to get in before being an operator is of any use.
async fn write_settings(
    state: &crate::State,
    server: &Server,
    answers: &Answers,
    progress: &mut Progress,
) -> Result<(), anyhow::Error> {
    let client = files::client(state, server).await?;
    let raw = files::read_text(&client, server, "server.properties", 1024 * 1024)
        .await?
        .unwrap_or_default();
    files::write_text(
        &client,
        server,
        "server.properties",
        super::config::set_properties(&raw, &[("online-mode", answers.online_mode.to_string())]),
    )
    .await?;

    players::enforce(state, server, answers.private, false)
        .await
        .map_err(|err| anyhow::anyhow!("{err:?}"))?;

    let whitelist = if answers.private {
        Answers::names(&answers.players)
    } else {
        Vec::new()
    };
    let changes = whitelist
        .into_iter()
        .map(|name| (name, players::Change::Whitelist(true)))
        .chain(
            Answers::names(&answers.operators)
                .into_iter()
                .map(|name| (name, players::Change::Operator(true))),
        );
    for (name, change) in changes {
        if let Err(err) = players::change(state, server, &name, change, false).await {
            tracing::debug!(server = %server.uuid, "minecraft setup {name}: {err:?}");
            if !progress.skipped.contains(&name) {
                progress.skipped.push(name);
            }
        }
    }

    Ok(())
}

/// The loaders Bedrock support exists for, for the setup page.
pub fn bedrock_loaders() -> Vec<Loader> {
    [
        Loader::Paper,
        Loader::Fabric,
        Loader::Quilt,
        Loader::Forge,
        Loader::NeoForge,
    ]
    .into_iter()
    .filter(|loader| bedrock::platform(Some(*loader)).is_some())
    .collect()
}

/// The software choices (by value) whose loader takes modpacks, with that loader: every mod
/// loader, not the Paper family (the same rule as [`modpacks::loaders`]).
fn modpack_loaders<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> std::collections::BTreeMap<compact_str::CompactString, Loader> {
    values
        .into_iter()
        .filter_map(|value| {
            Loader::from_value(value)
                .filter(|loader| *loader != Loader::Paper)
                .map(|loader| (value.into(), loader))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modpacks_are_for_mod_loaders_only() {
        let loaders =
            modpack_loaders(["purpur", "fabric", "NeoForge", "forge", "vanilla", "paper"]);

        assert_eq!(
            loaders.into_iter().collect::<Vec<_>>(),
            vec![
                ("NeoForge".into(), Loader::NeoForge),
                ("fabric".into(), Loader::Fabric),
                ("forge".into(), Loader::Forge),
            ]
        );
    }

    fn answers() -> Answers {
        Answers {
            variant: None,
            version: None,
            modpack: None,
            bedrock: false,
            online_mode: true,
            private: true,
            players: Vec::new(),
            operators: Vec::new(),
        }
    }

    #[test]
    fn names_are_valid_and_once() {
        assert_eq!(
            Answers::names(&[
                "Notch".into(),
                "notch".into(),
                " jeb_ ".into(),
                "no spaces".into()
            ]),
            vec!["Notch".to_string(), "jeb_".to_string()]
        );
    }

    fn pick(variant: Option<&str>, version: Option<&str>, modpack: bool) -> SoftwarePick {
        SoftwarePick {
            variant: variant.map(Into::into),
            version: version.map(Into::into),
            modpack: modpack.then(|| ModpackChoice {
                source: Source::Modrinth,
                project: "AANobbMI".into(),
                version: "abc".into(),
            }),
        }
    }

    #[test]
    fn the_pick_reads_from_a_pick_or_the_answers() {
        // a pending row holds the pick alone, a submitted one the answers around it
        assert_eq!(
            SoftwarePick::of(Some(
                &serde_json::json!({ "variant": "fabric", "version": "1.21.1" })
            )),
            pick(Some("fabric"), Some("1.21.1"), false)
        );
        let answers = serde_json::to_value(Answers {
            variant: Some("forge".into()),
            ..answers()
        })
        .unwrap();
        assert_eq!(
            SoftwarePick::of(Some(&answers)),
            pick(Some("forge"), None, false)
        );
        // a pack brings its own software
        assert_eq!(
            SoftwarePick::of(Some(
                &serde_json::to_value(pick(Some("fabric"), Some("latest"), true)).unwrap()
            )),
            pick(None, None, true)
        );
        assert_eq!(SoftwarePick::of(None), SoftwarePick::default());
        assert_eq!(
            SoftwarePick::of(Some(&serde_json::json!("not a pick"))),
            SoftwarePick::default()
        );
    }

    fn row(answers: Option<serde_json::Value>, completed: bool) -> Row {
        Row {
            answers,
            completed: completed.then(|| chrono::Utc::now().naive_utc()),
        }
    }

    #[test]
    fn work_is_left_until_the_pick_is_in_place_and_the_answers_applied() {
        let fabric = serde_json::to_value(pick(Some("fabric"), None, false)).unwrap();

        // nothing picked, nothing submitted: no worker
        assert!(!work_left(&row(None, false), None));
        // a pick no worker has taken up yet
        assert!(work_left(&row(Some(fabric.clone()), false), None));
        // submitted so long ago that its progress is gone: never applied again
        assert!(!work_left(&row(Some(fabric.clone()), true), None));

        let mut progress = Progress::new();
        progress.software = pick(Some("fabric"), None, false);
        progress.software_done = false;
        // being put in place
        assert!(work_left(
            &row(Some(fabric.clone()), false),
            Some(&progress)
        ));

        progress.software_done = true;
        progress.step = Step::Ready;
        // in place, the rest not answered yet
        assert!(!work_left(
            &row(Some(fabric.clone()), false),
            Some(&progress)
        ));
        // picked again meanwhile
        let forge = serde_json::to_value(pick(Some("forge"), None, false)).unwrap();
        assert!(work_left(&row(Some(forge), false), Some(&progress)));

        // submitted: the rest is left until it ended
        assert!(work_left(&row(Some(fabric.clone()), true), Some(&progress)));
        progress.step = Step::Done;
        assert!(!work_left(
            &row(Some(fabric.clone()), true),
            Some(&progress)
        ));
        progress.software_done = false;
        assert!(!work_left(&row(Some(fabric), true), Some(&progress)));
    }

    #[test]
    fn progress_stored_before_picks_still_reads() {
        // the earlier shape: step, error, skipped, started
        let old = rmp_serde::to_vec(&(
            Step::Settings,
            Option::<String>::None,
            Vec::<String>::new(),
            chrono::Utc::now(),
        ))
        .unwrap();
        let progress: Progress = rmp_serde::from_slice(&old).unwrap();
        assert_eq!(progress.step, Step::Settings);
        assert!(progress.software_done);
        assert_eq!(progress.software, SoftwarePick::default());
    }

    #[test]
    fn what_reinstalls() {
        assert!(!answers().reinstalls());
        assert!(
            Answers {
                version: Some("1.21.4".into()),
                ..answers()
            }
            .reinstalls()
        );
        assert_eq!(
            bedrock_loaders(),
            vec![Loader::Paper, Loader::Fabric, Loader::NeoForge]
        );
    }
}
