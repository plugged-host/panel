//! plugged.host: earning tokens through sponsored links (docs/features/economy.md).
//!
//! A trip starts with a claim: a row with a nonce, written before the player leaves, which binds
//! the way back to the account. The provider's page sends the player back to the Tokens page with
//! the nonce and the provider's proof; [`redeem`] checks everything local first (the claim is
//! this account's, still open and fresh, the proof was not used before, the daily cap) and asks the
//! provider last, because its anti-bypass check may use the proof up while answering. The unique
//! index on `(provider, token_hash)` is the guarantee against a proof paying twice.
//!
//! Providers: Linkvertise (a link from its dashboard, or its Full Script on the Tokens page, both
//! checked with its anti-bypass API) and Vylo (a short link made per trip, back with a proof only
//! the panel can make).

use crate::{
    economy::{Currency, change},
    models::user::User,
};
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

const LINKVERTISE_VERIFY: &str = "https://publisher.linkvertise.com/api/v1/anti_bypassing";
const VYLO_API: &str = "https://vylo.cc/api";
/// The parameter the panel's own Vylo proof comes back in.
const VYLO_PROOF: &str = "vt";
/// The parameter the return URL carries for Linkvertise's Full Script whitelist.
pub const LINKVERTISE_RETURN: &str = "lvr";
/// The parameter the claim's nonce comes back in.
pub const CLAIM_PARAM: &str = "claim";

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_GRANTED: &str = "granted";
pub const STATUS_REJECTED: &str = "rejected";

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Linkvertise,
    Vylo,
}

impl Provider {
    pub const ALL: [Self; 2] = [Self::Linkvertise, Self::Vylo];

    pub fn key(self) -> &'static str {
        match self {
            Self::Linkvertise => "linkvertise",
            Self::Vylo => "vylo",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|provider| provider.key() == key)
    }

    /// The provider's name as players see it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Linkvertise => "Linkvertise",
            Self::Vylo => "Vylo",
        }
    }

    /// The parameters the provider's proof may come back in.
    fn token_params(self) -> &'static [&'static str] {
        match self {
            Self::Linkvertise => &["hash", "token", "key", "k"],
            Self::Vylo => &[VYLO_PROOF],
        }
    }
}

/// One provider's settings, read once per request.
struct Config {
    provider: Provider,
    enabled: bool,
    reward: i64,
    daily_cap: i64,
    /// Linkvertise: the anti-bypass token. Vylo: the API key.
    secret: Option<String>,
    link: Option<String>,
    publisher: Option<i64>,
    whitelist: Vec<String>,
    min_seconds: i64,
}

impl Config {
    fn of(settings: &crate::settings::economy::AppSettingsEconomy, provider: Provider) -> Self {
        match provider {
            Provider::Linkvertise => Self {
                provider,
                enabled: settings.linkvertise_enabled,
                reward: settings.linkvertise_reward.max(1) as i64,
                daily_cap: settings.linkvertise_daily_cap.max(0) as i64,
                secret: settings.linkvertise_token.as_deref().map(str::to_string),
                link: settings.linkvertise_link.as_deref().map(str::to_string),
                publisher: settings
                    .linkvertise_publisher
                    .as_deref()
                    .and_then(|publisher| publisher.parse().ok())
                    .filter(|publisher| *publisher > 0),
                whitelist: whitelist(settings.linkvertise_whitelist.as_deref()),
                min_seconds: 0,
            },
            Provider::Vylo => Self {
                provider,
                enabled: settings.vylo_enabled,
                reward: settings.vylo_reward.max(1) as i64,
                daily_cap: settings.vylo_daily_cap.max(0) as i64,
                secret: settings.vylo_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.vylo_min_seconds.max(0) as i64,
            },
        }
    }

    fn full_script(&self) -> bool {
        self.provider == Provider::Linkvertise && self.publisher.is_some()
    }

    fn configured(&self) -> bool {
        if !self.enabled || self.secret.is_none() {
            return false;
        }

        match self.provider {
            Provider::Linkvertise => {
                self.full_script()
                    || self
                        .link
                        .as_deref()
                        .is_some_and(|link| link.to_ascii_lowercase().starts_with("https://"))
            }
            Provider::Vylo => true,
        }
    }
}

/// The providers the Tokens page offers, with the tokens a link pays and the daily cap (0: none).
/// Nothing while tokens are off.
pub fn offered(
    settings: &crate::settings::economy::AppSettingsEconomy,
) -> Vec<(Provider, i64, i64)> {
    if !settings.tokens_enabled {
        return Vec::new();
    }

    Provider::ALL
        .into_iter()
        .map(|provider| Config::of(settings, provider))
        .filter(Config::configured)
        .map(|config| (config.provider, config.reward, config.daily_cap))
        .collect()
}

/// The Full Script's whitelist: the patterns given, else the return parameter. Never empty: an
/// empty whitelist would have the script rewrite every link on the page.
pub fn whitelist(configured: Option<&str>) -> Vec<String> {
    let patterns: Vec<String> = configured
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|pattern| {
            !pattern.is_empty()
                && pattern
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._/=*-".contains(&b))
        })
        .map(str::to_string)
        .collect();

    if patterns.is_empty() {
        vec![format!("{LINKVERTISE_RETURN}=")]
    } else {
        patterns
    }
}

/// A provider's proof as it may appear in a query string: letters, digits, `-` and `_`.
pub fn extract_token(provider: Provider, params: &HashMap<String, String>) -> Option<String> {
    provider.token_params().iter().find_map(|param| {
        let value = params.get(*param)?.trim();

        (value.len() >= 8
            && value.len() <= 255
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .then(|| value.to_string())
    })
}

/// The Vylo proof of a claim: only the panel can make it, so a trip that skipped the link has
/// none.
pub fn vylo_proof(app_key: &str, nonce: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(app_key.as_bytes())
        .expect("hmac accepts keys of any length");
    mac.update(format!("vylo|{nonce}").as_bytes());

    hex::encode(mac.finalize().into_bytes())[..40].to_string()
}

/// A Linkvertise link from the dashboard with the way back filled in.
pub fn fill_link(link: &str, return_url: &str, nonce: &str) -> String {
    link.replace("{return}", &urlencoding::encode(return_url))
        .replace(
            "{return_b64}",
            &base64::engine::general_purpose::STANDARD.encode(return_url),
        )
        .replace("{nonce}", &urlencoding::encode(nonce))
}

/// Where a provider sends the player back to: the Tokens page with the claim.
pub fn return_url(app_url: &str, provider: Provider, nonce: &str) -> String {
    let base = format!(
        "{}/account/tokens?{CLAIM_PARAM}={}",
        app_url.trim_end_matches('/'),
        urlencoding::encode(nonce)
    );

    match provider {
        Provider::Linkvertise => format!("{base}&{LINKVERTISE_RETURN}=1"),
        Provider::Vylo => base,
    }
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[derive(ToSchema, Serialize)]
pub struct ApiEarnProvider {
    pub provider: Provider,
    pub reward: i64,
    /// 0: no cap.
    pub daily_cap: i64,
    pub granted_today: i64,
    /// Linkvertise's Full Script runs on the page: the link is the return URL, the script turns
    /// it into a Linkvertise link.
    pub full_script: bool,
    pub publisher: Option<i64>,
    pub whitelist: Vec<String>,
}

#[derive(ToSchema, Serialize)]
pub struct ApiTokens {
    pub enabled: bool,
    pub tokens: i64,
    pub tokens_earned: i64,
    pub claim_minutes: i64,
    pub providers: Vec<ApiEarnProvider>,
    pub transactions: Vec<crate::economy::ApiTransaction>,
}

async fn granted_today(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
    provider: Provider,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_token_claims
        WHERE user_uuid = $1 AND provider = $2 AND status = $3
            AND completed >= date_trunc('day', NOW())",
    )
    .bind(user_uuid)
    .bind(provider.key())
    .bind(STATUS_GRANTED)
    .fetch_one(database.write())
    .await
}

/// The Tokens page: the balance, the ways to earn and the history.
pub async fn tokens(state: &crate::State, user: &User) -> Result<ApiTokens, anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    let summary = crate::economy::summary(state, user).await?;

    let mut providers = Vec::new();
    if economy.tokens_enabled {
        for provider in Provider::ALL {
            let config = Config::of(&economy, provider);
            if !config.configured() {
                continue;
            }

            providers.push(ApiEarnProvider {
                provider,
                reward: config.reward,
                daily_cap: config.daily_cap,
                granted_today: granted_today(&state.database, user.uuid, provider).await?,
                full_script: config.full_script(),
                publisher: config.publisher.filter(|_| config.full_script()),
                whitelist: if config.full_script() {
                    config.whitelist.clone()
                } else {
                    Vec::new()
                },
            });
        }
    }

    Ok(ApiTokens {
        enabled: economy.tokens_enabled,
        tokens: summary.tokens,
        tokens_earned: summary.tokens_earned,
        claim_minutes: economy.claim_minutes.max(1) as i64,
        providers,
        transactions: crate::economy::transactions(
            &state.database,
            Currency::Tokens,
            user.uuid,
            crate::economy::HISTORY,
        )
        .await?,
    })
}

#[derive(Debug)]
pub enum EarnError {
    Disabled,
    NotConfigured,
    Capped,
    Throttled,
    /// The provider could not make a link (Vylo did not answer).
    ProviderFailed,
    Other(anyhow::Error),
}

impl EarnError {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "tokens are turned off".into(),
            Self::NotConfigured => "this way of earning is not set up".into(),
            Self::Capped => "you earned the most for today from this one".into(),
            Self::Throttled => "too many tries, wait a while".into(),
            Self::ProviderFailed => "the link could not be made, try again later".into(),
            Self::Other(err) => format!("{err:#}"),
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Disabled => 403,
            Self::NotConfigured => 400,
            Self::Capped => 409,
            Self::Throttled => 429,
            Self::ProviderFailed => 502,
            Self::Other(_) => 500,
        }
    }
}

impl From<anyhow::Error> for EarnError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<sqlx::Error> for EarnError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(err.into())
    }
}

#[derive(ToSchema, Serialize)]
pub struct ApiEarnStart {
    /// Where to send the player (for the Full Script: the link it turns into a Linkvertise one).
    pub url: String,
    pub nonce: String,
}

/// Opens a claim and works out where the player goes. With Linkvertise's Full Script the link has
/// to be on the page before the script runs, so the page opens its claim when it loads; an open
/// one is used again then, rather than one more per reload.
pub async fn start(
    state: &crate::State,
    user: &User,
    provider: Provider,
    ip: std::net::IpAddr,
) -> Result<ApiEarnStart, EarnError> {
    let (economy, app_url) = {
        let settings = state.settings.get().await?;
        (settings.economy.clone(), settings.app.url.clone())
    };
    if !economy.tokens_enabled {
        return Err(EarnError::Disabled);
    }

    let config = Config::of(&economy, provider);
    if !config.configured() {
        return Err(EarnError::NotConfigured);
    }
    if config.daily_cap > 0
        && granted_today(&state.database, user.uuid, provider).await? >= config.daily_cap
    {
        return Err(EarnError::Capped);
    }

    let claim_minutes = economy.claim_minutes.max(1) as i64;

    if config.full_script() {
        let open: Option<String> = sqlx::query_scalar(
            "SELECT nonce FROM user_token_claims
            WHERE user_uuid = $1 AND provider = $2 AND status = $3
                AND created >= NOW() - make_interval(mins => $4)
            ORDER BY created DESC LIMIT 1",
        )
        .bind(user.uuid)
        .bind(provider.key())
        .bind(STATUS_PENDING)
        .bind(claim_minutes as i32)
        .fetch_optional(state.database.write())
        .await?;

        if let Some(nonce) = open {
            return Ok(ApiEarnStart {
                url: return_url(&app_url, provider, &nonce),
                nonce,
            });
        }
    }

    // Tries, not grants: the cap does not stop a loop opening a thousand claims. Derived from the
    // cap, so one setting tunes both.
    let tries = (config.daily_cap * 3).max(5) as u64;
    if state
        .cache
        .ratelimit(
            format!("earn::{}", provider.key()),
            tries,
            3600,
            user.uuid.to_string(),
        )
        .await
        .is_err()
    {
        return Err(EarnError::Throttled);
    }

    let nonce = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 64);
    sqlx::query(
        "INSERT INTO user_token_claims (user_uuid, provider, nonce, status, ip)
        VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user.uuid)
    .bind(provider.key())
    .bind(&nonce)
    .bind(STATUS_PENDING)
    .bind(sqlx::types::ipnetwork::IpNetwork::from(ip))
    .execute(state.database.write())
    .await?;

    let back = return_url(&app_url, provider, &nonce);
    let url = match provider {
        Provider::Linkvertise if config.full_script() => back,
        Provider::Linkvertise => {
            fill_link(config.link.as_deref().unwrap_or_default(), &back, &nonce)
        }
        Provider::Vylo => {
            let destination = format!(
                "{back}&{VYLO_PROOF}={}",
                vylo_proof(&state.env.app_encryption_key, &nonce)
            );

            shorten_vylo(
                state,
                config.secret.as_deref().unwrap_or_default(),
                &destination,
            )
            .await
            .ok_or(EarnError::ProviderFailed)?
        }
    };

    Ok(ApiEarnStart { url, nonce })
}

async fn shorten_vylo(state: &crate::State, api_key: &str, destination: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Answer {
        status: Option<String>,
        #[serde(rename = "shortenedUrl")]
        shortened_url: Option<String>,
    }

    let response = state
        .client
        .get(VYLO_API)
        .query(&[("api", api_key), ("url", destination)])
        .timeout(std::time::Duration::from_secs(8))
        .header("Accept", "application/json")
        .send()
        .await;

    let answer = match response {
        Ok(response) if response.status().is_success() => response.json::<Answer>().await.ok(),
        Ok(response) => {
            tracing::warn!("vylo answered {} to a shorten", response.status());
            None
        }
        Err(err) => {
            tracing::warn!("vylo did not answer a shorten: {}", err.without_url());
            None
        }
    }?;

    answer
        .shortened_url
        .filter(|_| answer.status.as_deref() == Some("success"))
        .filter(|url| {
            url.to_ascii_lowercase().starts_with("https://") && !url.contains(char::is_whitespace)
        })
}

/// Asks Linkvertise whether a proof is genuine. No answer is a no.
async fn verify_linkvertise(state: &crate::State, api_token: &str, token: &str) -> bool {
    let response = state
        .client
        .post(format!(
            "{LINKVERTISE_VERIFY}?token={}",
            urlencoding::encode(api_token)
        ))
        .timeout(std::time::Duration::from_secs(8))
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "hash": token }))
        .send()
        .await;

    let response = match response {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            tracing::info!(
                "linkvertise answered {} to an anti-bypass check",
                response.status()
            );
            return false;
        }
        Err(err) => {
            tracing::warn!(
                "linkvertise did not answer an anti-bypass check: {}",
                err.without_url()
            );
            return false;
        }
    };

    let Ok(body) = response.text().await else {
        return false;
    };

    linkvertise_says_valid(&body)
}

/// Linkvertise's answer: `true`, or an object with `status`, `valid` or `success` true.
pub fn linkvertise_says_valid(body: &str) -> bool {
    if body.trim() == "true" {
        return true;
    }

    let Ok(answer) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };

    ["status", "valid", "success"].iter().any(|field| {
        matches!(answer.get(*field), Some(serde_json::Value::Bool(true)))
            || answer.get(*field).and_then(serde_json::Value::as_i64) == Some(1)
            || answer.get(*field).and_then(serde_json::Value::as_str) == Some("true")
    })
}

#[derive(ToSchema, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Redemption {
    Granted,
    /// Settled before (a reload of the return page).
    Settled,
    NoClaim,
    Expired,
    NoToken,
    Replay,
    Capped,
    Invalid,
}

#[derive(ToSchema, Serialize)]
pub struct ApiRedeem {
    pub status: Redemption,
    pub amount: i64,
    pub provider: Option<Provider>,
}

async fn reject(
    database: &crate::database::Database,
    claim_uuid: uuid::Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE user_token_claims SET status = $2, reason = $3, completed = NOW()
        WHERE uuid = $1 AND status = $4",
    )
    .bind(claim_uuid)
    .bind(STATUS_REJECTED)
    .bind(reason)
    .bind(STATUS_PENDING)
    .execute(database.write())
    .await?;

    Ok(())
}

/// Settles a returning claim: everything local first, the provider last and once.
pub async fn redeem(
    state: &crate::State,
    user: &User,
    nonce: &str,
    params: &HashMap<String, String>,
) -> Result<ApiRedeem, EarnError> {
    let economy = state.settings.get().await?.economy.clone();
    if !economy.tokens_enabled {
        return Err(EarnError::Disabled);
    }

    let answer = |status, amount, provider| ApiRedeem {
        status,
        amount,
        provider,
    };

    let Some(row) = sqlx::query(
        "SELECT uuid, provider, status, amount,
            created >= NOW() - make_interval(mins => $3) AS fresh,
            EXTRACT(EPOCH FROM NOW() - created)::BIGINT AS age
        FROM user_token_claims WHERE nonce = $1 AND user_uuid = $2",
    )
    .bind(nonce)
    .bind(user.uuid)
    .bind(economy.claim_minutes.max(1))
    .fetch_optional(state.database.write())
    .await?
    else {
        return Ok(answer(Redemption::NoClaim, 0, None));
    };

    let claim_uuid: uuid::Uuid = row.try_get("uuid")?;
    let Some(provider) = Provider::from_key(&row.try_get::<String, _>("provider")?) else {
        return Ok(answer(Redemption::Invalid, 0, None));
    };

    if row.try_get::<String, _>("status")? != STATUS_PENDING {
        return Ok(answer(
            Redemption::Settled,
            row.try_get::<i32, _>("amount")? as i64,
            Some(provider),
        ));
    }
    if !row.try_get::<bool, _>("fresh")? {
        reject(&state.database, claim_uuid, "expired").await?;
        return Ok(answer(Redemption::Expired, 0, Some(provider)));
    }

    let Some(token) = extract_token(provider, params) else {
        reject(&state.database, claim_uuid, "no_token").await?;
        return Ok(answer(Redemption::NoToken, 0, Some(provider)));
    };
    let token_hash = hash_token(&token);

    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM user_token_claims
            WHERE provider = $1 AND token_hash = $2 AND uuid <> $3)",
    )
    .bind(provider.key())
    .bind(&token_hash)
    .bind(claim_uuid)
    .fetch_one(state.database.write())
    .await?;
    if used {
        reject(&state.database, claim_uuid, "replay").await?;
        return Ok(answer(Redemption::Replay, 0, Some(provider)));
    }

    let config = Config::of(&economy, provider);
    if config.daily_cap > 0
        && granted_today(&state.database, user.uuid, provider).await? >= config.daily_cap
    {
        reject(&state.database, claim_uuid, "daily_cap").await?;
        return Ok(answer(Redemption::Capped, 0, Some(provider)));
    }

    let verified = match provider {
        Provider::Linkvertise => match config.secret.as_deref() {
            Some(api_token) => verify_linkvertise(state, api_token, &token).await,
            None => false,
        },
        Provider::Vylo => {
            let proof = vylo_proof(&state.env.app_encryption_key, nonce);
            let age: i64 = row.try_get("age")?;

            constant_time_eq(proof.as_bytes(), token.as_bytes()) && age >= config.min_seconds
        }
    };
    if !verified {
        reject(&state.database, claim_uuid, "not_verified").await?;
        return Ok(answer(Redemption::Invalid, 0, Some(provider)));
    }

    grant(
        state,
        user,
        claim_uuid,
        provider,
        &token_hash,
        config.reward,
    )
    .await
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

async fn grant(
    state: &crate::State,
    user: &User,
    claim_uuid: uuid::Uuid,
    provider: Provider,
    token_hash: &str,
    reward: i64,
) -> Result<ApiRedeem, EarnError> {
    let mut transaction = state.database.write().begin().await?;

    // Locked, so one nonce sent twice at once is paid once; the token index catches a different
    // nonce reusing one proof.
    let status: String =
        sqlx::query_scalar("SELECT status FROM user_token_claims WHERE uuid = $1 FOR UPDATE")
            .bind(claim_uuid)
            .fetch_one(&mut *transaction)
            .await?;
    if status != STATUS_PENDING {
        return Ok(ApiRedeem {
            status: Redemption::Settled,
            amount: 0,
            provider: Some(provider),
        });
    }

    let marked = sqlx::query(
        "UPDATE user_token_claims
        SET status = $2, token_hash = $3, amount = $4, completed = NOW()
        WHERE uuid = $1",
    )
    .bind(claim_uuid)
    .bind(STATUS_GRANTED)
    .bind(token_hash)
    .bind(reward as i32)
    .execute(&mut *transaction)
    .await;

    if let Err(sqlx::Error::Database(err)) = &marked
        && err.is_unique_violation()
    {
        drop(transaction);
        reject(&state.database, claim_uuid, "replay_race").await?;

        return Ok(ApiRedeem {
            status: Redemption::Replay,
            amount: 0,
            provider: Some(provider),
        });
    }
    marked?;

    change(
        &mut transaction,
        Currency::Tokens,
        user.uuid,
        reward,
        &format!("earn:{}", provider.key()),
        match provider {
            Provider::Linkvertise => "Linkvertise",
            Provider::Vylo => "Vylo",
        },
        serde_json::json!({ "claim_uuid": claim_uuid }),
    )
    .await?;
    transaction.commit().await?;

    Ok(ApiRedeem {
        status: Redemption::Granted,
        amount: reward,
        provider: Some(provider),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_set_up_providers_are_offered() {
        let mut settings = crate::settings::economy::AppSettingsEconomy::default();
        assert!(offered(&settings).is_empty());

        settings.vylo_enabled = true;
        settings.vylo_api_key = Some("<secret>".into());
        settings.vylo_reward = 2;
        settings.linkvertise_enabled = true;
        settings.linkvertise_token = Some("<secret>".into());
        settings.linkvertise_link = Some("http://link.example.com".into());
        assert_eq!(offered(&settings), vec![(Provider::Vylo, 2, 5)]);

        settings.tokens_enabled = false;
        assert!(offered(&settings).is_empty());
    }

    #[test]
    fn the_whitelist_is_never_empty() {
        assert_eq!(whitelist(None), vec!["lvr="]);
        assert_eq!(whitelist(Some(" , ")), vec!["lvr="]);
        assert_eq!(whitelist(Some("a<b, ok=")), vec!["ok="]);
        assert_eq!(
            whitelist(Some("panel.example.com/account/*, lvr=")),
            vec!["panel.example.com/account/*", "lvr="]
        );
    }

    #[test]
    fn tokens_come_from_the_provider_parameters_only() {
        let params = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>()
        };

        assert_eq!(
            extract_token(Provider::Linkvertise, &params(&[("hash", "abcdefgh12")])),
            Some("abcdefgh12".into())
        );
        assert_eq!(
            extract_token(Provider::Linkvertise, &params(&[("hash", "short")])),
            None
        );
        assert_eq!(
            extract_token(Provider::Linkvertise, &params(&[("hash", "abc def ghi")])),
            None
        );
        assert_eq!(
            extract_token(Provider::Vylo, &params(&[("hash", "abcdefgh12")])),
            None
        );
        assert_eq!(
            extract_token(Provider::Vylo, &params(&[("vt", "0123456789abcdef")])),
            Some("0123456789abcdef".into())
        );
    }

    #[test]
    fn the_vylo_proof_depends_on_key_and_nonce() {
        let proof = vylo_proof("key", "nonce");

        assert_eq!(proof.len(), 40);
        assert_eq!(proof, vylo_proof("key", "nonce"));
        assert_ne!(proof, vylo_proof("key", "other"));
        assert_ne!(proof, vylo_proof("other", "nonce"));
    }

    #[test]
    fn links_get_the_way_back() {
        let back = return_url("https://panel.example.com/", Provider::Linkvertise, "n0nce");
        assert_eq!(
            back,
            "https://panel.example.com/account/tokens?claim=n0nce&lvr=1"
        );

        let link = fill_link(
            "https://link.example.com/1?r={return_b64}&n={nonce}",
            &back,
            "n0nce",
        );
        assert!(link.starts_with("https://link.example.com/1?r=aHR0cHM6"));
        assert!(link.ends_with("&n=n0nce"));
        assert_eq!(
            fill_link(
                "https://l.example.com/?to={return}",
                "https://a.example.com/?x=1&y",
                "n"
            ),
            "https://l.example.com/?to=https%3A%2F%2Fa.example.com%2F%3Fx%3D1%26y"
        );
    }

    #[test]
    fn linkvertise_answers_are_read_strictly() {
        assert!(linkvertise_says_valid("true"));
        assert!(linkvertise_says_valid(r#"{"status":true}"#));
        assert!(linkvertise_says_valid(r#"{"valid":1}"#));
        assert!(linkvertise_says_valid(r#"{"success":"true"}"#));
        assert!(!linkvertise_says_valid("false"));
        assert!(!linkvertise_says_valid(
            r#"{"status":false,"message":"true"}"#
        ));
        assert!(!linkvertise_says_valid("<html>"));
    }

    #[test]
    fn constant_time_compare() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
