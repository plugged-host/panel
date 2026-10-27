//! plugged.host: noprofit.space accounts.
//!
//! Optional, like the noprofit plugin on plugged.host's Pelican panel. While Admin › Settings ›
//! Accounts › noprofit.space leaves it off (or misses the accounts URL, the client or a sign-in
//! secret of at least 32 characters), the panel signs people in on its own (Calagopus's login and
//! registration). Connected, noprofit.space owns sign-in:
//!
//! - everyone signs in at noprofit.space (see [`Config::decode_token`]) and an account's first
//!   sign-in creates its panel user,
//! - registration is off and password sign-in is left to admins without a noprofit.space account
//!   (the emergency admin made with `panel-rs users create`); the panel's own OAuth providers and
//!   security keys are off,
//! - with the API secret the panel keeps its users in step by pulling their snapshots (profile,
//!   admin flag, plan, credits, Discord; [`Config::pull`], every minute), suspends the users whose
//!   account noprofit.space no longer has (and deletes them after the days the settings give),
//!   and lets noprofit.space check the SFTP passwords of its users, who have none on the panel
//!   ([`Config::verify_password`]),
//! - with the API secret noprofit.space also calls the panel: what deleting an account deletes
//!   here, and deleting it ([`account_summary`], [`delete_account`], `/api/noprofit/account/*`,
//!   signed like the panel's own calls, [`Config::verify`]),
//! - the credits the panel gives or takes itself (Discord invite rewards and their clawbacks, an
//!   admin's change) are queued and sent to noprofit.space, which holds the balance
//!   ([`queue_credit_change`], [`push_credit_changes`]),
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
    /// neo's gateway (docs/features/neo.md); its own secret, signed like the API.
    neo_secret: Option<String>,
}

impl Config {
    /// The connection the noprofit.space settings describe, or `None` when they leave sign-in to
    /// the panel (Admin › Settings › Accounts › noprofit.space, [`crate::settings::noprofit`]).
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
            neo_secret: settings
                .neo_secret
                .as_deref()
                .map(str::trim)
                .filter(|secret| secret.len() >= MIN_SECRET_LENGTH)
                .map(str::to_string),
        })
    }

    /// Whether neo runs on noprofit.space (its gateway secret is set).
    #[inline]
    pub fn neo_enabled(&self) -> bool {
        self.neo_secret.is_some()
    }

    /// The `X-NP-*` headers for a request to noprofit.space's neo gateway, or `None` without its
    /// secret.
    pub fn sign_neo_request(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Option<[(&'static str, String); 3]> {
        let secret = self.neo_secret.as_ref()?;
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
        let timeout = std::time::Duration::from_secs(8);

        self.post_within(client, path, body, timeout).await
    }

    /// [`Self::post`] with its own time limit (a page waiting for the answer gives up sooner).
    async fn post_within(
        &self,
        client: &reqwest::Client,
        path: &str,
        body: &serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<reqwest::Response, anyhow::Error> {
        let body = serde_json::to_vec(body)?;
        let headers = self
            .sign_request("POST", path, &body)
            .ok_or_else(|| anyhow::anyhow!("the noprofit.space API secret is not set"))?;

        let mut request = client
            .post(format!("{}{path}", self.accounts_url))
            .timeout(timeout)
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
        usage: &[Usage],
    ) -> Result<Vec<Snapshot>, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/accounts";

        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .post(
                client,
                PATH,
                &serde_json::json!({ "account_ids": account_ids, "usage": usage }),
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

    /// The account's Resource Store on noprofit.space: its credits, what each resource costs
    /// it now (`discount` percent off, the panel's sale) and its latest credit history
    /// (docs/features/economy.md).
    pub async fn store(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        discount: i64,
    ) -> Result<Store, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/store";

        let mut body = serde_json::json!({ "account_id": account_id });
        if discount > 0 {
            body["discount"] = discount.into();
        }

        let response = self.post(client, PATH, &body).await?;
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("noprofit.space answered {status} to the store");
        }

        Ok(response.json().await?)
    }

    /// Buys `steps` steps of a resource at the price the player was shown, `discount` percent
    /// off. A price that moved since buys nothing.
    pub async fn purchase(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        resource: crate::pricing::StoreResource,
        steps: i64,
        price: i64,
        discount: i64,
    ) -> Result<Purchase, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/purchase";

        let mut body = serde_json::json!({
            "account_id": account_id,
            "resource": resource,
            "steps": steps,
            "price": price,
        });
        if discount > 0 {
            body["discount"] = discount.into();
        }

        let response = self.post(client, PATH, &body).await?;
        let status = response.status();

        match status {
            status if status.is_success() => Ok(Purchase::Done(response.json().await?)),
            reqwest::StatusCode::CONFLICT => {
                let answer: serde_json::Value = response.json().await.unwrap_or_default();

                Ok(Purchase::PriceChanged(serde_json::from_value(answer).ok()))
            }
            reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                let answer: serde_json::Value = response.json().await.unwrap_or_default();

                Ok(Purchase::Refused(
                    answer
                        .get("message")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ))
            }
            status => anyhow::bail!("noprofit.space answered {status} to a store purchase"),
        }
    }

    /// Takes `amount` credits off the account for something bought on this panel (a membership).
    /// Sent again with the same `key`, nothing more is taken. `None` when the balance is short.
    pub async fn spend_credits(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        amount: i64,
        kind: &str,
        description: &str,
        key: &str,
    ) -> Result<Option<i64>, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/credits/spend";

        let response = self
            .post(
                client,
                PATH,
                &serde_json::json!({
                    "account_id": account_id,
                    "amount": amount,
                    "type": kind,
                    "description": description,
                    "idempotency_key": key,
                }),
            )
            .await?;
        let status = response.status();

        #[derive(Deserialize)]
        struct Spent {
            credits: i64,
        }

        match status {
            status if status.is_success() => Ok(Some(response.json::<Spent>().await?.credits)),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY => Ok(None),
            status => anyhow::bail!("noprofit.space answered {status} to a credit spend"),
        }
    }

    /// The account's standing (docs/features/standing.md). A page waits for it, so it gives up
    /// after a few seconds.
    pub async fn standing(
        &self,
        client: &reqwest::Client,
        account_id: i64,
    ) -> Result<crate::standing::Standing, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/standing";

        let response = self
            .post_within(
                client,
                PATH,
                &serde_json::json!({ "account_id": account_id }),
                std::time::Duration::from_secs(4),
            )
            .await?;
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("noprofit.space answered {status} to a standing request");
        }

        let answer: serde_json::Value = response.json().await?;

        match crate::standing::Standing::from_answer(&answer) {
            Some(standing) => Ok(standing),
            None => anyhow::bail!("noprofit.space's answer has no standing"),
        }
    }

    /// Reports a suspension made here as a strike on the account (docs/features/standing.md):
    /// `reference` (`server:<uuid>`) counts once however often it is sent. An `Err` is worth
    /// sending again (unreachable, refused secret, throttled, a 5xx).
    pub async fn report_strike(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        reference: &str,
        reason: &str,
    ) -> Result<StrikeAnswer, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/standing/strikes";

        let response = self
            .post(
                client,
                PATH,
                &serde_json::json!({
                    "account_id": account_id,
                    "reference": reference,
                    "reason": reason,
                }),
            )
            .await?;
        let status = response.status();

        match status {
            // 201 recorded now, 200 recorded before
            status if status.is_success() => {
                let answer: serde_json::Value = response.json().await.unwrap_or_default();

                match crate::standing::Standing::from_answer(&answer) {
                    Some(standing) => Ok(StrikeAnswer::Recorded(standing)),
                    None => anyhow::bail!("noprofit.space's answer to a strike has no standing"),
                }
            }
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::UNPROCESSABLE_ENTITY => {
                Ok(StrikeAnswer::Refused(status))
            }
            status => anyhow::bail!("noprofit.space answered {status} to a strike"),
        }
    }

    /// Whether noprofit.space signed this request to the panel (`/api/noprofit/*`) with the API
    /// secret, at most [`REQUEST_TOLERANCE`] seconds from `now`. Without an API secret nothing
    /// is. The caller still has to accept each nonce once.
    pub fn verify(&self, request: &SignedRequest<'_>, now: i64) -> bool {
        self.api_secret
            .as_deref()
            .is_some_and(|secret| request.signed_with(secret, now))
    }

    /// Gives (or, negative, takes) credits made on this panel (a Discord invite reward, its
    /// clawback, an admin's change). Once per `key`, however often it is sent; noprofit.space
    /// takes at most what the balance holds and says how much it applied.
    /// [`CreditAnswer::Failed`] is worth trying again, an `Err` stops this round of sending
    /// (unreachable, refused secret, throttled).
    #[allow(clippy::too_many_arguments)]
    pub async fn apply_credits(
        &self,
        client: &reqwest::Client,
        account_id: i64,
        user_uuid: uuid::Uuid,
        amount: i64,
        kind: &str,
        description: &str,
        key: &str,
    ) -> Result<CreditAnswer, anyhow::Error> {
        const PATH: &str = "/api/v1/internal/calagopus/credits/apply";

        let response = self
            .post(
                client,
                PATH,
                &serde_json::json!({
                    "account_id": account_id,
                    "user_uuid": user_uuid,
                    "delta": amount,
                    "type": kind,
                    "description": description,
                    "idempotency_key": key,
                }),
            )
            .await?;
        let status = response.status();

        #[derive(Deserialize)]
        struct Applied {
            applied: i64,
            credits: i64,
        }

        match status {
            status if status.is_success() => {
                let answer = response.json::<Applied>().await?;

                Ok(CreditAnswer::Applied {
                    applied: answer.applied,
                    credits: answer.credits,
                })
            }
            reqwest::StatusCode::NOT_FOUND
            | reqwest::StatusCode::CONFLICT
            | reqwest::StatusCode::UNPROCESSABLE_ENTITY => Ok(CreditAnswer::Refused(status)),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::TOO_MANY_REQUESTS => {
                anyhow::bail!("noprofit.space answered {status} to a credit change")
            }
            status => Ok(CreditAnswer::Failed(status)),
        }
    }
}

/// How far the timestamp of a request noprofit.space signed may be from the panel's clock, in
/// seconds (noprofit.space's own tolerance).
pub const REQUEST_TOLERANCE: i64 = 60;

/// A request to the panel signed by noprofit.space (`X-NP-*` headers), as it arrived.
pub struct SignedRequest<'a> {
    pub method: &'a str,
    /// The path without the query, as noprofit.space signed it (`/api/noprofit/account/delete`).
    pub path: &'a str,
    pub timestamp: &'a str,
    pub nonce: &'a str,
    pub signature: &'a str,
    pub body: &'a [u8],
}

impl SignedRequest<'_> {
    /// Whether `secret` signed it: a timestamp at most [`REQUEST_TOLERANCE`] seconds from
    /// `now`, a nonce of 16 to 64 of `[A-Za-z0-9_-]` and the signature over all of it
    /// ([`request_signature`]). Short secrets reject everything.
    pub fn signed_with(&self, secret: &str, now: i64) -> bool {
        if secret.len() < MIN_SECRET_LENGTH
            || self.timestamp.is_empty()
            || !self.timestamp.bytes().all(|b| b.is_ascii_digit())
            || !(16..=64).contains(&self.nonce.len())
            || !self
                .nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return false;
        }
        let Ok(timestamp) = self.timestamp.parse::<i64>() else {
            return false;
        };
        if (now - timestamp).abs() > REQUEST_TOLERANCE {
            return false;
        }
        let Ok(signature) = hex::decode(self.signature) else {
            return false;
        };

        let canonical = format!(
            "{}\n/{}\n{}\n{}\n{}",
            self.method.to_ascii_uppercase(),
            self.path.trim_start_matches('/'),
            self.timestamp,
            self.nonce,
            hex::encode(Sha256::digest(self.body)),
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("hmac accepts keys of any length");
        mac.update(canonical.as_bytes());

        mac.verify_slice(&signature).is_ok()
    }
}

/// noprofit.space's answer to a strike ([`Config::report_strike`]).
#[derive(Debug, Clone)]
pub enum StrikeAnswer {
    /// Recorded, now or before under the same reference: the account's standing after it.
    Recorded(crate::standing::Standing),
    /// Refused for good: no such account (404) or invalid (422).
    Refused(reqwest::StatusCode),
}

/// noprofit.space's answer to a credit change ([`Config::apply_credits`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditAnswer {
    /// Applied, now or before under the same key: `applied` is what was applied then (a
    /// deduction takes at most the balance), `credits` the balance now.
    Applied { applied: i64, credits: i64 },
    /// Refused for good: no such account (404), the key belongs to another change (409), invalid
    /// (422).
    Refused(reqwest::StatusCode),
    /// Not applied this time (a 5xx): sent again later.
    Failed(reqwest::StatusCode),
}

/// noprofit.space's Resource Store for one account.
#[derive(Deserialize, Clone, Debug)]
pub struct Store {
    pub credits: i64,
    pub catalog: Vec<crate::pricing::CatalogEntry>,
    #[serde(default)]
    pub transactions: Vec<StoreTransaction>,
    /// The account's Discord rewards, when noprofit.space sends them: the store page offers what
    /// is left of them (docs/features/economy.md#the-resource-store).
    #[serde(default)]
    pub discord: Option<StoreDiscord>,
    /// The percent noprofit.space took off the prices; 0 when it applied none.
    #[serde(default)]
    pub discount: i64,
}

/// What the account's Discord membership adds to its plan and could still add (accounts
/// `DiscordGuilds::status`), and the credits for invites and boosts.
#[derive(Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct StoreDiscord {
    /// What the Discord servers the account is in add to its plan.
    pub bonus: crate::allowance::Resources,
    /// What being in all of them would add.
    pub max_bonus: crate::allowance::Resources,
    /// Credits for each person the account invites to the Discord server (0: none).
    pub credits_per_invite: i64,
    /// Credits for a boost (0: none).
    pub credits_per_boost: i64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StoreTransaction {
    pub amount: i64,
    pub balance_after: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub enum Purchase {
    Done(Store),
    /// The price moved; the store as it is now, when noprofit.space sent it.
    PriceChanged(Option<Store>),
    /// Not for sale, or the balance is short; noprofit.space's reason.
    Refused(String),
}

/// Pulls one linked user's snapshot again and applies it, after something changed their credits
/// or plan on noprofit.space.
pub async fn refresh_user(
    state: &crate::State,
    user_uuid: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;

    let Some(noprofit) = state.settings.get().await?.noprofit.config() else {
        return Ok(());
    };
    let Some(link) = Link::by_user_uuid(&state.database, user_uuid).await? else {
        return Ok(());
    };
    let usage = Usage::of(&state.database, &[link.account_id]).await?;
    let Some(snapshot) = noprofit
        .pull(&state.client, &[link.account_id], &usage)
        .await?
        .into_iter()
        .find(|snapshot| snapshot.account_id == link.account_id)
    else {
        return Ok(());
    };
    let Some(mut user) =
        crate::models::user::User::by_uuid_optional(&state.database, user_uuid).await?
    else {
        return Ok(());
    };

    apply_snapshot(state, &mut user, &snapshot).await
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
    /// `{cpu, memory, disk, base_*, max_*, total_*, elsewhere_*, cut_*}`: CPU in %, memory and
    /// disk in MiB. cpu/memory/disk are this panel's share of the account's pool, `total_*` the
    /// whole pool, `elsewhere_*` what its other services use, `cut_*` what lower bases took from
    /// this panel so far (only counting up).
    #[serde(default)]
    pub plan: Option<serde_json::Value>,
    /// `{id, username, avatar}` when a Discord account is linked.
    #[serde(default)]
    pub discord: Option<serde_json::Value>,
}

/// What an account's servers here use, sent with every pull: noprofit.space counts it in the pool
/// the account shares with its other services (switched.cloud, the Pelican panel), which then
/// have that much less, as this panel has less for theirs (`elsewhere_*` in the plan).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Usage {
    pub account_id: i64,
    pub user_uuid: uuid::Uuid,
    pub cpu: i64,
    pub memory: i64,
    pub disk: i64,
    pub servers: i64,
}

impl Usage {
    /// The usage of these accounts' users (0 for one without servers).
    pub async fn of(
        database: &crate::database::Database,
        account_ids: &[i64],
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT n.account_id, n.user_uuid,
                COALESCE(SUM(s.cpu), 0)::BIGINT AS cpu,
                COALESCE(SUM(s.memory), 0)::BIGINT AS memory,
                COALESCE(SUM(s.disk), 0)::BIGINT AS disk,
                COUNT(s.uuid)::BIGINT AS servers
            FROM user_noprofit_accounts n
            LEFT JOIN servers s ON s.owner_uuid = n.user_uuid
            WHERE n.account_id = ANY($1) AND n.deleted IS NULL
            GROUP BY n.account_id, n.user_uuid",
        )
        .bind(account_ids)
        .fetch_all(database.read())
        .await
    }
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

    /// Every account noprofit.space was found to no longer have, longest gone first.
    pub async fn deleted_account_ids(
        database: &crate::database::Database,
    ) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT account_id FROM user_noprofit_accounts WHERE deleted IS NOT NULL ORDER BY deleted ASC",
        )
        .fetch_all(database.read())
        .await
    }

    /// Links a user to an account, or refreshes the link, with the account's snapshot. A smaller
    /// plan than before is written down for the servers to give back
    /// ([`crate::allowance_cuts`]).
    pub async fn store(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        snapshot: &Snapshot,
    ) -> Result<(), sqlx::Error> {
        // the CTE reads the row as it was before this statement changes it
        let old_plan: Option<serde_json::Value> = sqlx::query_scalar(
            "WITH old AS (SELECT plan FROM user_noprofit_accounts WHERE user_uuid = $1)
            INSERT INTO user_noprofit_accounts
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
                synced = NOW()
            RETURNING (SELECT plan FROM old)",
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
        .fetch_one(database.write())
        .await?;

        if let (Some(old), Some(new)) = (&old_plan, &snapshot.plan) {
            let decrease = crate::allowance_cuts::plan_decrease(old, new);
            crate::allowance_cuts::record(
                database,
                user_uuid,
                decrease,
                crate::allowance_cuts::REASON_PLAN,
            )
            .await?;
        }

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
/// noprofit.space no longer has, with their servers, until the deletion time the settings give
/// (`deleted_account_days`). Accounts found missing before are asked about again: one that is
/// back is restored, one still missing once the days are over is deleted with its servers
/// ([`delete_account`]). Sends the panel's credit changes first ([`push_credit_changes`]). Runs
/// every minute on the primary instance (backend tasks.rs).
pub async fn sync_all(state: &crate::State) -> Result<(), anyhow::Error> {
    use crate::models::ByUuid;

    let (noprofit, deleted_days) = {
        let settings = state.settings.get().await?;

        (
            settings
                .noprofit
                .config()
                .filter(|noprofit| noprofit.sync_enabled()),
            settings.noprofit.deleted_account_days,
        )
    };
    let Some(noprofit) = noprofit else {
        return Ok(());
    };

    // first, so the snapshots below already count them
    if let Err(err) = push_credit_changes(state).await {
        tracing::warn!("failed to send credit changes to noprofit.space: {err:#}");
    }
    // abuse suspensions noprofit.space has not confirmed as strikes yet (docs/features/standing.md)
    if let Err(err) = crate::standing::send_pending(state, &noprofit).await {
        tracing::warn!("failed to send strikes to noprofit.space: {err:#}");
    }

    let account_ids = Link::live_account_ids(&state.database).await?;
    let delete_at = deletion_time(chrono::Utc::now(), deleted_days);

    for batch in account_ids.chunks(PULL_BATCH) {
        let usage = Usage::of(&state.database, batch).await?;
        let snapshots = noprofit.pull(&state.client, batch, &usage).await?;
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
            suspend_vanished(state, user_uuid, delete_at).await;
        }
    }

    // the accounts found missing before: back, or (after the days) deleted here too
    let gone = Link::deleted_account_ids(&state.database).await?;
    for batch in gone.chunks(PULL_BATCH) {
        for snapshot in noprofit.pull(&state.client, batch, &[]).await? {
            restore_vanished(state, &snapshot).await;
        }
    }

    if deleted_days > 0 {
        let due: Vec<i64> = sqlx::query_scalar(
            "SELECT account_id FROM user_noprofit_accounts
            WHERE deleted IS NOT NULL AND deleted <= NOW() - make_interval(days => $1)
            ORDER BY synced
            LIMIT $2",
        )
        .bind(deleted_days)
        .bind(DELETE_BATCH)
        .fetch_all(state.database.write())
        .await?;

        for account_id in due {
            match delete_account(state, account_id).await {
                Ok(AccountDeletion::Deleted { servers }) => {
                    tracing::info!(
                        account_id,
                        servers,
                        "deleted the user of a noprofit.space account that is gone"
                    );
                }
                Ok(AccountDeletion::Refused(reason)) => {
                    tracing::warn!(
                        account_id,
                        "the user of a noprofit.space account that is gone was not deleted: {reason}"
                    );
                    retry_later(state, account_id).await;
                }
                Err(err) => {
                    tracing::warn!(
                        account_id,
                        "failed to delete the user of a noprofit.space account that is gone: {err:#}"
                    );
                    retry_later(state, account_id).await;
                }
            }
        }
    }

    Ok(())
}

/// Users of accounts gone from noprofit.space deleted per run of [`sync_all`] at most.
const DELETE_BATCH: i64 = 10;

/// Puts a deletion that did not go through at the back of the line (`synced` orders them), so
/// it never holds up the others.
async fn retry_later(state: &crate::State, account_id: i64) {
    if let Err(err) =
        sqlx::query("UPDATE user_noprofit_accounts SET synced = NOW() WHERE account_id = $1")
            .bind(account_id)
            .execute(state.database.write())
            .await
    {
        tracing::warn!(
            account_id,
            "failed to put a noprofit.space deletion back in line: {err:#}"
        );
    }
}

/// When the servers of an account found missing now are deleted: `days` later, never for 0.
pub fn deletion_time(
    now: chrono::DateTime<chrono::Utc>,
    days: i32,
) -> Option<chrono::DateTime<chrono::Utc>> {
    (days > 0).then(|| now + chrono::Duration::days(days as i64))
}

/// noprofit.space no longer has the account of this user: the user is suspended (noted in the
/// link, so a return lifts it) and its servers too, until `delete_at`.
async fn suspend_vanished(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    delete_at: Option<chrono::DateTime<chrono::Utc>>,
) {
    use crate::models::{ByUuid, UpdatableModel};

    let mut user = match crate::models::user::User::by_uuid_optional(&state.database, user_uuid)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(user = %user_uuid, "failed to load the user of a deleted noprofit.space account: {err:#}");
            return;
        }
    };

    tracing::info!(user = %user.uuid, "noprofit.space no longer has this account, suspending its user");

    // a user suspended already (by an admin) stays so if the account comes back
    if !user.suspended {
        let options = crate::models::user::UpdateUserOptions {
            suspended: Some(true),
            ..Default::default()
        };

        match user.update(state, options).await {
            Ok(()) => {
                if let Err(err) = sqlx::query(
                    "UPDATE user_noprofit_accounts SET user_suspended = true WHERE user_uuid = $1",
                )
                .bind(user.uuid)
                .execute(state.database.write())
                .await
                {
                    tracing::warn!(user = %user.uuid, "failed to note the suspension of a deleted noprofit.space account: {err:#}");
                }
            }
            Err(err) => {
                tracing::warn!(user = %user.uuid, "failed to suspend the user of a deleted noprofit.space account: {err:#?}");
            }
        }
    }

    // plugged.host: its servers stop too (docs/features/suspension.md)
    if let Err(err) =
        crate::models::server::suspension::suspend_owned_for_noprofit(state, user.uuid, delete_at)
            .await
    {
        tracing::warn!(user = %user.uuid, "failed to suspend the servers of a deleted noprofit.space account: {err:#?}");
    }
}

/// An account found missing before is in noprofit.space's answer again (it was left out by
/// mistake): its link is live again, and the suspensions the sync made are lifted.
async fn restore_vanished(state: &crate::State, snapshot: &Snapshot) {
    use crate::models::{ByUuid, UpdatableModel};

    let link = match Link::by_account_id(&state.database, snapshot.account_id).await {
        Ok(Some(link)) if link.deleted.is_some() => link,
        Ok(_) => return,
        Err(err) => {
            tracing::warn!(
                account_id = snapshot.account_id,
                "failed to load a noprofit.space link: {err:#}"
            );
            return;
        }
    };
    let mut user = match crate::models::user::User::by_uuid_optional(
        &state.database,
        link.user_uuid,
    )
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(user = %link.user_uuid, "failed to load the user of a noprofit.space account: {err:#}");
            return;
        }
    };

    tracing::info!(user = %user.uuid, "noprofit.space has this account again, lifting its suspension");

    // the link is live again (Link::store)
    if let Err(err) = apply_snapshot(state, &mut user, snapshot).await {
        tracing::warn!(user = %user.uuid, "failed to apply a noprofit.space snapshot: {err:#?}");
        return;
    }

    // the CTE reads the row as it was before this statement changes it
    let suspended_by_sync = match sqlx::query_scalar::<_, bool>(
        "WITH old AS (SELECT user_suspended FROM user_noprofit_accounts WHERE user_uuid = $1)
        UPDATE user_noprofit_accounts SET user_suspended = false
        WHERE user_uuid = $1
        RETURNING (SELECT user_suspended FROM old)",
    )
    .bind(user.uuid)
    .fetch_optional(state.database.write())
    .await
    {
        Ok(suspended) => suspended.unwrap_or(false),
        Err(err) => {
            tracing::warn!(user = %user.uuid, "failed to read the suspension of a noprofit.space account: {err:#}");
            return;
        }
    };

    if suspended_by_sync && user.suspended {
        let options = crate::models::user::UpdateUserOptions {
            suspended: Some(false),
            ..Default::default()
        };

        if let Err(err) = user.update(state, options).await {
            tracing::warn!(user = %user.uuid, "failed to lift the suspension of a noprofit.space account: {err:#?}");
        }
    }

    if let Err(err) =
        crate::models::server::suspension::unsuspend_owned_for_noprofit(state, user.uuid).await
    {
        tracing::warn!(user = %user.uuid, "failed to lift the suspension of a noprofit.space account's servers: {err:#}");
    }
}

/// What deleting an account on noprofit.space also deletes here, for its confirmation page:
/// the panel user's name and its servers' names. `None` when no user here has the account.
pub async fn account_summary(
    state: &crate::State,
    account_id: i64,
) -> Result<Option<(compact_str::CompactString, Vec<compact_str::CompactString>)>, anyhow::Error> {
    use crate::models::ByUuid;

    let Some(link) = Link::by_account_id(&state.database, account_id).await? else {
        return Ok(None);
    };
    let Some(user) =
        crate::models::user::User::by_uuid_optional(&state.database, link.user_uuid).await?
    else {
        return Ok(None);
    };
    let servers: Vec<String> =
        sqlx::query_scalar("SELECT name FROM servers WHERE owner_uuid = $1 ORDER BY name")
            .bind(user.uuid)
            .fetch_all(state.database.read())
            .await?;

    Ok(Some((
        user.username,
        servers.into_iter().map(Into::into).collect(),
    )))
}

/// What [`delete_account`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountDeletion {
    /// The user is gone (or there was none), with this many servers.
    Deleted { servers: usize },
    /// Nothing more was deleted, for this reason (shown on noprofit.space). Servers deleted
    /// before it stay deleted; asking again carries on.
    Refused(String),
}

/// Deletes the panel user of a noprofit.space account, all the way: every server it owns (on
/// its node too, with all their backups: the owner is gone, nobody is left to keep them for),
/// the backups still kept of servers deleted before, the activity that names it (it carries
/// its addresses) and the user with everything that belongs to it. The panel's last admin is
/// never deleted. A server whose node cannot delete it stops the deletion there.
pub async fn delete_account(
    state: &crate::State,
    account_id: i64,
) -> Result<AccountDeletion, anyhow::Error> {
    use crate::models::{ByUuid, DeletableModel};

    // one at a time per account (noprofit.space asking again, the sync)
    let _lock = state
        .cache
        .lock(
            format!("noprofit::account_delete::{account_id}"),
            Some(600),
            None,
        )
        .await?;

    let Some(link) = Link::by_account_id(&state.database, account_id).await? else {
        return Ok(AccountDeletion::Deleted { servers: 0 });
    };
    let Some(user) =
        crate::models::user::User::by_uuid_optional(&state.database, link.user_uuid).await?
    else {
        return Ok(AccountDeletion::Deleted { servers: 0 });
    };

    if user.admin {
        let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE admin")
            .fetch_one(state.database.write())
            .await?;

        if admins <= 1 {
            return Ok(AccountDeletion::Refused(
                "This is the last administrator of the panel, so it cannot be deleted.".into(),
            ));
        }
    }

    let mut deleted = 0;
    loop {
        let servers = crate::models::server::Server::by_owner_uuid_with_pagination(
            &state.database,
            user.uuid,
            1,
            50,
            None,
        )
        .await?;
        if servers.data.is_empty() {
            break;
        }

        for server in servers.data {
            if let Err(err) = delete_server_with_backups(state, &server).await {
                tracing::warn!(user = %user.uuid, server = %server.uuid, "failed to delete a server of a deleted noprofit.space account: {err:#}");

                let mut reason = format!(
                    "The server \"{}\" could not be deleted, so the account was left alone.",
                    server.name
                );
                if deleted > 0 {
                    reason.push_str(&format!(
                        " {deleted} server(s) had already been deleted before that happened."
                    ));
                }

                return Ok(AccountDeletion::Refused(reason));
            }

            deleted += 1;
        }
    }

    // the copies of its servers deleted before, kept for it (shared/src/deleted_servers.rs)
    let held: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT server_backups.uuid
        FROM deleted_servers
        JOIN server_backups ON server_backups.uuid = ANY(deleted_servers.backups)
        WHERE deleted_servers.owner_uuid = $1
            AND server_backups.deleted IS NULL
            AND server_backups.deleting IS NULL",
    )
    .bind(user.uuid)
    .fetch_all(state.database.write())
    .await?;
    for backup_uuid in held {
        if let Some(backup) = crate::models::server_backup::ServerBackup::by_uuid_optional(
            &state.database,
            backup_uuid,
        )
        .await?
            && let Err(err) = backup.delete(state, Default::default()).await
        {
            tracing::warn!(user = %user.uuid, backup = %backup_uuid, "failed to delete a kept backup of a deleted noprofit.space account: {err:#}");
        }
    }

    for table in ["server_activities", "admin_activities"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE user_uuid = $1"
        )))
        .bind(user.uuid)
        .execute(state.database.write())
        .await?;
    }

    user.delete(state, ()).await?;

    tracing::info!(user = %user.uuid, account_id, servers = deleted, "deleted the panel user of a deleted noprofit.space account");

    Ok(AccountDeletion::Deleted { servers: deleted })
}

/// Deletes a server with all its backups, system backups included (the server delete of
/// upstream's admin route with "delete backups"). A backup that fails to delete is logged; the
/// server is gone either way.
pub async fn delete_server_with_backups(
    state: &crate::State,
    server: &crate::models::server::Server,
) -> Result<(), anyhow::Error> {
    use crate::models::DeletableModel;

    let backups = crate::models::server_backup::ServerBackup::all_by_server_uuid(
        &state.database,
        server.uuid,
    )
    .await?;

    server
        .delete(
            state,
            crate::models::server::DeleteServerOptions { force: false },
        )
        .await?;

    for backup in backups {
        let backup_uuid = backup.uuid;
        if let Err(err) = backup.delete(state, Default::default()).await {
            tracing::warn!(server = %server.uuid, backup = %backup_uuid, "failed to delete a backup of a deleted server: {err:#}");
        }
    }

    Ok(())
}

/// Credits changes sent per run of [`push_credit_changes`] at most.
const PUSH_BATCH: i64 = 100;

/// The balance a credit change may count on: what noprofit.space last said plus the changes it
/// has not confirmed yet. Locks the link row for the caller's transaction. `None` when the user
/// has no live noprofit.space account. Returns `(account_id, balance)`.
pub async fn pending_balance(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
) -> Result<Option<(i64, i64)>, sqlx::Error> {
    let link: Option<(i64, i64)> = sqlx::query_as(
        "SELECT account_id, credits FROM user_noprofit_accounts
        WHERE user_uuid = $1 AND deleted IS NULL
        FOR UPDATE",
    )
    .bind(user_uuid)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((account_id, credits)) = link else {
        return Ok(None);
    };
    let unsent: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM user_noprofit_credit_changes
        WHERE user_uuid = $1 AND sent IS NULL",
    )
    .bind(user_uuid)
    .fetch_one(&mut **transaction)
    .await?;

    Ok(Some((account_id, credits + unsent)))
}

/// Records a credit change made here while noprofit.space holds the balance (a Discord invite
/// reward, its clawback, an admin's change), in the caller's transaction. It is sent once the
/// transaction is committed ([`spawn_credit_push`]) or by the next account sync.
#[allow(clippy::too_many_arguments)]
pub async fn queue_credit_change(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
    account_id: i64,
    amount: i64,
    kind: &str,
    description: &str,
    data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let description: String = description.chars().take(255).collect();

    sqlx::query(
        "INSERT INTO user_noprofit_credit_changes
            (user_uuid, account_id, amount, type, description, data)
        VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_uuid)
    .bind(account_id)
    .bind(amount)
    .bind(kind)
    .bind(description)
    .bind(data)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Sends the queued credit changes now, in the background (after the transaction that queued
/// them is committed). What fails waits for the next account sync.
pub fn spawn_credit_push(state: &crate::State) {
    let state = std::sync::Arc::clone(state);

    tokio::spawn(async move {
        if let Err(err) = push_credit_changes(&state).await {
            tracing::warn!("failed to send credit changes to noprofit.space: {err:#}");
        }
    });
}

/// Sends the credit changes noprofit.space has not confirmed, oldest first, each under its own
/// key (so a resend applies nothing twice), and keeps what it answers: how much it applied and
/// the balance after, which is also the account's balance here now. A refused change (no such
/// account, invalid) is kept as sent with nothing applied. When noprofit.space cannot be reached
/// the round stops; a change it failed (5xx) holds back that user's later ones until the next
/// round, so their order is kept.
pub async fn push_credit_changes(state: &crate::State) -> Result<(), anyhow::Error> {
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

    // one round at a time; the other one sends these
    let Ok(_lock) = state
        .cache
        .lock("noprofit::credit_push", Some(120), Some(200))
        .await
    else {
        return Ok(());
    };

    #[derive(sqlx::FromRow)]
    struct Change {
        uuid: uuid::Uuid,
        user_uuid: uuid::Uuid,
        account_id: i64,
        amount: i64,
        #[sqlx(rename = "type")]
        kind: String,
        description: String,
    }

    let changes: Vec<Change> = sqlx::query_as(
        "SELECT uuid, user_uuid, account_id, amount, type, description
        FROM user_noprofit_credit_changes
        WHERE sent IS NULL
        ORDER BY created, uuid
        LIMIT $1",
    )
    .bind(PUSH_BATCH)
    .fetch_all(state.database.write())
    .await?;

    let mut held_back = std::collections::HashSet::new();
    for change in changes {
        if held_back.contains(&change.user_uuid) {
            continue;
        }

        let answer = noprofit
            .apply_credits(
                &state.client,
                change.account_id,
                change.user_uuid,
                change.amount,
                &change.kind,
                &change.description,
                &change.uuid.to_string(),
            )
            .await;

        match answer {
            Ok(CreditAnswer::Applied { applied, credits }) => {
                let mut transaction = state.database.write().begin().await?;
                sqlx::query(
                    "UPDATE user_noprofit_credit_changes
                    SET sent = NOW(), applied = $2, balance_after = $3, last_error = NULL
                    WHERE uuid = $1",
                )
                .bind(change.uuid)
                .bind(applied)
                .bind(credits)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "UPDATE user_noprofit_accounts SET credits = $3
                    WHERE user_uuid = $1 AND account_id = $2",
                )
                .bind(change.user_uuid)
                .bind(change.account_id)
                .bind(credits)
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await?;

                if applied != change.amount {
                    tracing::info!(change = %change.uuid, user = %change.user_uuid, amount = change.amount, applied, "noprofit.space applied part of a credit change");
                }
            }
            Ok(CreditAnswer::Refused(status)) => {
                tracing::warn!(change = %change.uuid, user = %change.user_uuid, amount = change.amount, "noprofit.space refused a credit change ({status})");

                sqlx::query(
                    "UPDATE user_noprofit_credit_changes
                    SET sent = NOW(), applied = 0, attempts = attempts + 1, last_error = $2
                    WHERE uuid = $1",
                )
                .bind(change.uuid)
                .bind(format!("refused: {status}"))
                .execute(state.database.write())
                .await?;
            }
            Ok(CreditAnswer::Failed(status)) => {
                held_back.insert(change.user_uuid);
                record_failure(state, change.uuid, &status.to_string()).await?;
            }
            Err(err) => {
                record_failure(state, change.uuid, &format!("{err:#}")).await?;

                return Err(err);
            }
        }
    }

    Ok(())
}

async fn record_failure(
    state: &crate::State,
    change_uuid: uuid::Uuid,
    error: &str,
) -> Result<(), sqlx::Error> {
    let error: String = error.chars().take(1000).collect();

    sqlx::query(
        "UPDATE user_noprofit_credit_changes
        SET attempts = attempts + 1, last_error = $2
        WHERE uuid = $1",
    )
    .bind(change_uuid)
    .bind(error)
    .execute(state.database.write())
    .await?;

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
            neo_secret: None,
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

    fn signed(signature: &'static str) -> SignedRequest<'static> {
        SignedRequest {
            method: "POST",
            path: "/api/noprofit/account/delete",
            timestamp: "1760000000",
            nonce: "abcdefghijklmnop0123456789ABCDEF",
            signature,
            body: br#"{"account_id":42}"#,
        }
    }

    /// The same vector, as noprofit.space sends it to the panel.
    #[test]
    fn verifies_requests_noprofit_space_signed() {
        let request = signed("bf6d6ccbaa9363281b768107160b56d9d764cbf1d4a24a550363de8a369af746");

        assert!(config().verify(&request, 1_760_000_000));
        assert!(config().verify(&request, 1_760_000_000 + REQUEST_TOLERANCE));
        assert!(config().verify(&request, 1_760_000_000 - REQUEST_TOLERANCE));
    }

    #[test]
    fn refuses_old_altered_or_unsigned_requests() {
        let good = "bf6d6ccbaa9363281b768107160b56d9d764cbf1d4a24a550363de8a369af746";

        // too old, or from the future
        assert!(!config().verify(&signed(good), 1_760_000_000 + REQUEST_TOLERANCE + 1));
        assert!(!config().verify(&signed(good), 1_760_000_000 - REQUEST_TOLERANCE - 1));

        // another body, path, method or nonce
        let mut request = signed(good);
        request.body = br#"{"account_id":43}"#;
        assert!(!config().verify(&request, 1_760_000_000));
        let mut request = signed(good);
        request.path = "/api/noprofit/account/summary";
        assert!(!config().verify(&request, 1_760_000_000));
        let mut request = signed(good);
        request.method = "GET";
        assert!(!config().verify(&request, 1_760_000_000));
        let mut request = signed(good);
        request.nonce = "abcdefghijklmnop0123456789ABCDEG";
        assert!(!config().verify(&request, 1_760_000_000));

        // malformed headers
        assert!(!config().verify(&signed("not hex"), 1_760_000_000));
        assert!(!config().verify(&signed(""), 1_760_000_000));
        let mut request = signed(good);
        request.nonce = "short";
        assert!(!config().verify(&request, 1_760_000_000));
        let mut request = signed(good);
        request.timestamp = "+1760000000";
        assert!(!config().verify(&request, 1_760_000_000));

        // another secret, or none
        assert!(
            !signed(good).signed_with("another-secret-0123456789abcdef0123456789", 1_760_000_000)
        );
        assert!(!signed(good).signed_with("short", 1_760_000_000));
        let mut without = config();
        without.api_secret = None;
        assert!(!without.verify(&signed(good), 1_760_000_000));
    }

    #[test]
    fn missing_accounts_are_deleted_after_the_days_set() {
        let now = chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap();

        assert_eq!(deletion_time(now, 0), None);
        assert_eq!(deletion_time(now, -3), None);
        assert_eq!(deletion_time(now, 7), Some(now + chrono::Duration::days(7)));
    }
}
