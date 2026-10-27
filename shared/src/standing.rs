//! plugged.host: an account's standing, from its suspensions (docs/features/standing.md).
//!
//! An abuse suspension of a server is a strike on its owner (a devbox's: its person,
//! [`report_server_strike`], reference `server:<uuid>`, counted once). The strikes give the
//! standing: one is a warning, then the account is limited, very limited and finally suspended,
//! and while a limit lasts (days after the latest strike) or once it is suspended, the account
//! cannot create servers ([`create_refusal`]; admins are never limited).
//!
//! - **On its own** the panel keeps the strikes (`user_strikes`, kept when the server is deleted)
//!   and counts them with the rules of Admin › Settings › Security › Abuse ([`Rules`]: 2 limit for
//!   7 days, 3 for 30 days, 4 suspend by default).
//! - **With noprofit.space** (connected, with its API secret) the strikes also go there, which
//!   counts them across all its services, and the standing of the accounts it made is its answer
//!   (with a link to the account's standing page there). The panel still writes its own record.
//!   noprofit.space not answering limits nobody (the create goes on, the failure is logged), and
//!   a strike it did not confirm is sent again by the account sync every minute
//!   ([`send_pending`]).
//!
//! Nothing here ever holds up or undoes a suspension.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How long noprofit.space's answer is kept per account.
const CACHE_SECONDS: u64 = 60;
/// How long "noprofit.space did not answer" is kept, so an outage does not slow every page down.
const UNKNOWN_SECONDS: u64 = 20;
/// How long a strike noprofit.space has not confirmed is kept to be sent again (and how far back
/// the account sync looks for abuse suspensions to send).
const PENDING_SECONDS: u64 = 7 * 24 * 60 * 60;
/// The longest reason kept with a strike (a suspension's reason is at most this long too).
const REASON_MAX: usize = 1000;

/// An account's standing: the panel's own, or noprofit.space's answer to `standing` and
/// `standing/strikes`. The same shape either way.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[schema(title = "AccountStanding")]
pub struct Standing {
    /// `good`, `warning`, `limited`, `very_limited` or `suspended`.
    pub level: compact_str::CompactString,
    /// The account's suspensions (with noprofit.space: on every service of it).
    pub suspensions: i64,
    /// Whether the account may create servers now.
    pub can_create: bool,
    /// Until when it may not; `None` while it may, or with no end (`suspended`).
    pub create_blocked_until: Option<chrono::DateTime<chrono::Utc>>,
    /// The account's standing page on noprofit.space; `None` without noprofit.space.
    pub url: Option<compact_str::CompactString>,
}

impl Standing {
    /// noprofit.space's answer, leniently: an unreadable date counts as none, a link that is not
    /// http(s) is left out. `None` without a level or `can_create`.
    pub fn from_answer(answer: &serde_json::Value) -> Option<Self> {
        let level = answer.get("level")?.as_str()?.trim();
        if level.is_empty() {
            return None;
        }

        Some(Self {
            level: level.into(),
            suspensions: answer
                .get("suspensions")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0)
                .max(0),
            can_create: answer.get("can_create")?.as_bool()?,
            create_blocked_until: answer
                .get("create_blocked_until")
                .and_then(serde_json::Value::as_str)
                .and_then(parse_time),
            url: answer
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
                .map(Into::into),
        })
    }

    /// Whether it keeps the account from creating at `now`: a block whose end has passed (an
    /// answer may be up to a minute old) no longer does.
    pub fn blocks_creating(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        !self.can_create && self.create_blocked_until.is_none_or(|until| until > now)
    }

    /// The level in English, for messages: `very_limited` reads "very limited".
    pub fn level_label(&self) -> String {
        self.level.replace('_', " ")
    }

    /// Why a create is refused, in the style of the other create refusals (lower case, no full
    /// stop): the level, until when, and where to read more.
    pub fn refusal(&self) -> String {
        let mut message = format!(
            "your account's standing is {}, so it cannot create servers",
            self.level_label()
        );
        if let Some(until) = self.create_blocked_until {
            message.push_str(&format!(" until {}", until.format("%Y-%m-%d %H:%M UTC")));
        }
        if let Some(url) = &self.url {
            message.push_str(&format!(" (see {url})"));
        }

        message
    }
}

/// The panel's own standing rules (Admin › Settings › Security › Abuse,
/// `AppSettingsAbuse::standing_rules`): from how many strikes an account is limited, very limited
/// and suspended, and for how many days after its latest strike a limit keeps it from creating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub limited_at: i64,
    pub limited_days: i64,
    pub very_limited_at: i64,
    pub very_limited_days: i64,
    pub suspended_at: i64,
}

impl Rules {
    /// Whether the thresholds never go down: limited, very limited, suspended (one may equal the
    /// next, which skips a level).
    pub fn ordered(&self) -> bool {
        self.limited_at >= 1
            && self.limited_at <= self.very_limited_at
            && self.very_limited_at <= self.suspended_at
    }

    /// The standing of an account with `strikes`, the latest at `latest`, at `now`. A limit's
    /// days count from the latest strike; once they are over the level stays, but the account
    /// may create again.
    pub fn standing(
        &self,
        strikes: i64,
        latest: Option<chrono::DateTime<chrono::Utc>>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Standing {
        let (level, days) = if strikes <= 0 {
            ("good", None)
        } else if strikes >= self.suspended_at {
            ("suspended", None)
        } else if strikes >= self.very_limited_at {
            ("very_limited", Some(self.very_limited_days))
        } else if strikes >= self.limited_at {
            ("limited", Some(self.limited_days))
        } else {
            ("warning", None)
        };

        let until = days
            .zip(latest)
            .map(|(days, latest)| latest + chrono::Duration::days(days))
            .filter(|until| *until > now);

        Standing {
            level: level.into(),
            suspensions: strikes.max(0),
            can_create: level != "suspended" && until.is_none(),
            create_blocked_until: until,
            url: None,
        }
    }
}

/// An ISO 8601 time as noprofit.space writes it (`2026-11-03T18:00:00+00:00`, `…Z`, with or
/// without fractions), or a plain `2026-11-03 18:00:00` read as UTC.
fn parse_time(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let value = value.trim();

    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|time| time.and_utc())
        })
}

/// What the cache keeps per account: the standing, or `None` when noprofit.space did not say.
#[derive(Serialize, Deserialize)]
struct Cached {
    standing: Option<Standing>,
}

fn standing_key(account_id: i64) -> String {
    format!("standing::account::{account_id}")
}

/// The connection to noprofit.space when it can be asked (connected, with the API secret).
async fn config(state: &crate::State) -> Option<crate::noprofit::Config> {
    match state.settings.get().await {
        Ok(settings) => settings
            .noprofit
            .config()
            .filter(crate::noprofit::Config::sync_enabled),
        Err(err) => {
            tracing::warn!("failed to read the settings for the account standing: {err:#}");
            None
        }
    }
}

/// noprofit.space's standing of an account, kept a minute. `None` when it does not answer
/// (logged, kept 20 seconds).
async fn of_account(
    state: &crate::State,
    noprofit: &crate::noprofit::Config,
    account_id: i64,
) -> Option<Standing> {
    let key = standing_key(account_id);

    if let Ok(Some(cached)) = state.cache.get::<Cached>(&key).await {
        return cached.standing;
    }

    let standing = match noprofit.standing(&state.client, account_id).await {
        Ok(standing) => Some(standing),
        Err(err) => {
            tracing::warn!(
                account_id,
                "noprofit.space did not say the account's standing, not limiting it: {err:#}"
            );
            None
        }
    };

    let ttl = if standing.is_some() {
        CACHE_SECONDS
    } else {
        UNKNOWN_SECONDS
    };
    let cached = Cached {
        standing: standing.clone(),
    };
    if let Err(err) = state.cache.set(&key, ttl, &cached).await {
        tracing::warn!(account_id, "failed to keep the account standing: {err:#}");
    }

    standing
}

/// The panel's own standing of a user: its strikes, counted with the rules of the settings.
/// `None` when they cannot be read (logged).
async fn local(state: &crate::State, user_uuid: uuid::Uuid) -> Option<Standing> {
    let rules = match state.settings.get().await {
        Ok(settings) => settings.abuse.standing_rules(),
        Err(err) => {
            tracing::warn!("failed to read the settings for the account standing: {err:#}");
            return None;
        }
    };

    let counted = sqlx::query_as::<_, (i64, Option<chrono::NaiveDateTime>)>(
        r#"
        SELECT COUNT(*), MAX(created) FROM user_strikes WHERE user_uuid = $1
        "#,
    )
    .bind(user_uuid)
    .fetch_one(state.database.read())
    .await;
    let (strikes, latest) = match counted {
        Ok(counted) => counted,
        Err(err) => {
            tracing::warn!(user = %user_uuid, "failed to count the user's strikes: {err:#}");
            return None;
        }
    };
    let latest = latest.map(|latest| latest.and_utc());

    Some(rules.standing(strikes, latest, chrono::Utc::now()))
}

/// A user's standing: noprofit.space's for a user it made (while it is connected, with its API
/// secret), else the panel's own. `None` when the one to ask does not answer (logged): nobody is
/// limited then.
pub async fn of_user(state: &crate::State, user_uuid: uuid::Uuid) -> Option<Standing> {
    if let Some(noprofit) = config(state).await {
        match crate::noprofit::Link::by_user_uuid(&state.database, user_uuid).await {
            Ok(Some(link)) => return of_account(state, &noprofit, link.account_id).await,
            // a user noprofit.space did not make (an emergency admin): the panel's own record
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    user = %user_uuid,
                    "failed to read the user's noprofit.space account: {err:#}"
                );
                return None;
            }
        }
    }

    local(state, user_uuid).await
}

/// The standing that keeps this user from creating a server now, if one does. Admins are never
/// limited.
pub async fn create_refusal(
    state: &crate::State,
    user: &crate::models::user::User,
) -> Option<Standing> {
    if user.admin {
        return None;
    }

    of_user(state, user.uuid)
        .await
        .filter(|standing| standing.blocks_creating(chrono::Utc::now()))
}

/// A strike noprofit.space has not confirmed yet.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PendingStrike {
    account_id: i64,
    reason: String,
}

fn server_reference(server_uuid: uuid::Uuid) -> String {
    format!("server:{server_uuid}")
}

fn strike_key(reference: &str) -> String {
    format!("standing::strike::{reference}")
}

/// Records the abuse suspension of a server as a strike on its owner (a devbox's: its person), in
/// the background: the suspension never waits for it and stands whatever happens to the strike.
/// With noprofit.space it goes there too; one it does not confirm is sent again by the account
/// sync ([`send_pending`]) while the server stays suspended, for up to a week.
pub fn report_server_strike(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    owner_uuid: uuid::Uuid,
    reason: String,
) {
    let state = state.clone();

    tokio::spawn(async move {
        if let Err(err) = report(&state, server_uuid, owner_uuid, &reason).await {
            tracing::warn!(
                server = %server_uuid,
                "failed to report an abuse suspension to noprofit.space, the account sync tries again: {err:#}"
            );
        }
    });
}

async fn report(
    state: &crate::State,
    server_uuid: uuid::Uuid,
    owner_uuid: uuid::Uuid,
    reason: &str,
) -> Result<(), anyhow::Error> {
    // a devbox's servers are the system account's: its person is the one who did it
    let owner_uuid = crate::devboxes::blocks::person_of(state, server_uuid)
        .await
        .ok()
        .flatten()
        .unwrap_or(owner_uuid);
    let reference = server_reference(server_uuid);
    let reason: String = reason.chars().take(REASON_MAX).collect();

    // the panel's own record, with noprofit.space or without
    if let Err(err) = record(&state.database, owner_uuid, &reference, &reason).await {
        tracing::warn!(server = %server_uuid, "failed to record a strike: {err:#}");
    }

    let Some(noprofit) = config(state).await else {
        return Ok(());
    };
    let Some(link) = crate::noprofit::Link::by_user_uuid(&state.database, owner_uuid).await? else {
        return Ok(());
    };

    let pending = PendingStrike {
        account_id: link.account_id,
        reason,
    };

    // kept first, so a strike lost on the way is sent again
    if let Err(err) = state
        .cache
        .set(&strike_key(&reference), PENDING_SECONDS, &pending)
        .await
    {
        tracing::warn!(
            server = %server_uuid,
            "failed to keep a strike for noprofit.space: {err:#}"
        );
    }

    send(state, &noprofit, &reference, &pending).await
}

/// A strike in the panel's own record (`user_strikes`), once per user and reference.
async fn record(
    database: &crate::database::Database,
    user_uuid: uuid::Uuid,
    reference: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO user_strikes (user_uuid, reference, reason)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_uuid, reference) DO NOTHING
        "#,
    )
    .bind(user_uuid)
    .bind(reference)
    .bind(reason)
    .execute(database.write())
    .await?;

    Ok(())
}

/// Takes the panel's own strike for a server's abuse suspension back: an admin dismissed a
/// finding on it as a false positive. noprofit.space keeps a strike it was sent.
pub async fn forgive_server(database: &crate::database::Database, server_uuid: uuid::Uuid) {
    if let Err(err) = sqlx::query("DELETE FROM user_strikes WHERE reference = $1")
        .bind(server_reference(server_uuid))
        .execute(database.write())
        .await
    {
        tracing::warn!(server = %server_uuid, "failed to take a strike back: {err:#}");
    }
}

/// Sends one strike to noprofit.space. Confirmed (new or recorded before) or refused for good,
/// it is no longer pending; anything else is an error and it stays.
async fn send(
    state: &crate::State,
    noprofit: &crate::noprofit::Config,
    reference: &str,
    pending: &PendingStrike,
) -> Result<(), anyhow::Error> {
    match noprofit
        .report_strike(
            &state.client,
            pending.account_id,
            reference,
            &pending.reason,
        )
        .await?
    {
        crate::noprofit::StrikeAnswer::Recorded(standing) => {
            tracing::info!(
                account_id = pending.account_id,
                reference,
                level = %standing.level,
                "reported a suspension to noprofit.space"
            );
            let cached = Cached {
                standing: Some(standing),
            };
            state
                .cache
                .set(&standing_key(pending.account_id), CACHE_SECONDS, &cached)
                .await
                .ok();
        }
        crate::noprofit::StrikeAnswer::Refused(status) => {
            tracing::warn!(
                account_id = pending.account_id,
                reference,
                "noprofit.space refused a strike ({status}), not sending it again"
            );
        }
    }

    state.cache.invalidate(&strike_key(reference)).await.ok();

    Ok(())
}

/// Sends the strikes noprofit.space has not confirmed yet: of the servers suspended for abuse in
/// the last week (and still suspended), those with a strike waiting. Stops at the first failure,
/// the next run carries on. Part of the account sync (`noprofit::sync_all`), every minute.
pub async fn send_pending(
    state: &crate::State,
    noprofit: &crate::noprofit::Config,
) -> Result<(), anyhow::Error> {
    let suspended: Vec<uuid::Uuid> = sqlx::query_scalar(
        r#"
        SELECT uuid FROM servers
        WHERE suspended AND suspension_kind = 'abuse'
            AND suspended_at > NOW() - make_interval(secs => $1)
        ORDER BY suspended_at
        "#,
    )
    .bind(PENDING_SECONDS as f64)
    .fetch_all(state.database.read())
    .await?;

    for server_uuid in suspended {
        let reference = server_reference(server_uuid);
        let Some(pending) = state
            .cache
            .get::<PendingStrike>(&strike_key(&reference))
            .await?
        else {
            continue;
        };

        send(state, noprofit, &reference, &pending).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    const RULES: Rules = Rules {
        limited_at: 2,
        limited_days: 7,
        very_limited_at: 3,
        very_limited_days: 30,
        suspended_at: 4,
    };

    #[test]
    fn reads_the_answer() {
        let standing = Standing::from_answer(&json!({
            "level": "very_limited",
            "suspensions": 3,
            "can_create": false,
            "create_blocked_until": "2026-11-26T18:00:00+00:00",
            "url": "https://accounts.example.com/standing",
        }))
        .unwrap();

        assert_eq!(standing.level.as_str(), "very_limited");
        assert_eq!(standing.suspensions, 3);
        assert!(!standing.can_create);
        assert_eq!(
            standing.create_blocked_until,
            Some(at("2026-11-26T18:00:00Z"))
        );
        assert_eq!(
            standing.url.as_deref(),
            Some("https://accounts.example.com/standing")
        );
        assert_eq!(standing.level_label(), "very limited");
    }

    #[test]
    fn reads_leniently() {
        let standing = Standing::from_answer(&json!({
            "level": "limited",
            "suspensions": 2,
            "can_create": false,
            "create_blocked_until": "2026-11-03 18:00:00",
            "url": "javascript:alert(1)",
        }))
        .unwrap();

        assert_eq!(
            standing.create_blocked_until,
            Some(at("2026-11-03T18:00:00Z"))
        );
        assert!(standing.url.is_none());

        let standing = Standing::from_answer(&json!({
            "level": "good",
            "can_create": true,
            "create_blocked_until": "soon",
            "url": null,
        }))
        .unwrap();
        assert_eq!(standing.suspensions, 0);
        assert_eq!(standing.create_blocked_until, None);

        for answer in [
            json!({ "level": "good" }),
            json!({ "can_create": true }),
            json!([]),
        ] {
            assert_eq!(Standing::from_answer(&answer), None);
        }
    }

    #[test]
    fn blocks_creating_until_the_end() {
        let now = at("2026-11-01T12:00:00Z");
        let mut standing = Standing {
            level: "limited".into(),
            suspensions: 2,
            can_create: false,
            create_blocked_until: Some(at("2026-11-03T18:00:00Z")),
            url: None,
        };
        assert!(standing.blocks_creating(now));

        // an answer up to a minute old whose block has ended since
        standing.create_blocked_until = Some(at("2026-11-01T11:59:30Z"));
        assert!(!standing.blocks_creating(now));

        // suspended: no end
        standing.level = "suspended".into();
        standing.create_blocked_until = None;
        assert!(standing.blocks_creating(now));

        standing.can_create = true;
        assert!(!standing.blocks_creating(now));
    }

    #[test]
    fn refusal_names_the_level_the_end_and_the_page() {
        let standing = Standing {
            level: "very_limited".into(),
            suspensions: 3,
            can_create: false,
            create_blocked_until: Some(at("2026-11-26T18:00:00Z")),
            url: Some("https://accounts.example.com/standing".into()),
        };

        assert_eq!(
            standing.refusal(),
            "your account's standing is very limited, so it cannot create servers until 2026-11-26 18:00 UTC (see https://accounts.example.com/standing)"
        );

        let standing = Standing {
            level: "suspended".into(),
            suspensions: 4,
            can_create: false,
            create_blocked_until: None,
            url: None,
        };
        assert_eq!(
            standing.refusal(),
            "your account's standing is suspended, so it cannot create servers"
        );
    }

    #[test]
    fn counts_strikes_into_levels() {
        let now = at("2026-11-10T12:00:00Z");

        let standing = RULES.standing(0, None, now);
        assert_eq!(standing.level.as_str(), "good");
        assert!(standing.can_create);

        let standing = RULES.standing(1, Some(at("2026-11-10T11:00:00Z")), now);
        assert_eq!(standing.level.as_str(), "warning");
        assert!(standing.can_create);
        assert_eq!(standing.create_blocked_until, None);

        // limited for 7 days after the latest strike
        let standing = RULES.standing(2, Some(at("2026-11-07T12:00:00Z")), now);
        assert_eq!(standing.level.as_str(), "limited");
        assert!(!standing.can_create);
        assert_eq!(
            standing.create_blocked_until,
            Some(at("2026-11-14T12:00:00Z"))
        );

        // the days are over: still limited, but creating again
        let standing = RULES.standing(2, Some(at("2026-11-01T12:00:00Z")), now);
        assert_eq!(standing.level.as_str(), "limited");
        assert!(standing.can_create);
        assert_eq!(standing.create_blocked_until, None);

        let standing = RULES.standing(3, Some(at("2026-11-01T12:00:00Z")), now);
        assert_eq!(standing.level.as_str(), "very_limited");
        assert!(!standing.can_create);
        assert_eq!(
            standing.create_blocked_until,
            Some(at("2026-12-01T12:00:00Z"))
        );

        for strikes in [4, 9] {
            let standing = RULES.standing(strikes, Some(at("2026-01-01T00:00:00Z")), now);
            assert_eq!(standing.level.as_str(), "suspended");
            assert!(!standing.can_create);
            assert_eq!(standing.create_blocked_until, None);
            assert_eq!(standing.suspensions, strikes);
        }

        assert!(RULES.standing(4, None, now).url.is_none());
    }

    #[test]
    fn rules_must_not_go_down() {
        let now = at("2026-11-10T12:00:00Z");
        assert!(RULES.ordered());

        // equal thresholds skip a level
        let skips = Rules {
            very_limited_at: 4,
            ..RULES
        };
        assert!(skips.ordered());
        assert_eq!(skips.standing(3, None, now).level.as_str(), "limited");

        let backwards = Rules {
            limited_at: 3,
            very_limited_at: 2,
            ..RULES
        };
        assert!(!backwards.ordered());

        let never_good = Rules {
            limited_at: 0,
            ..RULES
        };
        assert!(!never_good.ordered());
    }
}
