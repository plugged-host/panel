//! plugged.host: the membership bundle with another panel, both ways
//! (docs/features/economy.md#memberships-across-pluggedhost-and-switchedcloud): plugged.host's
//! Premium includes switched.cloud Plus, and switched.cloud Premium includes plugged.host Plus.
//!
//! The other panel knows its users by their noprofit.space account, so the bundle needs
//! noprofit.space. Admin › Settings › Economy › Memberships turns it on with the other panel's URL
//! and a shared secret (one secret signs both directions).
//!
//! **Receiving** ([`receive`], `POST /api/memberships/bundle`, the same wire format as below): the
//! other panel says when each account's second membership there ends, and Plus here runs until
//! then (`user_memberships.bundle_plus_until`, counted by [`crate::economy::Membership::has_plus`]).
//! A future date sets it; `null` or a past date ends a running one now (so the lapse is settled
//! and told like any other) and leaves an ended one as it was. An account with no user here yet is
//! kept in `pending_membership_bundles` and applied within a minute of its user being linked.
//!
//! **Sending**: [`sync`] (every minute) tells the other panel when the second membership of each
//! linked account here ends:
//!
//! - whenever `plusplus_until` differs from what the other panel last confirmed
//!   (`user_memberships.bundle_synced_until`), however it changed: bought, given, renewed, set or
//!   ended by an admin. An ended membership is sent as it is (its past end, or `null` once an admin
//!   cleared it), so the other panel ends its part too;
//! - once a day for every one that still runs (`bundle_synced` older than a day), so a lost
//!   answer or a reset on the other side heals by itself.
//!
//! Only an answer in the 2xx range marks a batch as delivered; anything else is sent again the
//! next minute.
//!
//! Wire format: `POST {url}/api/partner/plugged-host/membership` with
//! `{"memberships": [{"account_id": <noprofit.space account id>, "premium_until": "<RFC 3339>" | null}, ...]}`
//! (at most [`BATCH`] a request), with `X-Timestamp: <unix seconds>` and
//! `X-Signature: hex(hmac_sha256(secret, "<timestamp>.<raw body>"))`. The other panel accepts a
//! timestamp up to 300 seconds off and answers `{"applied": n, "pending": m}` (`pending`: accounts
//! that have not signed in there yet, kept until they do).

use crate::settings::economy::AppSettingsEconomy;
use chrono::SubsecRound;
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::Sha256;
use sqlx::Row;

/// The other panel's route.
pub const PATH: &str = "/api/partner/plugged-host/membership";
/// How far the other panel's clock may be off (either way), in seconds.
pub const TOLERANCE: i64 = 300;
/// Memberships one request to this panel may carry.
pub const MAX_ENTRIES: usize = 500;
/// Memberships per request.
pub const BATCH: usize = 100;
/// Memberships looked at per run; the rest wait for the next minute.
const PER_RUN: i64 = 1000;
/// Shorter secrets turn the bundle off, here and on the other panel.
pub const MIN_SECRET_LENGTH: usize = 32;

/// Whether the bundle runs: switched on with the other panel's URL and a secret of at least
/// [`MIN_SECRET_LENGTH`] characters, and noprofit.space connected (the account ids are its).
pub fn offered(economy: &AppSettingsEconomy, connected: bool) -> bool {
    connected
        && economy.bundle_enabled
        && !economy.bundle_url.trim().is_empty()
        && economy
            .bundle_secret
            .as_deref()
            .is_some_and(|secret| secret.trim().len() >= MIN_SECRET_LENGTH)
}

/// The other panel, from the settings.
struct Target {
    /// Without a trailing slash.
    url: String,
    secret: String,
}

impl Target {
    fn from_settings(economy: &AppSettingsEconomy, connected: bool) -> Option<Self> {
        if !offered(economy, connected) {
            return None;
        }

        Some(Self {
            url: economy.bundle_url.trim().trim_end_matches('/').to_string(),
            secret: economy.bundle_secret.as_deref()?.trim().to_string(),
        })
    }

    /// One signed batch; an error unless the other panel answered 2xx.
    async fn send(
        &self,
        client: &reqwest::Client,
        memberships: &[Entry],
    ) -> Result<(), anyhow::Error> {
        let body = serde_json::to_vec(&Body { memberships })?;
        let timestamp = chrono::Utc::now().timestamp();
        let signature = signature(&self.secret, timestamp, &body);

        let response = client
            .post(format!("{}{PATH}", self.url))
            .timeout(std::time::Duration::from_secs(10))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("X-Timestamp", timestamp.to_string())
            .header("X-Signature", signature)
            .body(body)
            .send()
            .await?;
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("the membership bundle's panel answered {status}");
        }

        Ok(())
    }
}

/// One account's second membership as the other panel gets it.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub account_id: i64,
    /// When the second membership ends (also once it has), RFC 3339 in UTC; `None` (`null`)
    /// when an admin cleared it.
    pub premium_until: Option<String>,
}

impl Entry {
    pub fn new(account_id: i64, until: Option<chrono::NaiveDateTime>) -> Self {
        Self {
            account_id,
            premium_until: until.map(|until| {
                until
                    .and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            }),
        }
    }
}

#[derive(Serialize)]
struct Body<'a> {
    memberships: &'a [Entry],
}

/// `hex(hmac_sha256(secret, "<timestamp>.<body>"))`.
pub fn signature(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts keys of any length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);

    hex::encode(mac.finalize().into_bytes())
}

/// Sends the other panel every second membership it has not confirmed yet (see the module docs).
/// A no-op while the bundle is off.
pub async fn sync(state: &crate::State) -> Result<(), anyhow::Error> {
    // first, the other way round: Plus that came for accounts linked since
    claim_pending(state).await?;

    let target = {
        let settings = state.settings.get().await?;
        Target::from_settings(&settings.economy, settings.noprofit.connected())
    };
    let Some(target) = target else {
        return Ok(());
    };

    // read on the primary: what was just marked as sent must not come back from a lagging replica
    let rows = sqlx::query(
        "SELECT user_memberships.user_uuid, user_noprofit_accounts.account_id,
            user_memberships.plusplus_until
        FROM user_memberships
        JOIN user_noprofit_accounts
            ON user_noprofit_accounts.user_uuid = user_memberships.user_uuid
        WHERE user_noprofit_accounts.deleted IS NULL
            AND ((user_memberships.bundle_synced IS NULL
                    AND user_memberships.plusplus_until > NOW())
                OR (user_memberships.bundle_synced IS NOT NULL
                    AND user_memberships.plusplus_until
                        IS DISTINCT FROM user_memberships.bundle_synced_until)
                OR (user_memberships.plusplus_until > NOW()
                    AND user_memberships.bundle_synced < NOW() - INTERVAL '1 day'))
        ORDER BY user_memberships.bundle_synced NULLS FIRST
        LIMIT $1",
    )
    .bind(PER_RUN)
    .fetch_all(state.database.write())
    .await?;

    let mut due = Vec::with_capacity(rows.len());
    for row in rows {
        let user_uuid: uuid::Uuid = row.try_get("user_uuid")?;
        let account_id: i64 = row.try_get("account_id")?;
        let until: Option<chrono::NaiveDateTime> = row.try_get("plusplus_until")?;
        due.push((user_uuid, account_id, until));
    }

    for chunk in due.chunks(BATCH) {
        let entries: Vec<Entry> = chunk
            .iter()
            .map(|&(_, account_id, until)| Entry::new(account_id, until))
            .collect();
        target.send(&state.client, &entries).await?;

        // the value sent, not the one now: a change made meanwhile is sent next minute
        for &(user_uuid, _, until) in chunk {
            sqlx::query(
                "UPDATE user_memberships SET bundle_synced_until = $2, bundle_synced = NOW()
                WHERE user_uuid = $1",
            )
            .bind(user_uuid)
            .bind(until)
            .execute(state.database.write())
            .await?;
        }
    }

    Ok(())
}

/// The other panel's signature checks out: `signature` is the body's, and `timestamp` a number
/// at most [`TOLERANCE`] seconds from `now`. Compared in constant time.
pub fn verify(secret: &str, timestamp: &str, signature: &str, body: &[u8], now: i64) -> bool {
    let timestamp = timestamp.trim();
    if timestamp.is_empty()
        || timestamp.len() > 12
        || !timestamp.bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let Ok(at) = timestamp.parse::<i64>() else {
        return false;
    };
    if (now - at).abs() > TOLERANCE {
        return false;
    }
    let Ok(signature) = hex::decode(signature.trim()) else {
        return false;
    };

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts keys of any length");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);

    mac.verify_slice(&signature).is_ok()
}

/// One account's second membership on the other panel, as it sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    /// The noprofit.space account (`user_noprofit_accounts.account_id`).
    pub account_id: i64,
    pub premium_until: Option<chrono::NaiveDateTime>,
}

/// The request's accounts (`{"memberships": [...]}`, or one such object), or `None` when the
/// body is not that.
pub fn parse(body: &[u8]) -> Option<Vec<Update>> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let items: Vec<&serde_json::Value> = match value.get("memberships") {
        Some(serde_json::Value::Array(list)) => list.iter().collect(),
        Some(_) => return None,
        None => vec![&value],
    };
    if items.len() > MAX_ENTRIES {
        return None;
    }

    items
        .into_iter()
        .map(|item| {
            let account_id = item.get("account_id")?.as_i64().filter(|id| *id > 0)?;
            let premium_until = match item.get("premium_until") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(text)) => {
                    Some(chrono::DateTime::parse_from_rfc3339(text).ok()?.naive_utc())
                }
                Some(_) => return None,
            };

            Some(Update {
                account_id,
                premium_until,
            })
        })
        .collect()
}

/// When the bundle's Plus ends once the other panel said `given`: a future date sets it; `None`
/// or a past date ends a running one now and leaves an ended one (or none) as it was.
pub fn next_until(
    current: Option<chrono::NaiveDateTime>,
    given: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> Option<chrono::NaiveDateTime> {
    match given {
        Some(until) if until > now => Some(until.trunc_subsecs(0)),
        _ => match current {
            Some(current) if current > now => Some(now.trunc_subsecs(0)),
            other => other,
        },
    }
}

/// What a request from the other panel came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Received {
    /// The bundle is off or not set up here (404).
    Off,
    /// A wrong signature or a timestamp too far off (401).
    Unauthorized,
    /// A body that is not the wire format (422).
    Invalid,
    /// Applied to `applied` accounts' users, `pending` kept for accounts without a user yet.
    Done { applied: usize, pending: usize },
}

/// `POST /api/memberships/bundle`: the other panel's second memberships, signed with the shared
/// secret.
pub async fn receive(
    state: &crate::State,
    timestamp: &str,
    signature: &str,
    body: &[u8],
) -> Result<Received, anyhow::Error> {
    let secret = {
        let settings = state.settings.get().await?;
        Target::from_settings(&settings.economy, settings.noprofit.connected())
            .map(|target| target.secret)
    };
    let Some(secret) = secret else {
        return Ok(Received::Off);
    };
    if !verify(
        &secret,
        timestamp,
        signature,
        body,
        chrono::Utc::now().timestamp(),
    ) {
        return Ok(Received::Unauthorized);
    }
    let Some(updates) = parse(body) else {
        return Ok(Received::Invalid);
    };

    let (mut applied, mut pending) = (0, 0);
    for update in &updates {
        if apply(state, update).await? {
            applied += 1;
        } else {
            pending += 1;
        }
    }

    Ok(Received::Done { applied, pending })
}

/// One account's update: onto its user's memberships (`true`), or kept while it runs for an
/// account no user here has yet (`false`).
async fn apply(state: &crate::State, update: &Update) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().naive_utc();
    let user_uuid: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT user_uuid FROM user_noprofit_accounts WHERE account_id = $1 AND deleted IS NULL",
    )
    .bind(update.account_id)
    .fetch_optional(state.database.write())
    .await?;

    let Some(user_uuid) = user_uuid else {
        match update.premium_until.filter(|until| *until > now) {
            Some(until) => {
                sqlx::query(
                    "INSERT INTO pending_membership_bundles (account_id, plus_until)
                    VALUES ($1, $2)
                    ON CONFLICT (account_id) DO UPDATE SET plus_until = EXCLUDED.plus_until",
                )
                .bind(update.account_id)
                .bind(until.trunc_subsecs(0))
                .execute(state.database.write())
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM pending_membership_bundles WHERE account_id = $1")
                    .bind(update.account_id)
                    .execute(state.database.write())
                    .await?;
            }
        }

        return Ok(false);
    };

    let current: Option<Option<chrono::NaiveDateTime>> =
        sqlx::query_scalar("SELECT bundle_plus_until FROM user_memberships WHERE user_uuid = $1")
            .bind(user_uuid)
            .fetch_optional(state.database.write())
            .await?;
    let current = current.flatten();
    let until = next_until(current, update.premium_until, now);

    if until != current {
        sqlx::query(
            "INSERT INTO user_memberships (user_uuid, bundle_plus_until) VALUES ($1, $2)
            ON CONFLICT (user_uuid) DO UPDATE SET bundle_plus_until = EXCLUDED.bundle_plus_until",
        )
        .bind(user_uuid)
        .bind(until)
        .execute(state.database.write())
        .await?;

        crate::economy::membership_changed(state, user_uuid).await;
    }

    // one kept from before the account was linked is no longer needed
    sqlx::query("DELETE FROM pending_membership_bundles WHERE account_id = $1")
        .bind(update.account_id)
        .execute(state.database.write())
        .await?;

    Ok(true)
}

/// Plus that came for accounts with no user here then, onto their user once one is linked (each
/// minute, with [`sync`]); ended ones are dropped. A user the other panel reached since keeps
/// its own date.
async fn claim_pending(state: &crate::State) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM pending_membership_bundles WHERE plus_until <= NOW()")
        .execute(state.database.write())
        .await?;

    let claimed: Vec<uuid::Uuid> = sqlx::query_scalar(
        "WITH claimed AS (
            DELETE FROM pending_membership_bundles
            USING user_noprofit_accounts
            WHERE user_noprofit_accounts.account_id = pending_membership_bundles.account_id
                AND user_noprofit_accounts.deleted IS NULL
            RETURNING user_noprofit_accounts.user_uuid, pending_membership_bundles.plus_until
        )
        INSERT INTO user_memberships (user_uuid, bundle_plus_until)
        SELECT user_uuid, plus_until FROM claimed
        ON CONFLICT (user_uuid) DO UPDATE SET bundle_plus_until = EXCLUDED.bundle_plus_until
            WHERE user_memberships.bundle_plus_until IS NULL
        RETURNING user_uuid",
    )
    .fetch_all(state.database.write())
    .await?;

    for user_uuid in claimed {
        crate::economy::membership_changed(state, user_uuid).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn the_body_and_its_signature_match_the_receiver() {
        // the same vector is in switched.cloud's src/plans/bundle.rs; python3:
        // hmac.new(b"s"*40, b"1800000000." + body, hashlib.sha256).hexdigest()
        let body = serde_json::to_vec(&Body {
            memberships: &[Entry::new(42, Some(at(28, 19)))],
        })
        .unwrap();

        assert_eq!(
            String::from_utf8(body.clone()).unwrap(),
            r#"{"memberships":[{"account_id":42,"premium_until":"2026-11-28T19:00:00Z"}]}"#
        );
        assert_eq!(
            signature(&"s".repeat(40), 1_800_000_000, &body),
            "2e825fa1e17d6c37372ef9bdcd58b042ddf29ff45db21782a74854651cb48f4f"
        );
    }

    #[test]
    fn a_cleared_membership_is_sent_as_null() {
        assert_eq!(
            serde_json::to_string(&Entry::new(7, None)).unwrap(),
            r#"{"account_id":7,"premium_until":null}"#
        );
    }

    #[test]
    fn the_bundle_needs_noprofit_a_url_and_a_long_secret() {
        let mut economy = AppSettingsEconomy {
            bundle_enabled: true,
            bundle_secret: Some("<secret>".into()),
            ..Default::default()
        };
        assert!(!offered(&economy, true), "a short secret turns it off");

        economy.bundle_secret = Some("s".repeat(32).into());
        assert!(offered(&economy, true));
        assert!(!offered(&economy, false), "noprofit.space is needed");

        economy.bundle_url = "".into();
        assert!(!offered(&economy, true));

        economy.bundle_url = "https://panel.example.com/".into();
        economy.bundle_enabled = false;
        assert!(!offered(&economy, true));

        economy.bundle_enabled = true;
        let target = Target::from_settings(&economy, true).unwrap();
        assert_eq!(target.url, "https://panel.example.com");
    }

    #[test]
    fn a_request_is_verified_like_it_is_signed() {
        let secret = "s".repeat(40);
        let body = br#"{"memberships":[{"account_id":42,"premium_until":"2026-11-28T19:00:00Z"}]}"#;
        let signature = "2e825fa1e17d6c37372ef9bdcd58b042ddf29ff45db21782a74854651cb48f4f";
        let now = 1_800_000_000;

        assert!(verify(
            &secret,
            "1800000000",
            signature,
            body,
            now + TOLERANCE
        ));
        assert!(verify(
            &secret,
            "1800000000",
            &signature.to_uppercase(),
            body,
            now - TOLERANCE
        ));
        assert!(!verify(
            &secret,
            "1800000000",
            signature,
            body,
            now + TOLERANCE + 1
        ));
        assert!(!verify(&secret, "1800000001", signature, body, now));
        assert!(!verify(&"t".repeat(40), "1800000000", signature, body, now));
        assert!(!verify(&secret, "", signature, body, now));
        assert!(!verify(&secret, "1800000000", "zz", body, now));

        assert_eq!(
            parse(body).unwrap(),
            vec![Update {
                account_id: 42,
                premium_until: Some(at(28, 19)),
            }]
        );
        assert_eq!(
            parse(br#"{"account_id":7,"premium_until":null}"#).unwrap(),
            vec![Update {
                account_id: 7,
                premium_until: None,
            }]
        );
        assert!(parse(br#"{"memberships":[{"account_id":0,"premium_until":null}]}"#).is_none());
        assert!(parse(br#"{"memberships":[{"account_id":7,"premium_until":"soon"}]}"#).is_none());
        assert!(parse(b"nope").is_none());
    }

    #[test]
    fn an_end_in_the_past_ends_the_bundles_plus_now() {
        let now = at(10, 0);

        assert_eq!(next_until(None, Some(at(20, 0)), now), Some(at(20, 0)));
        assert_eq!(
            next_until(Some(at(12, 0)), Some(at(20, 0)), now),
            Some(at(20, 0))
        );
        assert_eq!(next_until(Some(at(20, 0)), None, now), Some(now));
        assert_eq!(next_until(Some(at(20, 0)), Some(at(9, 0)), now), Some(now));
        assert_eq!(next_until(Some(at(5, 0)), None, now), Some(at(5, 0)));
        assert_eq!(next_until(None, None, now), None);
    }

    #[test]
    fn the_bundles_plus_counts_as_plus() {
        use crate::economy::{Membership, TIER_PLUS, TIER_PREMIUM, lost_tiers};

        let settings = AppSettingsEconomy::default();
        let now = at(10, 0);
        let bundle = Membership {
            plus_until: Some(at(5, 0)),
            bundle_plus_until: Some(at(20, 0)),
            ..Default::default()
        };

        assert!(bundle.has_plus(now) && bundle.bundle_active(now));
        assert!(bundle.unlocks(Some(TIER_PLUS), now));
        assert!(!bundle.unlocks(Some(TIER_PREMIUM), now));
        assert_eq!(bundle.priority(now), 1);
        assert_eq!(bundle.burst_cores(&settings, now), 1);
        assert_eq!(bundle.plus_runs_until(), Some(at(20, 0)));
        assert_eq!(lost_tiers(&bundle, now), vec![TIER_PREMIUM]);

        let ended = Membership {
            bundle_plus_until: Some(at(9, 0)),
            ..bundle
        };
        assert!(!ended.has_plus(now));
        assert_eq!(lost_tiers(&ended, now), vec![TIER_PREMIUM, TIER_PLUS]);
    }
}
