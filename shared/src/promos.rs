//! plugged.host: the membership popup (docs/features/promos.md).
//!
//! Accounts without a membership that see ads get, at most once per `ads::promo_hours`, a card
//! with the membership they are closest to: the dearest one their tokens already buy, else the
//! cheapest, its perks (from the settings, only what is on) and how far their tokens are. The
//! moment it was last shown is kept on the account (`users.promo_shown`), so it follows the person
//! across devices; a claim is one conditional update, so two tabs never both show it.

use crate::{
    ads::Audience,
    economy::{MembershipKind, Pay, membership_offer},
    models::user::User,
    settings::neo::NeoPlan,
};
use serde::Serialize;
use utoipa::ToSchema;

/// One thing the membership gives, as the popup lists it.
#[derive(ToSchema, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "id", rename_all = "snake_case")]
pub enum ApiPromoPerk {
    /// Its location tier opens public locations.
    Locations,
    /// The start queue lets it go first (the second membership) or ahead of accounts without one.
    Queue { first: bool },
    /// CPU above every server's limit while the node has it to spare.
    BurstCores { cores: i64 },
    /// No ads and no waiting room (the second membership while Fewer Ads for Members is on).
    NoAds,
    /// No ads on the pages, only the waiting room before a start (the first).
    FewerAds,
    /// The panel itself costs tokens; both memberships include it.
    PanelAccess,
    /// More neo usage, in % over the free limits.
    Neo { percent: i64 },
    /// Archives, unpacking and backups run on more threads.
    FasterBackups,
}

#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiPromo {
    pub membership: MembershipKind,
    pub name: compact_str::CompactString,
    /// Tokens it costs now (a membership sale applied), and without the sale.
    pub cost: i64,
    pub full_cost: i64,
    pub days: i64,
    /// The account's tokens.
    pub tokens: i64,
    pub perks: Vec<ApiPromoPerk>,
}

/// The membership to show: the dearest one `tokens` already buy, else the cheapest. `offers` are
/// the memberships sold for tokens with their cost now.
pub fn pick(offers: &[(MembershipKind, i64)], tokens: i64) -> Option<MembershipKind> {
    offers
        .iter()
        .filter(|(_, cost)| *cost <= tokens)
        .max_by_key(|(_, cost)| *cost)
        .or_else(|| offers.iter().min_by_key(|(_, cost)| *cost))
        .map(|(kind, _)| *kind)
}

/// Whether an account created at `created` is past its first day at `now`.
pub fn past_first_day(created: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> bool {
    now - created >= chrono::Duration::days(1)
}

/// What `kind` gives, from the settings; only what is on. The locations it opens are added by the
/// caller, who knows which tiers have public locations.
fn perks(settings: &crate::settings::AppSettings, kind: MembershipKind) -> Vec<ApiPromoPerk> {
    let second = kind == MembershipKind::Plusplus;
    let economy = &settings.economy;
    let mut perks = Vec::new();

    if settings.queue.enabled {
        perks.push(ApiPromoPerk::Queue { first: second });
    }
    let cores = if second {
        economy.plusplus_burst_cores
    } else {
        economy.plus_burst_cores
    }
    .max(0) as i64;
    if cores > 0 {
        perks.push(ApiPromoPerk::BurstCores { cores });
    }
    if settings.ads.enabled && settings.ads.member_free {
        perks.push(if second {
            ApiPromoPerk::NoAds
        } else {
            ApiPromoPerk::FewerAds
        });
    }
    if economy.panel_access_enabled && economy.panel_access_tokens > 0 {
        perks.push(ApiPromoPerk::PanelAccess);
    }
    if crate::neo::offered(settings) {
        let percent = settings.neo.usage_percent(if second {
            NeoPlan::Plusplus
        } else {
            NeoPlan::Plus
        }) - 100;
        if percent > 0 {
            perks.push(ApiPromoPerk::Neo { percent });
        }
    }
    perks.push(ApiPromoPerk::FasterBackups);

    perks
}

/// The popup for `user` if it is due, marking it shown; `None` when it is off, not for this
/// account, or shown less than `promo_hours` ago.
pub async fn claim(state: &crate::State, user: &User) -> Result<Option<ApiPromo>, anyhow::Error> {
    let now = chrono::Utc::now();
    if user.admin
        || user.suspended
        || user.has_plus
        || user.has_plusplus
        || !past_first_day(user.created, now.naive_utc())
    {
        return Ok(None);
    }

    let (hours, offers) = {
        let settings = state.settings.get().await?;
        let economy = &settings.economy;
        if !settings.ads.promo_enabled || !economy.membership_enabled || !economy.tokens_enabled {
            return Ok(None);
        }
        let percent = economy.membership_discount_at(now);
        // each membership sold for tokens: its offer, name and perks
        let offers: Vec<(
            MembershipKind,
            crate::economy::MembershipOffer,
            compact_str::CompactString,
            Vec<ApiPromoPerk>,
        )> = [MembershipKind::Plus, MembershipKind::Plusplus]
            .into_iter()
            .filter_map(|kind| {
                membership_offer(economy, kind, Pay::Tokens, percent)
                    .filter(|offer| offer.cost > 0)
                    .map(|offer| {
                        (
                            kind,
                            offer,
                            kind.name(economy).into(),
                            perks(&settings, kind),
                        )
                    })
            })
            .collect();

        (settings.ads.promo_hours.max(1), offers)
    };
    if offers.is_empty() || crate::ads::audience(state, user).await? == Audience::None {
        return Ok(None);
    }

    let tokens: Option<i64> = sqlx::query_scalar(
        "UPDATE users SET promo_shown = NOW()
        WHERE uuid = $1 AND (promo_shown IS NULL OR promo_shown < NOW() - make_interval(hours => $2))
        RETURNING tokens",
    )
    .bind(user.uuid)
    .bind(hours)
    .fetch_optional(state.database.write())
    .await?;
    let Some(tokens) = tokens else {
        return Ok(None);
    };

    let costs: Vec<(MembershipKind, i64)> = offers
        .iter()
        .map(|(kind, offer, _, _)| (*kind, offer.cost))
        .collect();
    let Some(kind) = pick(&costs, tokens) else {
        return Ok(None);
    };
    let Some((_, offer, name, mut perks)) = offers
        .into_iter()
        .find(|(offered, _, _, _)| *offered == kind)
    else {
        return Ok(None);
    };

    // the locations its tier opens, when there are public ones
    let tiers: &[&str] = match kind {
        MembershipKind::Plus => &[crate::economy::TIER_PLUS],
        MembershipKind::Plusplus => &[crate::economy::TIER_PLUS, crate::economy::TIER_PREMIUM],
    };
    let locations: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM locations WHERE public AND tier = ANY($1))",
    )
    .bind(tiers)
    .fetch_one(state.database.read())
    .await?;
    if locations {
        perks.insert(0, ApiPromoPerk::Locations);
    }

    Ok(Some(ApiPromo {
        membership: kind,
        name,
        cost: offer.cost,
        full_cost: offer.full_cost,
        days: offer.days,
        tokens,
        perks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dearest_affordable_membership_else_the_cheapest() {
        let offers = [(MembershipKind::Plus, 15), (MembershipKind::Plusplus, 30)];

        assert_eq!(pick(&offers, 9), Some(MembershipKind::Plus));
        assert_eq!(pick(&offers, 15), Some(MembershipKind::Plus));
        assert_eq!(pick(&offers, 29), Some(MembershipKind::Plus));
        assert_eq!(pick(&offers, 30), Some(MembershipKind::Plusplus));
        assert_eq!(
            pick(&[(MembershipKind::Plusplus, 30)], 0),
            Some(MembershipKind::Plusplus)
        );
        assert_eq!(pick(&[], 100), None);
    }

    #[test]
    fn not_on_the_first_day() {
        let created = chrono::NaiveDate::from_ymd_opt(2026, 10, 28)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        assert!(!past_first_day(
            created,
            created + chrono::Duration::hours(23)
        ));
        assert!(past_first_day(
            created,
            created + chrono::Duration::hours(24)
        ));
    }
}
