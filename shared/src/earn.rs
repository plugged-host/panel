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
//! checked with its anti-bypass API) and the shorteners (Vylo, Cuty, ShrinkEarn, ShrinkMe and
//! AdFoc.us make a short link per trip, back with a proof only the panel can make).
//!
//! CPX Research is a survey wall, not a link: no claim and no way back. Its wall opens in a new tab
//! with the account's uuid signed in it ([`cpx_research_wall`]); each survey pays what the wall
//! shows (CPX works it out in tokens from the Currency Factor set in its dashboard), told by CPX's
//! server-to-server postback ([`survey_postback`]), which is signed with the app's secure hash.
//! A transaction pays once (the same unique index); only finished surveys (`type=complete`) count
//! toward the daily cap, so a screen-out's small bonus never uses up the day; a reversal
//! (`status=2`) takes the tokens back, never below 0.

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
const CUTY_API: &str = "https://cuty.io/api";
const SHRINKEARN_API: &str = "https://shrinkearn.com/api";
const SHRINKME_API: &str = "https://shrinkme.io/api";
const ADFOCUS_API: &str = "https://adfoc.us/api/";
const CPX_RESEARCH_WALL: &str = "https://offers.cpx-research.com/index.php";
/// The most one survey postback pays, in hundredths (10 000 tokens): a Currency Factor set far too
/// high in CPX's dashboard cannot empty the economy.
const SURVEY_MOST: i64 = 1_000_000;
/// The ledger type of tokens a survey wall took back (the grant itself is `earn:<provider>`).
pub const TYPE_EARN_REVERSAL: &str = "earn_reversal";
/// The parameter the panel's own shortener proof comes back in.
const SHORTENER_PROOF: &str = "vt";
/// The parameter the return URL carries for Linkvertise's Full Script whitelist.
pub const LINKVERTISE_RETURN: &str = "lvr";
/// The parameter the claim's nonce comes back in.
pub const CLAIM_PARAM: &str = "claim";
/// Between the nonce and the proof when the proof rides in the claim (AdFoc.us).
const CLAIM_PROOF_SEPARATOR: char = '.';

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_GRANTED: &str = "granted";
pub const STATUS_REJECTED: &str = "rejected";
/// A survey CPX Research reversed after it paid (or before it was ever credited).
pub const STATUS_REVERSED: &str = "reversed";

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// A survey wall, first on the Tokens page: it pays the most.
    CpxResearch,
    Linkvertise,
    Vylo,
    Cuty,
    ShrinkEarn,
    ShrinkMe,
    #[serde(rename = "adfocus")]
    AdFocus,
}

impl Provider {
    /// In the Tokens page's order: the first row's button is the page's filled one.
    pub const ALL: [Self; 7] = [
        Self::CpxResearch,
        Self::Linkvertise,
        Self::Vylo,
        Self::Cuty,
        Self::ShrinkEarn,
        Self::ShrinkMe,
        Self::AdFocus,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::CpxResearch => "cpx_research",
            Self::Linkvertise => "linkvertise",
            Self::Vylo => "vylo",
            Self::Cuty => "cuty",
            Self::ShrinkEarn => "shrinkearn",
            Self::ShrinkMe => "shrinkme",
            Self::AdFocus => "adfocus",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|provider| provider.key() == key)
    }

    /// The provider's name as players see it.
    pub fn label(self) -> &'static str {
        match self {
            Self::CpxResearch => "CPX Research",
            Self::Linkvertise => "Linkvertise",
            Self::Vylo => "Vylo",
            Self::Cuty => "Cuty",
            Self::ShrinkEarn => "ShrinkEarn",
            Self::ShrinkMe => "ShrinkMe",
            Self::AdFocus => "AdFoc.us",
        }
    }

    /// A survey wall: pays what each survey shows, through its postback, with no claim.
    pub fn is_survey(self) -> bool {
        self == Self::CpxResearch
    }

    /// The shortener's API, if it is one.
    fn api(self) -> Option<&'static str> {
        match self {
            Self::CpxResearch | Self::Linkvertise => None,
            Self::Vylo => Some(VYLO_API),
            Self::Cuty => Some(CUTY_API),
            Self::ShrinkEarn => Some(SHRINKEARN_API),
            Self::ShrinkMe => Some(SHRINKME_API),
            Self::AdFocus => Some(ADFOCUS_API),
        }
    }

    /// The parameters the provider's proof may come back in.
    fn token_params(self) -> &'static [&'static str] {
        match self {
            Self::Linkvertise => &["hash", "token", "key", "k"],
            _ => &[SHORTENER_PROOF],
        }
    }
}

/// A setting's tokens in hundredths (`1.5` -> `150`), at least one hundredth.
pub fn hundredths(tokens: f64) -> i64 {
    if tokens.is_finite() {
        ((tokens * 100.0).round() as i64).max(1)
    } else {
        100
    }
}

/// Hundredths of a token as text without trailing zeros: `150` -> `1.5`, `200` -> `2`.
pub fn tokens_text(hundredths: i64) -> compact_str::CompactString {
    let whole = hundredths / 100;
    match hundredths % 100 {
        0 => compact_str::format_compact!("{whole}"),
        rest if rest % 10 == 0 => compact_str::format_compact!("{whole}.{}", rest / 10),
        rest => compact_str::format_compact!("{whole}.{rest:02}"),
    }
}

/// A link's reward added to what the account had saved: the whole tokens to pay now and the
/// hundredths saved for the next link (`(50, 150)` -> `(2, 0)`; `(0, 150)` -> `(1, 50)`).
pub fn split_reward(saved: i64, reward: i64) -> (i64, i64) {
    let total = saved.max(0) + reward.max(0);
    (total / 100, total % 100)
}

/// What the `nth` link of the day on one provider (0: the first) pays, in hundredths: the full
/// reward, then `decay_percent` less than the one before each time, never under `floor_percent`
/// of the full reward. Advertisers pay less for a person they already showed their ads to.
pub fn link_reward(full: i64, decay_percent: i32, floor_percent: i32, nth: i64) -> i64 {
    let full = full.max(1);
    let floor = (full * floor_percent.clamp(1, 100) as i64 + 50) / 100;
    let keep = 100 - decay_percent.clamp(0, 90) as i64;

    // in millionths, so integer steps round once at the end
    let mut scaled = full * 1_000_000;
    for _ in 0..nth.clamp(0, 1000) {
        scaled = scaled * keep / 100;
        if scaled <= floor * 1_000_000 {
            break;
        }
    }

    ((scaled + 500_000) / 1_000_000).max(floor).max(1)
}

/// One provider's settings, read once per request.
struct Config {
    provider: Provider,
    enabled: bool,
    /// Hundredths of a token per link; 0 for a survey wall, which pays what each survey shows.
    reward: i64,
    /// A survey wall: finished surveys a day, its bonuses not counted.
    daily_cap: i64,
    /// Linkvertise: the anti-bypass token. Vylo: the API key. CPX Research: the app's secure hash.
    secret: Option<String>,
    link: Option<String>,
    publisher: Option<i64>,
    whitelist: Vec<String>,
    min_seconds: i64,
    /// CPX Research's app id.
    app_id: Option<String>,
}

impl Config {
    fn of(settings: &crate::settings::economy::AppSettingsEconomy, provider: Provider) -> Self {
        match provider {
            Provider::CpxResearch => Self {
                provider,
                enabled: settings.cpx_research_enabled,
                reward: 0,
                daily_cap: settings.cpx_research_daily_cap.max(0) as i64,
                secret: settings.cpx_research_secret.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: 0,
                app_id: settings.cpx_research_app_id.as_deref().map(str::to_string),
            },
            Provider::Linkvertise => Self {
                provider,
                enabled: settings.linkvertise_enabled,
                reward: hundredths(settings.linkvertise_reward),
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
                app_id: None,
            },
            Provider::Vylo => Self {
                provider,
                enabled: settings.vylo_enabled,
                reward: hundredths(settings.vylo_reward),
                daily_cap: settings.vylo_daily_cap.max(0) as i64,
                secret: settings.vylo_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.vylo_min_seconds.max(0) as i64,
                app_id: None,
            },
            Provider::Cuty => Self {
                provider,
                enabled: settings.cuty_enabled,
                reward: hundredths(settings.cuty_reward),
                daily_cap: settings.cuty_daily_cap.max(0) as i64,
                secret: settings.cuty_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.cuty_min_seconds.max(0) as i64,
                app_id: None,
            },
            Provider::ShrinkEarn => Self {
                provider,
                enabled: settings.shrinkearn_enabled,
                reward: hundredths(settings.shrinkearn_reward),
                daily_cap: settings.shrinkearn_daily_cap.max(0) as i64,
                secret: settings.shrinkearn_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.shrinkearn_min_seconds.max(0) as i64,
                app_id: None,
            },
            Provider::ShrinkMe => Self {
                provider,
                enabled: settings.shrinkme_enabled,
                reward: hundredths(settings.shrinkme_reward),
                daily_cap: settings.shrinkme_daily_cap.max(0) as i64,
                secret: settings.shrinkme_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.shrinkme_min_seconds.max(0) as i64,
                app_id: None,
            },
            Provider::AdFocus => Self {
                provider,
                enabled: settings.adfocus_enabled,
                reward: hundredths(settings.adfocus_reward),
                daily_cap: settings.adfocus_daily_cap.max(0) as i64,
                secret: settings.adfocus_api_key.as_deref().map(str::to_string),
                link: None,
                publisher: None,
                whitelist: Vec::new(),
                min_seconds: settings.adfocus_min_seconds.max(0) as i64,
                app_id: None,
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
            Provider::CpxResearch => self.app_id.is_some(),
            Provider::Linkvertise => {
                self.full_script()
                    || self
                        .link
                        .as_deref()
                        .is_some_and(|link| link.to_ascii_lowercase().starts_with("https://"))
            }
            _ => true,
        }
    }
}

/// The providers the Tokens page offers, with the hundredths of a token a link pays (0: a survey
/// wall, which pays what each survey shows) and the daily cap (0: none).
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

/// The shortener proof of a claim: only the panel can make it, so a trip that skipped the link has
/// none. It names the provider, so a proof from one shortener cannot settle another's claim.
pub fn shortener_proof(app_key: &str, provider: Provider, nonce: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(app_key.as_bytes())
        .expect("hmac accepts keys of any length");
    mac.update(format!("{}|{nonce}", provider.key()).as_bytes());

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
        _ => base,
    }
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[derive(ToSchema, Serialize)]
pub struct ApiEarnProvider {
    pub provider: Provider,
    /// Tokens the first link of the day pays, up to 2 decimals (0 for a survey wall).
    pub reward: f64,
    /// Tokens the next link pays: less with each link already done today (`link_reward`).
    pub next_reward: f64,
    /// 0: no cap. A survey wall: finished surveys a day.
    pub daily_cap: i64,
    pub granted_today: i64,
    /// A survey wall (CPX Research): each survey pays what the wall shows, once the provider's
    /// postback confirms it; the link opens in a new tab.
    pub survey: bool,
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
    /// The part of a token saved toward the next one (fractional link rewards), 0 to 0.99.
    pub token_fraction: f64,
    pub claim_minutes: i64,
    pub providers: Vec<ApiEarnProvider>,
    pub transactions: Vec<crate::economy::ApiTransaction>,
}

/// What today's cap counts: granted claims since midnight UTC; a survey wall's finished surveys
/// only (their `reason` is the event, [`SurveyEvent::key`]), so its bonuses never use up the day.
async fn granted_today(
    executor: impl sqlx::PgExecutor<'_>,
    user_uuid: uuid::Uuid,
    provider: Provider,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_token_claims
        WHERE user_uuid = $1 AND provider = $2 AND status = $3
            AND completed >= date_trunc('day', NOW())
            AND ($4::TEXT IS NULL OR reason = $4)",
    )
    .bind(user_uuid)
    .bind(provider.key())
    .bind(STATUS_GRANTED)
    .bind(provider.is_survey().then_some(SurveyEvent::Complete.key()))
    .fetch_one(executor)
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

            let today = granted_today(state.database.write(), user.uuid, provider).await?;
            providers.push(ApiEarnProvider {
                provider,
                reward: config.reward as f64 / 100.0,
                next_reward: if provider.is_survey() {
                    0.0
                } else {
                    link_reward(
                        config.reward,
                        economy.link_decay_percent,
                        economy.link_floor_percent,
                        today,
                    ) as f64
                        / 100.0
                },
                daily_cap: config.daily_cap,
                granted_today: today,
                survey: provider.is_survey(),
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
        token_fraction: sqlx::query_scalar::<_, i32>(
            "SELECT token_fraction FROM users WHERE uuid = $1",
        )
        .bind(user.uuid)
        .fetch_optional(state.database.read())
        .await?
        .unwrap_or(0) as f64
            / 100.0,
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
        && granted_today(state.database.write(), user.uuid, provider).await? >= config.daily_cap
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

    // a survey wall opens no claim: its postback names the account
    if provider.is_survey() {
        return Ok(ApiEarnStart {
            url: cpx_research_wall(
                config.app_id.as_deref().unwrap_or_default(),
                user.uuid,
                config.secret.as_deref().unwrap_or_default(),
            ),
            nonce: String::new(),
        });
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
        // answered above, without a claim
        Provider::CpxResearch => return Err(EarnError::NotConfigured),
        Provider::Linkvertise if config.full_script() => back,
        Provider::Linkvertise => {
            fill_link(config.link.as_deref().unwrap_or_default(), &back, &nonce)
        }
        Provider::Vylo
        | Provider::Cuty
        | Provider::ShrinkEarn
        | Provider::ShrinkMe
        | Provider::AdFocus => {
            let proof = shortener_proof(&state.env.app_encryption_key, provider, &nonce);
            let api = provider.api().unwrap_or_default();
            let api_key = config.secret.as_deref().unwrap_or_default();

            match provider {
                Provider::AdFocus => {
                    shorten_adfocus(state, api, api_key, &adfocus_destination(&back, &proof)).await
                }
                _ => {
                    let destination = format!("{back}&{SHORTENER_PROOF}={proof}");
                    shorten_shortener(state, api, api_key, &destination).await
                }
            }
            .ok_or(EarnError::ProviderFailed)?
        }
    };

    Ok(ApiEarnStart { url, nonce })
}

async fn shorten_shortener(
    state: &crate::State,
    api: &str,
    api_key: &str,
    destination: &str,
) -> Option<String> {
    #[derive(Deserialize)]
    struct Answer {
        status: Option<String>,
        #[serde(rename = "shortenedUrl")]
        shortened_url: Option<String>,
    }

    let response = state
        .client
        .get(api)
        .query(&[("api", api_key), ("url", destination)])
        .timeout(std::time::Duration::from_secs(8))
        .header("Accept", "application/json")
        .send()
        .await;

    let answer = match response {
        Ok(response) if response.status().is_success() => response.json::<Answer>().await.ok(),
        Ok(response) => {
            tracing::warn!("a shortener answered {} to a shorten", response.status());
            None
        }
        Err(err) => {
            tracing::warn!(
                "a shortener did not answer a shorten: {}",
                err.without_url()
            );
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

/// AdFoc.us's way back: it refuses an encoded `url`, and a raw one may lose what follows an `&`,
/// so the proof rides in the claim (`?claim=<nonce>.<proof>`, see [`split_claim`]).
fn adfocus_destination(back: &str, proof: &str) -> String {
    format!("{back}{CLAIM_PROOF_SEPARATOR}{proof}")
}

/// A claim as it comes back: the nonce, and the proof when it rides along (AdFoc.us).
pub fn split_claim(claim: &str) -> (&str, Option<&str>) {
    match claim.split_once(CLAIM_PROOF_SEPARATOR) {
        Some((nonce, proof)) => (nonce, Some(proof)),
        None => (claim, None),
    }
}

/// AdFoc.us's API (`?key=<key>&url=<destination>`, the destination raw and last) answers the short
/// link as plain text, or `0`.
async fn shorten_adfocus(
    state: &crate::State,
    api: &str,
    api_key: &str,
    destination: &str,
) -> Option<String> {
    // raw means it must need no encoding: the panel's address, the path and the claim
    if !destination.bytes().all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b':' | b'/' | b'.' | b'-' | b'_' | b'?' | b'=')
    }) {
        tracing::warn!("an adfoc.us destination needs encoding, which adfoc.us refuses");
        return None;
    }

    let response = state
        .client
        .get(format!(
            "{api}?key={}&url={destination}",
            urlencoding::encode(api_key)
        ))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {
            adfocus_link(&response.text().await.ok()?)
        }
        Ok(response) => {
            tracing::warn!("adfoc.us answered {} to a shorten", response.status());
            None
        }
        Err(err) => {
            tracing::warn!("adfoc.us did not answer a shorten: {}", err.without_url());
            None
        }
    }
}

/// The short link in an AdFoc.us answer, on https (its http links redirect there), or `None` for
/// `0` or anything that is not an AdFoc.us link.
fn adfocus_link(answer: &str) -> Option<String> {
    let answer = answer.trim();
    let path = answer
        .strip_prefix("https://adfoc.us/")
        .or_else(|| answer.strip_prefix("http://adfoc.us/"))?;

    (!path.is_empty() && path.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| format!("https://adfoc.us/{path}"))
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

    // a proof that rode in the claim counts as the shortener's parameter, unless that came too
    let (nonce, carried) = split_claim(nonce);
    let with_proof: HashMap<String, String>;
    let params = match carried {
        Some(proof) if !params.contains_key(SHORTENER_PROOF) => {
            with_proof = params
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .chain([(SHORTENER_PROOF.to_string(), proof.to_string())])
                .collect();
            &with_proof
        }
        _ => params,
    };

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
    let today = granted_today(state.database.write(), user.uuid, provider).await?;
    if config.daily_cap > 0 && today >= config.daily_cap {
        reject(&state.database, claim_uuid, "daily_cap").await?;
        return Ok(answer(Redemption::Capped, 0, Some(provider)));
    }

    let verified = match provider {
        // a survey has no way back: its postback pays it (survey_postback)
        Provider::CpxResearch => false,
        Provider::Linkvertise => match config.secret.as_deref() {
            Some(api_token) => verify_linkvertise(state, api_token, &token).await,
            None => false,
        },
        Provider::Vylo
        | Provider::Cuty
        | Provider::ShrinkEarn
        | Provider::ShrinkMe
        | Provider::AdFocus => {
            let proof = shortener_proof(&state.env.app_encryption_key, provider, nonce);
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
        link_reward(
            config.reward,
            economy.link_decay_percent,
            economy.link_floor_percent,
            today,
        ),
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

    // a fractional reward (1.5 per link): the whole tokens now, the rest saved on the account
    let saved: i32 =
        sqlx::query_scalar("SELECT token_fraction FROM users WHERE uuid = $1 FOR UPDATE")
            .bind(user.uuid)
            .fetch_one(&mut *transaction)
            .await?;
    let (whole, rest) = split_reward(saved as i64, reward);
    sqlx::query("UPDATE users SET token_fraction = $2 WHERE uuid = $1")
        .bind(user.uuid)
        .bind(rest as i32)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE user_token_claims SET amount = $2 WHERE uuid = $1")
        .bind(claim_uuid)
        .bind(whole as i32)
        .execute(&mut *transaction)
        .await?;
    if whole == 0 {
        transaction.commit().await?;

        return Ok(ApiRedeem {
            status: Redemption::Granted,
            amount: 0,
            provider: Some(provider),
        });
    }

    change(
        &mut transaction,
        Currency::Tokens,
        user.uuid,
        whole,
        &format!("earn:{}", provider.key()),
        provider.label(),
        serde_json::json!({ "claim_uuid": claim_uuid }),
    )
    .await?;
    transaction.commit().await?;

    Ok(ApiRedeem {
        status: Redemption::Granted,
        amount: whole,
        provider: Some(provider),
    })
}

/// CPX Research's hash: md5 of `<value>-<the app's secure hash>`, in lowercase hex. The wall's link
/// signs the account (`ext_user_id`), a postback its transaction (`trans_id`).
pub fn cpx_research_hash(value: &str, secret: &str) -> String {
    format!("{:x}", md5::compute(format!("{value}-{secret}")))
}

/// CPX Research's survey wall for an account: its uuid is the `ext_user_id` the postbacks name,
/// signed, so the wall cannot be opened in another account's name. No username or email is sent.
pub fn cpx_research_wall(app_id: &str, user_uuid: uuid::Uuid, secret: &str) -> String {
    let user = user_uuid.to_string();

    format!(
        "{CPX_RESEARCH_WALL}?app_id={}&ext_user_id={user}&secure_hash={}&username=&email=&subid_1=&subid_2=",
        urlencoding::encode(app_id),
        cpx_research_hash(&user, secret)
    )
}

/// A CPX Research postback, by the names the postback URL gives CPX's placeholders
/// (docs/features/economy.md).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CpxPostback {
    /// `1`: credited, `2`: reversed (fraud found later, say).
    pub status: String,
    pub trans_id: String,
    /// The wall's `ext_user_id`: the account's uuid.
    pub user_id: String,
    /// What it pays, in tokens, as a decimal.
    pub amount_local: String,
    /// CPX's `type`: `complete`, `out` or `bonus`.
    pub kind: String,
    /// The user's address when they took the survey.
    pub ip_click: String,
    /// md5 of `<trans_id>-<the app's secure hash>`.
    pub hash: String,
}

impl CpxPostback {
    /// From the query string. `secure_hash` and `ext_user_id` are read too, for a postback URL
    /// written by hand.
    pub fn from_params(params: &HashMap<String, String>) -> Self {
        let get = |names: &[&str]| {
            names
                .iter()
                .find_map(|name| params.get(*name))
                .map(|value| value.trim().to_string())
                .unwrap_or_default()
        };

        Self {
            status: get(&["status"]),
            trans_id: get(&["trans_id"]),
            user_id: get(&["user_id", "ext_user_id"]),
            amount_local: get(&["amount_local"]),
            kind: get(&["type"]),
            ip_click: get(&["ip_click"]),
            hash: get(&["hash", "secure_hash"]),
        }
    }

    /// Whether CPX Research sent it: only it can hash the transaction id with the app's secure
    /// hash. Nothing else is signed, which is why a transaction id pays once.
    pub fn signed(&self, secret: &str) -> bool {
        !self.trans_id.is_empty()
            && !secret.is_empty()
            && constant_time_eq(
                cpx_research_hash(&self.trans_id, secret).as_bytes(),
                self.hash.to_ascii_lowercase().as_bytes(),
            )
    }
}

/// What a survey postback pays for (CPX's `type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurveyEvent {
    /// A finished survey (`complete`): the only kind the daily cap counts.
    Complete,
    /// The small bonus for a qualified screen-out (`out`).
    ScreenOut,
    /// Another small bonus (`bonus`, for rating a survey, say).
    Bonus,
}

impl SurveyEvent {
    /// No type, or one CPX does not document, counts as a finished survey, so the cap holds
    /// whatever the postback URL sends.
    pub fn from_type(kind: &str) -> Self {
        match kind.trim().to_ascii_lowercase().as_str() {
            "out" => Self::ScreenOut,
            "bonus" => Self::Bonus,
            _ => Self::Complete,
        }
    }

    /// Kept as the claim's `reason`, which the daily cap counts by.
    pub fn key(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::ScreenOut => "out",
            Self::Bonus => "bonus",
        }
    }

    pub fn counts_toward_cap(self) -> bool {
        self == Self::Complete
    }
}

/// A postback's `amount_local` (tokens as a decimal, `12.5`) in hundredths of a token, rounded
/// down, at most [`SURVEY_MOST`]; 0 for anything that is not a plain positive decimal.
pub fn survey_amount(raw: &str) -> i64 {
    let raw = raw.trim();
    let (whole, fraction) = raw.split_once(['.', ',']).unwrap_or((raw, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return 0;
    }

    let whole = whole.trim_start_matches('0');
    if whole.len() > 12 {
        return SURVEY_MOST;
    }
    let whole: i64 = whole.parse().unwrap_or(0);
    let hundredths = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(2)
        .fold(0, |sum, digit| sum * 10 + i64::from(digit - b'0'));

    (whole * 100 + hundredths).min(SURVEY_MOST)
}

/// What the panel made of a survey postback. Everything but `Refused` is answered as handled, so
/// CPX does not send it again.
#[derive(Debug, PartialEq, Eq)]
pub enum PostbackOutcome {
    /// The survey wall is off or not set up, or the hash does not match.
    Refused,
    /// A status the panel does not know, or a transaction id far too long.
    Ignored,
    /// No account has the `user_id`.
    UnknownUser,
    /// The transaction was handled before.
    Duplicate,
    /// Paid: the whole tokens now, the rest saved on the account.
    Granted {
        user_uuid: uuid::Uuid,
        amount: i64,
        event: SurveyEvent,
    },
    /// A finished survey over the day's cap: kept, not paid.
    Capped { user_uuid: uuid::Uuid },
    /// It pays nothing (`amount_local` 0 or not a number): kept.
    NoAmount { user_uuid: uuid::Uuid },
    /// Reversed: `amount` tokens taken back, never more than the balance.
    Reversed { user_uuid: uuid::Uuid, amount: i64 },
    /// A reversal of a transaction that paid nothing or was never credited.
    NothingToReverse,
}

/// CPX Research's postback (`GET /api/earn/cpx-research`): pays a survey or takes one back.
pub async fn survey_postback(
    state: &crate::State,
    postback: &CpxPostback,
) -> Result<PostbackOutcome, anyhow::Error> {
    let economy = state.settings.get().await?.economy.clone();
    let config = Config::of(&economy, Provider::CpxResearch);
    if !economy.tokens_enabled || !config.configured() {
        tracing::warn!("a cpx research postback came while cpx research is off or not set up");
        return Ok(PostbackOutcome::Refused);
    }
    if !postback.signed(config.secret.as_deref().unwrap_or_default()) {
        tracing::warn!("a cpx research postback with a wrong hash");
        return Ok(PostbackOutcome::Refused);
    }
    if postback.trans_id.len() > 255 {
        tracing::warn!("a cpx research postback with a transaction id over 255 characters");
        return Ok(PostbackOutcome::Ignored);
    }

    let token_hash = hash_token(&postback.trans_id);
    let user_uuid = uuid::Uuid::parse_str(&postback.user_id).ok();

    match postback.status.as_str() {
        "1" => match user_uuid {
            Some(user_uuid) => {
                credit_survey(state, &config, user_uuid, postback, &token_hash).await
            }
            None => Ok(PostbackOutcome::UnknownUser),
        },
        "2" => reverse_survey(state, user_uuid, postback, &token_hash).await,
        status => {
            tracing::info!(
                trans_id = %postback.trans_id,
                "a cpx research postback with status {status:?}, ignored"
            );
            Ok(PostbackOutcome::Ignored)
        }
    }
}

/// `status=1`: counted and paid with the account locked, so two postbacks at once cannot both
/// take the day's last survey.
async fn credit_survey(
    state: &crate::State,
    config: &Config,
    user_uuid: uuid::Uuid,
    postback: &CpxPostback,
    token_hash: &str,
) -> Result<PostbackOutcome, anyhow::Error> {
    let provider = Provider::CpxResearch;
    let event = SurveyEvent::from_type(&postback.kind);
    let amount = survey_amount(&postback.amount_local);
    if amount >= SURVEY_MOST {
        tracing::warn!(
            trans_id = %postback.trans_id,
            "a cpx research survey pays {:?} tokens: paid {} at most, check the currency factor",
            postback.amount_local,
            tokens_text(SURVEY_MOST)
        );
    }

    let mut transaction = state.database.write().begin().await?;

    let saved: Option<i32> =
        sqlx::query_scalar("SELECT token_fraction FROM users WHERE uuid = $1 FOR UPDATE")
            .bind(user_uuid)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(saved) = saved else {
        return Ok(PostbackOutcome::UnknownUser);
    };

    let (status, reason) = if amount <= 0 {
        (STATUS_REJECTED, "no_amount")
    } else if event.counts_toward_cap()
        && config.daily_cap > 0
        && granted_today(&mut *transaction, user_uuid, provider).await? >= config.daily_cap
    {
        (STATUS_REJECTED, "daily_cap")
    } else {
        (STATUS_GRANTED, event.key())
    };

    // the unique index on (provider, token_hash): a repeated postback inserts nothing
    let nonce = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 64);
    let claim_uuid: Option<uuid::Uuid> = sqlx::query_scalar(
        "INSERT INTO user_token_claims
            (user_uuid, provider, nonce, token_hash, status, reason, ip, completed)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT DO NOTHING
        RETURNING uuid",
    )
    .bind(user_uuid)
    .bind(provider.key())
    .bind(&nonce)
    .bind(token_hash)
    .bind(status)
    .bind(reason)
    .bind(
        postback
            .ip_click
            .parse::<std::net::IpAddr>()
            .ok()
            .map(sqlx::types::ipnetwork::IpNetwork::from),
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(claim_uuid) = claim_uuid else {
        return Ok(PostbackOutcome::Duplicate);
    };

    if status != STATUS_GRANTED {
        transaction.commit().await?;

        return Ok(if reason == "daily_cap" {
            PostbackOutcome::Capped { user_uuid }
        } else {
            PostbackOutcome::NoAmount { user_uuid }
        });
    }

    // the same as a link: the whole tokens now, the rest saved on the account
    let (whole, rest) = split_reward(saved as i64, amount);
    sqlx::query("UPDATE users SET token_fraction = $2 WHERE uuid = $1")
        .bind(user_uuid)
        .bind(rest as i32)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE user_token_claims SET amount = $2 WHERE uuid = $1")
        .bind(claim_uuid)
        .bind(whole as i32)
        .execute(&mut *transaction)
        .await?;
    if whole > 0 {
        change(
            &mut transaction,
            Currency::Tokens,
            user_uuid,
            whole,
            &format!("earn:{}", provider.key()),
            provider.label(),
            serde_json::json!({
                "claim_uuid": claim_uuid,
                "trans_id": postback.trans_id,
                "type": event.key(),
            }),
        )
        .await?;
    }
    transaction.commit().await?;

    Ok(PostbackOutcome::Granted {
        user_uuid,
        amount: whole,
        event,
    })
}

/// `status=2`: takes back the whole tokens the transaction paid, never below 0 (what was spent
/// stays spent); the part of a token it added to the saved part stays. A transaction not seen yet
/// is kept as reversed, so a credit for it coming late pays nothing.
async fn reverse_survey(
    state: &crate::State,
    user_uuid: Option<uuid::Uuid>,
    postback: &CpxPostback,
    token_hash: &str,
) -> Result<PostbackOutcome, anyhow::Error> {
    let provider = Provider::CpxResearch;
    let mut transaction = state.database.write().begin().await?;

    let claim = sqlx::query(
        "SELECT uuid, user_uuid, status, amount FROM user_token_claims
        WHERE provider = $1 AND token_hash = $2 FOR UPDATE",
    )
    .bind(provider.key())
    .bind(token_hash)
    .fetch_optional(&mut *transaction)
    .await?;

    let Some(claim) = claim else {
        let owner: Option<uuid::Uuid> = match user_uuid {
            Some(user_uuid) => {
                sqlx::query_scalar("SELECT uuid FROM users WHERE uuid = $1")
                    .bind(user_uuid)
                    .fetch_optional(&mut *transaction)
                    .await?
            }
            None => None,
        };

        if let Some(owner) = owner {
            let nonce = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 64);
            let kept = sqlx::query(
                "INSERT INTO user_token_claims
                    (user_uuid, provider, nonce, token_hash, status, reason, completed)
                VALUES ($1, $2, $3, $4, $5, 'before_credit', NOW())
                ON CONFLICT DO NOTHING",
            )
            .bind(owner)
            .bind(provider.key())
            .bind(&nonce)
            .bind(token_hash)
            .bind(STATUS_REVERSED)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            // its credit came in the meantime: an error, so CPX sends the reversal again
            if kept == 0 {
                anyhow::bail!("a cpx research transaction was credited while it was reversed");
            }
            transaction.commit().await?;
        }

        tracing::info!(
            trans_id = %postback.trans_id,
            "a cpx research reversal of a transaction never credited"
        );
        return Ok(PostbackOutcome::NothingToReverse);
    };

    let claim_uuid: uuid::Uuid = claim.try_get("uuid")?;
    let owner: uuid::Uuid = claim.try_get("user_uuid")?;
    if claim.try_get::<String, _>("status")? != STATUS_GRANTED {
        return Ok(PostbackOutcome::NothingToReverse);
    }
    let paid = claim.try_get::<i32, _>("amount")? as i64;

    sqlx::query("UPDATE user_token_claims SET status = $2 WHERE uuid = $1")
        .bind(claim_uuid)
        .bind(STATUS_REVERSED)
        .execute(&mut *transaction)
        .await?;

    let balance: i64 = sqlx::query_scalar("SELECT tokens FROM users WHERE uuid = $1 FOR UPDATE")
        .bind(owner)
        .fetch_optional(&mut *transaction)
        .await?
        .unwrap_or(0);
    let taken = survey_taken_back(paid, balance);

    if paid > 0 {
        // it was never earned after all
        sqlx::query(
            "UPDATE users SET tokens_earned = GREATEST(tokens_earned - $2, 0) WHERE uuid = $1",
        )
        .bind(owner)
        .bind(paid)
        .execute(&mut *transaction)
        .await?;
    }
    if taken > 0 {
        change(
            &mut transaction,
            Currency::Tokens,
            owner,
            -taken,
            TYPE_EARN_REVERSAL,
            provider.label(),
            serde_json::json!({
                "claim_uuid": claim_uuid,
                "trans_id": postback.trans_id,
                "paid": paid,
            }),
        )
        .await?;
    }
    transaction.commit().await?;

    Ok(PostbackOutcome::Reversed {
        user_uuid: owner,
        amount: taken,
    })
}

/// What a reversal takes back: what the survey paid, no more than the balance (never below 0).
pub fn survey_taken_back(paid: i64, balance: i64) -> i64 {
    paid.min(balance).max(0)
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
        settings.vylo_reward = 2.0;
        settings.cuty_enabled = true;
        settings.cuty_api_key = Some("<secret>".into());
        settings.cuty_reward = 3.0;
        settings.shrinkearn_enabled = true;
        settings.shrinkearn_api_key = Some("<secret>".into());
        settings.shrinkearn_reward = 4.0;
        settings.shrinkme_enabled = true;
        settings.shrinkme_api_key = Some("<secret>".into());
        settings.shrinkme_reward = 5.0;
        settings.adfocus_enabled = true;
        settings.adfocus_api_key = Some("<secret>".into());
        settings.adfocus_reward = 6.0;
        settings.linkvertise_enabled = true;
        settings.linkvertise_token = Some("<secret>".into());
        settings.linkvertise_link = Some("http://link.example.com".into());
        // CPX Research needs the app id too
        settings.cpx_research_enabled = true;
        settings.cpx_research_secret = Some("<secret>".into());
        assert_eq!(offered(&settings)[0].0, Provider::Vylo);

        settings.cpx_research_app_id = Some("12345".into());
        assert_eq!(
            offered(&settings),
            vec![
                (Provider::CpxResearch, 0, 1),
                (Provider::Vylo, 200, 5),
                (Provider::Cuty, 300, 5),
                (Provider::ShrinkEarn, 400, 5),
                (Provider::ShrinkMe, 500, 5),
                (Provider::AdFocus, 600, 5),
            ]
        );

        settings.tokens_enabled = false;
        assert!(offered(&settings).is_empty());
    }

    #[test]
    fn fractional_rewards_pay_whole_tokens_and_save_the_rest() {
        assert_eq!(hundredths(1.5), 150);
        assert_eq!(hundredths(1.0), 100);
        assert_eq!(hundredths(0.004), 1);
        assert_eq!(tokens_text(150), "1.5");
        assert_eq!(tokens_text(200), "2");
        assert_eq!(tokens_text(125), "1.25");
        assert_eq!(tokens_text(5), "0.05");

        // 1.5 per link: 1, 2, 1, 2 ...
        let mut saved = 0;
        let mut paid = Vec::new();
        for _ in 0..4 {
            let (whole, rest) = split_reward(saved, 150);
            paid.push(whole);
            saved = rest;
        }
        assert_eq!(paid, [1, 2, 1, 2]);
        assert_eq!(saved, 0);
        assert_eq!(split_reward(0, 25), (0, 25));
        assert_eq!(split_reward(75, 25), (1, 0));
    }

    #[test]
    fn each_further_link_of_the_day_pays_less() {
        // 1.5 tokens, 15% less each time, never under 25%
        let paid: Vec<i64> = (0..12).map(|nth| link_reward(150, 15, 25, nth)).collect();
        assert_eq!(paid, [150, 128, 108, 92, 78, 67, 57, 48, 41, 38, 38, 38]);
        assert_eq!(link_reward(150, 0, 25, 9), 150);
        assert_eq!(link_reward(100, 50, 100, 3), 100);
        assert_eq!(link_reward(1, 90, 1, 20), 1);
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
        for provider in [
            Provider::Cuty,
            Provider::ShrinkEarn,
            Provider::ShrinkMe,
            Provider::AdFocus,
        ] {
            assert_eq!(
                extract_token(provider, &params(&[("vt", "0123456789abcdef")])),
                Some("0123456789abcdef".into())
            );
        }
    }

    #[test]
    fn the_shortener_proof_depends_on_key_provider_and_nonce() {
        let proof = shortener_proof("key", Provider::Vylo, "nonce");

        assert_eq!(proof.len(), 40);
        assert_eq!(proof, shortener_proof("key", Provider::Vylo, "nonce"));
        assert_ne!(proof, shortener_proof("key", Provider::Vylo, "other"));
        assert_ne!(proof, shortener_proof("other", Provider::Vylo, "nonce"));
        assert_ne!(proof, shortener_proof("key", Provider::Cuty, "nonce"));
    }

    #[test]
    fn adfocus_answers_are_read_strictly() {
        assert_eq!(
            adfocus_link("http://adfoc.us/12345678\n").as_deref(),
            Some("https://adfoc.us/12345678")
        );
        assert_eq!(
            adfocus_link("https://adfoc.us/abc123").as_deref(),
            Some("https://adfoc.us/abc123")
        );
        assert_eq!(adfocus_link("0"), None);
        assert_eq!(adfocus_link("https://adfoc.us/"), None);
        assert_eq!(adfocus_link("https://adfoc.us/a?b"), None);
        assert_eq!(adfocus_link("https://adfoc.us.example.com/1"), None);
        assert_eq!(adfocus_link("<html>"), None);

        let back = return_url("https://panel.example.com", Provider::AdFocus, "n0nce");
        let destination = adfocus_destination(&back, "0123456789abcdef");
        assert_eq!(
            destination,
            "https://panel.example.com/account/tokens?claim=n0nce.0123456789abcdef"
        );
        assert!(!destination.contains('&'));
        assert_eq!(
            split_claim("n0nce.0123456789abcdef"),
            ("n0nce", Some("0123456789abcdef"))
        );
        assert_eq!(split_claim("n0nce"), ("n0nce", None));
        assert_eq!(Provider::from_key("adfocus"), Some(Provider::AdFocus));
        assert_eq!(
            serde_json::to_string(&Provider::AdFocus).unwrap(),
            "\"adfocus\""
        );
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

    fn postback(pairs: &[(&str, &str)]) -> CpxPostback {
        CpxPostback::from_params(
            &pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        )
    }

    #[test]
    fn cpx_research_is_a_survey_wall_listed_first() {
        assert_eq!(Provider::ALL[0], Provider::CpxResearch);
        assert!(Provider::CpxResearch.is_survey());
        assert!(!Provider::Vylo.is_survey());
        assert_eq!(
            Provider::from_key("cpx_research"),
            Some(Provider::CpxResearch)
        );
        assert_eq!(
            serde_json::to_string(&Provider::CpxResearch).unwrap(),
            "\"cpx_research\""
        );
        assert_eq!(Provider::CpxResearch.label(), "CPX Research");
    }

    #[test]
    fn the_survey_wall_is_signed_for_the_account() {
        // md5("<value>-<secret>"), as CPX Research's docs give it
        assert_eq!(
            cpx_research_hash("trans123", "s3cret"),
            "5a1526dcbffeb8503c67d4e60c139c61"
        );

        let user = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(
            cpx_research_wall("12345", user, "s3cret"),
            "https://offers.cpx-research.com/index.php?app_id=12345\
            &ext_user_id=123e4567-e89b-12d3-a456-426614174000\
            &secure_hash=2396b8947423b6312d9e238addcdd5d9\
            &username=&email=&subid_1=&subid_2="
        );
    }

    #[test]
    fn a_postback_is_signed_by_its_transaction() {
        let signed = postback(&[
            ("status", "1"),
            ("trans_id", "trans123"),
            ("user_id", "123e4567-e89b-12d3-a456-426614174000"),
            ("amount_local", "12.5"),
            ("type", "complete"),
            ("hash", "5a1526dcbffeb8503c67d4e60c139c61"),
        ]);
        assert_eq!(signed.kind, "complete");
        assert!(signed.signed("s3cret"));
        assert!(!signed.signed("other"));
        assert!(!signed.signed(""));

        // the hash names the transaction, nothing else: another one does not match
        let other = CpxPostback {
            trans_id: "trans124".into(),
            ..signed.clone()
        };
        assert!(!other.signed("s3cret"));

        // upper-case hex and the hand-written names are read too
        let by_hand = postback(&[
            ("trans_id", " trans123 "),
            ("ext_user_id", "123e4567-e89b-12d3-a456-426614174000"),
            ("secure_hash", "5A1526DCBFFEB8503C67D4E60C139C61"),
        ]);
        assert_eq!(by_hand.user_id, signed.user_id);
        assert!(by_hand.signed("s3cret"));

        assert!(!postback(&[("hash", "5a1526dcbffeb8503c67d4e60c139c61")]).signed("s3cret"));
    }

    #[test]
    fn survey_amounts_are_read_in_hundredths_rounded_down() {
        assert_eq!(survey_amount("12.5"), 1250);
        assert_eq!(survey_amount("12"), 1200);
        assert_eq!(survey_amount("0.05"), 5);
        assert_eq!(survey_amount("1.999"), 199);
        assert_eq!(survey_amount("1,25"), 125);
        assert_eq!(survey_amount(".5"), 50);
        assert_eq!(survey_amount(" 007.10 "), 710);
        assert_eq!(survey_amount("0"), 0);
        assert_eq!(survey_amount("0.001"), 0);
        assert_eq!(survey_amount("-3"), 0);
        assert_eq!(survey_amount(""), 0);
        assert_eq!(survey_amount("."), 0);
        assert_eq!(survey_amount("1e5"), 0);
        assert_eq!(survey_amount("1.2.3"), 0);
        assert_eq!(survey_amount("NaN"), 0);
        assert_eq!(survey_amount("123456789012345678901234567890"), SURVEY_MOST);
        assert_eq!(survey_amount("99999"), SURVEY_MOST);
        assert_eq!(survey_amount("9999.99"), 999_999);
    }

    #[test]
    fn only_finished_surveys_count_toward_the_cap() {
        assert_eq!(SurveyEvent::from_type("complete"), SurveyEvent::Complete);
        assert_eq!(SurveyEvent::from_type("OUT"), SurveyEvent::ScreenOut);
        assert_eq!(SurveyEvent::from_type("bonus"), SurveyEvent::Bonus);
        // none, or one CPX does not document: a finished survey, so the cap still holds
        assert_eq!(SurveyEvent::from_type(""), SurveyEvent::Complete);
        assert_eq!(SurveyEvent::from_type("something"), SurveyEvent::Complete);

        assert!(SurveyEvent::Complete.counts_toward_cap());
        assert!(!SurveyEvent::ScreenOut.counts_toward_cap());
        assert!(!SurveyEvent::Bonus.counts_toward_cap());
        assert_eq!(SurveyEvent::ScreenOut.key(), "out");
    }

    #[test]
    fn a_reversal_never_takes_the_balance_below_zero() {
        assert_eq!(survey_taken_back(10, 25), 10);
        assert_eq!(survey_taken_back(10, 4), 4);
        assert_eq!(survey_taken_back(10, 0), 0);
        assert_eq!(survey_taken_back(0, 25), 0);
        assert_eq!(survey_taken_back(10, -5), 0);
    }
}
