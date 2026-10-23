//! plugged.host: noprofit.space accounts.
//!
//! Optional, like the noprofit plugin on plugged.host's Pelican panel. While Admin › Settings ›
//! noprofit.space leaves it off (or misses the accounts URL, the client or a sign-in secret of at
//! least 32 characters), the panel signs people in on its own (Calagopus's login and
//! registration). Connected, noprofit.space owns sign-in:
//!
//! - everyone signs in at noprofit.space (see [`Config::decode_token`]) and an account's first
//!   sign-in creates its panel user,
//! - registration is off and password sign-in is left to admins without a noprofit.space account
//!   (the emergency admin made with `panel-rs users create`); the panel's own OAuth providers and
//!   security keys are off,
//! - with the API secret the panel keeps its users in step by pulling their snapshots (profile,
//!   admin flag, plan, credits, Discord; [`Config::pull`], every minute), suspends the users whose
//!   account noprofit.space no longer has, and lets noprofit.space check the SFTP passwords of its
//!   users, who have none on the panel ([`Config::verify_password`]),
//! - with "admins only" only noprofit.space admins may sign in (while the panel is not open to
//!   everyone yet).
//!
//! The wire formats are noprofit.space's (accounts `App\Services\SignedToken` and
//! `App\Services\Neo\RequestSignature`): a token is `base64url(json) "." hex(hmac_sha256(secret,
//! base64url(json)))`, and a server-to-server request carries `X-NP-Timestamp`, `X-NP-Nonce` and
//! `X-NP-Signature = hex(hmac_sha256(secret, METHOD\npath\ntimestamp\nnonce\nhex(sha256(body))))`.

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Shorter secrets reject everything, as on noprofit.space.
const MIN_SECRET_LENGTH: usize = 32;
/// How far a token's `iat` may lie in the future (clock skew).
const CLOCK_SKEW: i64 = 60;
/// Snapshots asked for per request (noprofit.space accepts at most 500).
pub const PULL_BATCH: usize = 500;

pub struct Config {
    /// `https://accounts.noprofit.space`, without a trailing slash.
    pub accounts_url: String,
    /// The sign-in client noprofit.space knows this panel as.
    pub client: String,
    /// The tokens' audience, `{client}:sso` unless the settings say otherwise.
    pub audience: String,
    /// The callback noprofit.space sends tokens to, when it is not `{app url}/api/auth/noprofit/callback`.
    pub callback_url: Option<String>,
    /// Only noprofit.space admins may sign in.
    pub admins_only: bool,
    /// A first sign-in takes over the panel user with the same email (unless that one is linked to
    /// another account already).
    pub link_by_email: bool,
    sso_secret: String,
    api_secret: Option<String>,
}

impl Config {
    /// The connection the noprofit.space settings describe, or `None` when they leave sign-in to
    /// the panel (Admin › Settings › noprofit.space, [`crate::settings::noprofit`]).
    pub fn from_settings(
        settings: &crate::settings::noprofit::AppSettingsNoprofit,
    ) -> Option<Self> {
        if !settings.enabled {
            return None;
        }

        let accounts_url = settings.accounts_url.trim().trim_end_matches('/');
        let client = settings.sso_client.trim();
        let sso_secret = settings.sso_secret.as_deref().map(str::trim)?;

        if accounts_url.is_empty() || client.is_empty() || sso_secret.len() < MIN_SECRET_LENGTH {
            return None;
        }

        Some(Self {
            accounts_url: accounts_url.to_string(),
            client: client.to_string(),
            audience: settings
                .sso_audience
                .as_deref()
                .map(str::trim)
                .filter(|audience| !audience.is_empty())
                .map_or_else(|| format!("{client}:sso"), str::to_string),
            callback_url: settings
                .sso_callback_url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string),
            admins_only: settings.admins_only,
            link_by_email: settings.link_by_email,
            sso_secret: sso_secret.to_string(),
            api_secret: settings
                .api_secret
                .as_deref()
                .map(str::trim)
                .filter(|secret| secret.len() >= MIN_SECRET_LENGTH)
                .map(str::to_string),
        })
    }

    /// Whether the panel can pull account snapshots (the API secret is set).
    #[inline]
    pub fn sync_enabled(&self) -> bool {
        self.api_secret.is_some()
    }

    /// Where the browser goes to sign in: noprofit.space's `/sso` for this client.
    pub fn sso_url(&self, return_to: &str, state: &str) -> String {
        format!(
            "{}/sso?client={}&return_to={}&state={}",
            self.accounts_url,
            urlencoding::encode(&self.client),
            urlencoding::encode(return_to),
            urlencoding::encode(state),
        )
    }

    /// Checks a token noprofit.space sent to the callback: its signature, audience and lifetime.
    /// The caller still has to compare its state with the one the browser started with.
    pub fn decode_token(&self, token: &str) -> Result<SsoClaims, TokenError> {
        let (payload, signature) = token.split_once('.').ok_or(TokenError::Malformed)?;

        if payload.is_empty() || signature.contains('.') {
            return Err(TokenError::Malformed);
        }

        let signature = hex::decode(signature).map_err(|_| TokenError::BadSignature)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(self.sso_secret.as_bytes())
            .expect("hmac accepts keys of any length");
        mac.update(payload.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| TokenError::BadSignature)?;

        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload.trim_end_matches('='))
            .map_err(|_| TokenError::Malformed)?;
        let mut claims: RawClaims =
            serde_json::from_slice(&json).map_err(|_| TokenError::Malformed)?;

        if claims.snapshot.account_id == 0 {
            claims.snapshot.account_id = claims.account_user_id;
        }

        let now = chrono::Utc::now().timestamp();

        if claims.aud != self.audience {
            return Err(TokenError::WrongAudience);
        }
        if claims.exp < now || claims.iat > now + CLOCK_SKEW {
            return Err(TokenError::Expired);
        }
        if claims.nonce.is_empty() || claims.state.is_empty() {
            return Err(TokenError::Malformed);
        }
        if claims.snapshot.account_id <= 0 || claims.snapshot.email.is_empty() {
            return Err(TokenError::Malformed);
        }

        Ok(SsoClaims {
            state: claims.state,
            nonce: claims.nonce,
            snapshot: claims.snapshot,
        })
    }

    /// The `X-NP-*` headers for a request to noprofit.space, or `None` without an API secret.
    pub fn sign_request(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Option<[(&'static str, String); 3]> {
        let secret = self.api_secret.as_ref()?;
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let nonce = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 32);

        Some([
            ("X-NP-Timestamp", timestamp.clone()),
            ("X-NP-Nonce", nonce.clone()),
            (
                "X-NP-Signature",
                request_signature(secret, method, path, &timestamp, &nonce, body),
            ),
        ])
    }

    /// A signed POST to noprofit.space's internal API.
    async fn post(
        &self,
        client: &reqwest::Client,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, anyhow::Error> {
        let body = serde_json::to_vec(body)?;
        let headers = self
            .sign_request("POST", path, &body)
            .ok_or_else(|| anyhow::anyhow!("the noprofit.space API secret is not set"))?;

        let mut request = client
            .post(format!("{}{path}", self.accounts_url))
            .timeout(std::time::Duration::from_secs(8))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body);

        for (name, value) in headers {
            request = request.header(name, value);
        }

        Ok(request.send().await?)
    }

    /// The current snapshots of these accounts. An account noprofit.space no longer has is left
    /// out of the answer; that is how a deletion shows.
    pub async fn pull(
        &self,
        client: &reqwest::Client,
        account_ids: &[i64],
    ) -> Result<Vec<Snapshot>, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/accounts";

        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .post(
                client,
                PATH,
                &serde_json::json!({ "account_ids": account_ids }),
            )
            .await?;
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("noprofit.space answered {status} to the account sync");
        }

        #[derive(Deserialize)]
        struct PullResponse {
            accounts: Vec<Snapshot>,
        }

        Ok(response.json::<PullResponse>().await?.accounts)
    }

    /// Whether `password` is this account's noprofit.space password, for SFTP password logins:
    /// the account's panel user has no password of its own. `None` when noprofit.space could not
    /// say (unreachable, throttled, no API secret), which is no verdict on the password.
    /// noprofit.space allows 10 wrong guesses per account and minute, across all panels.
    pub async fn verify_password(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        password: &str,
    ) -> Option<bool> {
        const PATH: &str = "/api/v1/internal/calagopus/verify-password";

        if password.is_empty() || password.len() > 1024 {
            return Some(false);
        }

        let response = match self
            .post(
                client,
                PATH,
                &serde_json::json!({ "account_id": account_id, "password": password }),
            )
            .await
        {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!("noprofit.space could not check an SFTP password: {err:#}");
                return None;
            }
        };

        match response.status() {
            status if status.is_success() => {}
            reqwest::StatusCode::UNPROCESSABLE_ENTITY => return Some(false),
            reqwest::StatusCode::TOO_MANY_REQUESTS => return None,
            status => {
                tracing::warn!("noprofit.space answered {status} to an SFTP password check");
                return None;
            }
        }

        #[derive(Deserialize)]
        struct VerifyResponse {
            #[serde(default)]
            valid: bool,
        }

        match response.json::<VerifyResponse>().await {
            Ok(answer) => Some(answer.valid),
            Err(err) => {
                tracing::warn!(
                    "noprofit.space answered an SFTP password check with something else: {err:#}"
                );
                None
            }
        }
    }
}

/// `hex(hmac_sha256(secret, METHOD\npath\ntimestamp\nnonce\nhex(sha256(body))))`.
pub fn request_signature(
    secret: &str,
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    let canonical = format!(
        "{}\n/{}\n{timestamp}\n{nonce}\n{}",
        method.to_ascii_uppercase(),
        path.trim_start_matches('/'),
        hex::encode(Sha256::digest(body)),
    );

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts keys of any length");
    mac.update(canonical.as_bytes());

    hex::encode(mac.finalize().into_bytes())
}

/// A new state for a sign-in: 40 letters and digits (noprofit.space wants 32 to 128).
pub fn new_state() -> String {
    rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 40)
}

#[derive(Debug)]
pub enum TokenError {
    Malformed,
    BadSignature,
    WrongAudience,
    Expired,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Malformed => "malformed noprofit.space token",
            Self::BadSignature => "noprofit.space token signature mismatch",
            Self::WrongAudience => "noprofit.space token for another client",
            Self::Expired => "expired noprofit.space token",
        })
    }
}

impl std::error::Error for TokenError {}

#[derive(Deserialize)]
struct RawClaims {
    /// Every noprofit.space token names the account here; ours repeat it as `account_id`.
    #[serde(default)]
    account_user_id: i64,
    aud: String,
    iat: i64,
    exp: i64,
    nonce: String,
    #[serde(default)]
    state: String,
    #[serde(flatten)]
    snapshot: Snapshot,
}

pub struct SsoClaims {
    pub state: String,
    pub nonce: String,
    pub snapshot: Snapshot,
}

/// What the panel keeps of a noprofit.space account (accounts `PluggedHostCalagopus::snapshot`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub account_id: i64,
    pub email: String,
    /// The account's display name: not unique, may contain spaces.
    pub username: String,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub credits: i64,
    /// `{cpu, memory, disk, base_*, max_*}`: CPU in %, memory and disk in MiB.
    #[serde(default)]
    pub plan: Option<serde_json::Value>,
    /// `{id, username, avatar}` when a Discord account is linked.
    #[serde(default)]
    pub discord: Option<serde_json::Value>,
}

/// A panel user's noprofit.space account (table `user_noprofit_accounts`).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Link {
    pub user_uuid: uuid::Uuid,
    pub account_id: i64,
    pub email: String,
    pub username: String,
    pub is_admin: bool,
    pub avatar: Option<String>,
    pub credits: i64,
    pub plan: Option<serde_json::Value>,
    pub discord: Option<serde_json::Value>,
    pub deleted: Option<chrono::NaiveDateTime>,
    pub synced: chrono::NaiveDateTime,
    pub created: chrono::NaiveDateTime,
}

const LINK_COLUMNS: &str = "user_uuid, account_id, email, username, is_admin, avatar, credits, plan, discord, deleted, synced, created";

impl Link {
    pub async fn by_user_uuid(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {LINK_COLUMNS} FROM user_noprofit_accounts WHERE user_uuid = $1"
        )))
        .bind(user_uuid)
        .fetch_optional(database.read())
        .await
    }

    pub async fn by_account_id(
        database: &crate::database::Database,
        account_id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {LINK_COLUMNS} FROM user_noprofit_accounts WHERE account_id = $1"
        )))
        .bind(account_id)
        .fetch_optional(database.write())
        .await
    }

    /// Every account the panel still knows on noprofit.space, oldest sync first.
    pub async fn live_account_ids(
        database: &crate::database::Database,
    ) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT account_id FROM user_noprofit_accounts WHERE deleted IS NULL ORDER BY synced ASC",
        )
        .fetch_all(database.read())
        .await
    }

    /// Links a user to an account, or refreshes the link, with the account's snapshot.
    pub async fn store(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        snapshot: &Snapshot,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_noprofit_accounts
                (user_uuid, account_id, email, username, is_admin, avatar, credits, plan, discord, deleted, synced)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NOW())
            ON CONFLICT (user_uuid) DO UPDATE SET
                account_id = EXCLUDED.account_id,
                email = EXCLUDED.email,
                username = EXCLUDED.username,
                is_admin = EXCLUDED.is_admin,
                avatar = EXCLUDED.avatar,
                credits = EXCLUDED.credits,
                plan = EXCLUDED.plan,
                discord = EXCLUDED.discord,
                deleted = NULL,
                synced = NOW()",
        )
        .bind(user_uuid)
        .bind(snapshot.account_id)
        .bind(&snapshot.email)
        .bind(&snapshot.username)
        .bind(snapshot.is_admin)
        .bind(&snapshot.avatar)
        .bind(snapshot.credits)
        .bind(&snapshot.plan)
        .bind(&snapshot.discord)
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// Marks accounts noprofit.space no longer has; returns their users.
    pub async fn mark_deleted(
        database: &crate::database::Database,
        account_ids: &[i64],
    ) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        sqlx::query_scalar::<_, uuid::Uuid>(
            "UPDATE user_noprofit_accounts SET deleted = NOW(), synced = NOW()
            WHERE account_id = ANY($1) AND deleted IS NULL
            RETURNING user_uuid",
        )
        .bind(account_ids)
        .fetch_all(database.write())
        .await
    }
}

/// Splits an account's display name into Calagopus's first and last name.
pub fn split_name(
    display_name: &str,
) -> (
    Option<compact_str::CompactString>,
    Option<compact_str::CompactString>,
) {
    let display_name = display_name.trim();

    match display_name.split_once(char::is_whitespace) {
        Some((first, last)) if !last.trim().is_empty() => {
            (Some(first.into()), Some(last.trim().into()))
        }
        _ if display_name.is_empty() => (None, None),
        _ => (Some(display_name.into()), None),
    }
}

/// A Calagopus username (3 to 15 of `[a-zA-Z0-9_]`) made from an account's display name.
pub fn username_base(display_name: &str) -> String {
    let mut base: String = display_name
        .trim()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '-' || c == '.' {
                '_'
            } else {
                c
            }
        })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();

    while base.contains("__") {
        base = base.replace("__", "_");
    }

    let base: String = base.trim_matches('_').chars().take(15).collect();

    if base.len() < 3 { "user".into() } else { base }
}

/// The first free username for a display name: `name`, `name_2`, `name_3`, …
pub async fn unique_username(
    database: &crate::database::Database,
    display_name: &str,
) -> Result<String, anyhow::Error> {
    let base = username_base(display_name);

    for n in 1..100u32 {
        let candidate = if n == 1 {
            base.clone()
        } else {
            let suffix = format!("_{n}");
            format!("{}{suffix}", &base[..base.len().min(15 - suffix.len())])
        };

        if crate::models::user::User::by_username(database, &candidate)
            .await?
            .is_none()
        {
            return Ok(candidate);
        }
    }

    let suffix = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 6);

    Ok(format!("{}_{suffix}", &base[..base.len().min(8)]))
}

/// Brings a panel user in step with its account's snapshot: the link row, the admin flag, the
/// email, and the name and avatar when they changed on noprofit.space. The username stays what it
/// was made as: it is the user's SFTP login.
pub async fn apply_snapshot(
    state: &crate::State,
    user: &mut crate::models::user::User,
    snapshot: &Snapshot,
) -> Result<(), anyhow::Error> {
    use crate::models::UpdatableModel;

    let previous = Link::by_user_uuid(&state.database, user.uuid).await?;
    Link::store(&state.database, user.uuid, snapshot).await?;

    let (name_first, name_last) = split_name(&snapshot.username);
    let mut options = crate::models::user::UpdateUserOptions::default();
    let mut changed = false;

    if user.admin != snapshot.is_admin {
        options.admin = Some(snapshot.is_admin);
        changed = true;
    }
    // The name follows noprofit.space when it changes there; an edit here stands until then.
    let name_changed = previous
        .as_ref()
        .is_none_or(|link| link.username != snapshot.username);

    if name_changed && (user.name_first != name_first || user.name_last != name_last) {
        options.name_first = Some(name_first);
        options.name_last = Some(name_last);
        changed = true;
    }

    let email_changed = !user.email.eq_ignore_ascii_case(&snapshot.email);

    if changed && let Err(err) = user.update(state, options).await {
        tracing::warn!(user = %user.uuid, "failed to bring a user in step with noprofit.space: {err:#?}");
    }

    if email_changed {
        let options = crate::models::user::UpdateUserOptions {
            email: Some(snapshot.email.as_str().into()),
            ..Default::default()
        };

        // Another panel user may have this email; then it stays as it was.
        if let Err(err) = user.update(state, options).await {
            tracing::warn!(user = %user.uuid, "failed to take over the noprofit.space email: {err:#?}");
        }
    }

    let avatar_changed = previous.as_ref().map(|link| &link.avatar) != Some(&snapshot.avatar);

    if avatar_changed
        && !user.frozen
        && let Some(source_url) = snapshot.avatar.clone()
    {
        let state = std::sync::Arc::clone(state);
        let user_uuid = user.uuid;
        let previous_avatar = user.avatar.clone();

        tokio::spawn(async move {
            if let Err(err) = crate::models::user::User::import_avatar_by_uuid(
                &state,
                user_uuid,
                previous_avatar.as_deref(),
                &source_url,
            )
            .await
            {
                tracing::warn!(user = %user_uuid, "failed to import the noprofit.space avatar: {err:#?}");
            }
        });
    }

    Ok(())
}

/// Pulls every linked account's snapshot and applies it; suspends the users whose account
/// noprofit.space no longer has. Runs every minute on the primary instance (backend tasks.rs).
pub async fn sync_all(state: &crate::State) -> Result<(), anyhow::Error> {
    use crate::models::{ByUuid, UpdatableModel};

    let Some(noprofit) = state
        .settings
        .get()
        .await?
        .noprofit
        .config()
        .filter(|noprofit| noprofit.sync_enabled())
    else {
        return Ok(());
    };

    let account_ids = Link::live_account_ids(&state.database).await?;

    for batch in account_ids.chunks(PULL_BATCH) {
        let snapshots = noprofit.pull(&state.client, batch).await?;
        let found: std::collections::HashSet<i64> = snapshots
            .iter()
            .map(|snapshot| snapshot.account_id)
            .collect();

        for snapshot in &snapshots {
            let Some(link) = Link::by_account_id(&state.database, snapshot.account_id).await?
            else {
                continue;
            };
            let Some(mut user) =
                crate::models::user::User::by_uuid_optional(&state.database, link.user_uuid)
                    .await?
            else {
                continue;
            };

            if let Err(err) = apply_snapshot(state, &mut user, snapshot).await {
                tracing::warn!(user = %user.uuid, "failed to apply a noprofit.space snapshot: {err:#?}");
            }
        }

        let missing: Vec<i64> = batch
            .iter()
            .copied()
            .filter(|id| !found.contains(id))
            .collect();

        for user_uuid in Link::mark_deleted(&state.database, &missing).await? {
            let Some(mut user) =
                crate::models::user::User::by_uuid_optional(&state.database, user_uuid).await?
            else {
                continue;
            };

            tracing::info!(user = %user.uuid, "noprofit.space no longer has this account, suspending its user");

            let options = crate::models::user::UpdateUserOptions {
                suspended: Some(true),
                ..Default::default()
            };

            if let Err(err) = user.update(state, options).await {
                tracing::warn!(user = %user.uuid, "failed to suspend the user of a deleted noprofit.space account: {err:#?}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            accounts_url: "https://accounts.noprofit.space".into(),
            client: "plugged-host-calagopus".into(),
            audience: "plugged-host-calagopus:sso".into(),
            callback_url: None,
            admins_only: false,
            link_by_email: false,
            sso_secret: "test-secret-0123456789abcdef0123456789abcdef".into(),
            api_secret: Some("test-secret-0123456789abcdef0123456789abcdef".into()),
        }
    }

    fn token(claims: serde_json::Value, secret: &str) -> String {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());

        format!("{payload}.{}", hex::encode(mac.finalize().into_bytes()))
    }

    fn claims(now: i64) -> serde_json::Value {
        serde_json::json!({
            "account_user_id": 42,
            "account_id": 42,
            "email": "alice@example.com",
            "username": "Alice B/é",
            "is_admin": true,
            "avatar": null,
            "credits": 1234,
            "plan": {"cpu": 400, "memory": 12288, "disk": 51200},
            "discord": null,
            "state": "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789abcd",
            "aud": "plugged-host-calagopus:sso",
            "iat": now,
            "exp": now + 300,
            "nonce": "0123456789abcdef0123456789abcdef",
        })
    }

    #[test]
    fn names_and_usernames_from_display_names() {
        assert_eq!(
            split_name("Cala Gopus"),
            (Some("Cala".into()), Some("Gopus".into()))
        );
        assert_eq!(split_name("cloudy"), (Some("cloudy".into()), None));
        assert_eq!(username_base("Cala Gopus"), "Cala_Gopus");
        assert_eq!(username_base("Alice B/é"), "Alice_B");
        assert_eq!(username_base("éé"), "user");
        assert_eq!(username_base("a.very-long display name"), "a_very_long_dis");
    }

    /// The test vector noprofit.space's own code produces (PHP json_encode escapes `/` and `é`).
    #[test]
    fn verifies_a_token_signed_by_noprofit_space() {
        let token = "eyJhY2NvdW50X3VzZXJfaWQiOjQyLCJlbWFpbCI6ImFsaWNlQGV4YW1wbGUuY29tIiwidXNlcm5hbWUiOiJBbGljZSBCXC9cdTAwZTkiLCJpc19hZG1pbiI6ZmFsc2UsInN0YXRlIjoiQWJDZEVmR2hJaktsTW5PcFFyU3RVdld4WXowMTIzNDU2Nzg5YWJjZCIsImF1ZCI6InBsdWdnZWQtaG9zdC1jYWxhZ29wdXM6c3NvIiwiaWF0IjoxNzYwMDAwMDAwLCJleHAiOjE3NjAwMDAzMDAsIm5vbmNlIjoiMDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWYifQ.d7967cf6992909d06af9d2e9ed9a27ff51567e8f6df075d8d83a3ea79b9f879c";

        // Long expired, but everything before the lifetime check passes.
        assert!(matches!(
            config().decode_token(token),
            Err(TokenError::Expired)
        ));
    }

    #[test]
    fn decodes_a_fresh_token() {
        let now = chrono::Utc::now().timestamp();
        let claims = config()
            .decode_token(&token(
                claims(now),
                "test-secret-0123456789abcdef0123456789abcdef",
            ))
            .unwrap();

        assert_eq!(claims.snapshot.account_id, 42);
        assert_eq!(claims.snapshot.username, "Alice B/é");
        assert!(claims.snapshot.is_admin);
        assert_eq!(claims.snapshot.credits, 1234);
        assert_eq!(claims.state, "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789abcd");
    }

    #[test]
    fn refuses_other_secrets_audiences_and_old_tokens() {
        let now = chrono::Utc::now().timestamp();

        assert!(matches!(
            config().decode_token(&token(
                claims(now),
                "another-secret-0123456789abcdef0123456789"
            )),
            Err(TokenError::BadSignature)
        ));

        let mut other = claims(now);
        other["aud"] = "plugged-host:sso".into();
        assert!(matches!(
            config().decode_token(&token(
                other,
                "test-secret-0123456789abcdef0123456789abcdef"
            )),
            Err(TokenError::WrongAudience)
        ));

        assert!(matches!(
            config().decode_token(&token(
                claims(now - 1000),
                "test-secret-0123456789abcdef0123456789abcdef"
            )),
            Err(TokenError::Expired)
        ));

        assert!(matches!(
            config().decode_token("nope"),
            Err(TokenError::Malformed)
        ));
    }

    /// noprofit.space's own request-signature test vector.
    #[test]
    fn signs_requests_like_noprofit_space() {
        assert_eq!(
            request_signature(
                "test-secret-0123456789abcdef0123456789abcdef",
                "POST",
                "/api/noprofit/account/delete",
                "1760000000",
                "abcdefghijklmnop0123456789ABCDEF",
                br#"{"account_id":42}"#,
            ),
            "bf6d6ccbaa9363281b768107160b56d9d764cbf1d4a24a550363de8a369af746"
        );
    }
}
