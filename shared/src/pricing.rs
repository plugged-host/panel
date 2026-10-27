//! plugged.host: what CPU, memory and disk cost in the Resource Store without noprofit.space
//! (docs/features/economy.md). The same curve as noprofit.space's own store, so the two price
//! alike; with noprofit.space connected its store and prices are used instead.
//!
//! There is no ceiling. The price of a step depends on how much the account already has, measured
//! against its limit:
//!
//! - up to the limit, a step costs its share of the way there: half the step price at half the
//!   limit (never less than the floor), the full step price at the limit;
//! - from the limit to the soft limit the price grows smoothly to the soft multiplier times the
//!   step price;
//! - past the soft limit it doubles every `doubling` percent of the limit.
//!
//! A price is the area under that curve, so four steps bought at once cost what four single steps
//! would.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A price this high or higher: more than anyone can pay. Keeps the curve's far end finite.
pub const UNAFFORDABLE: i64 = 1_000_000_000;

/// The offers per resource, in steps.
pub const OFFER_STEPS: [i64; 3] = [1, 2, 4];

/// The most steps bought at once.
pub const MAX_STEPS: i64 = 50;

/// `price` with `percent` off, rounded half up, as noprofit.space rounds it. A price nobody can
/// pay stays one.
pub fn discounted(price: i64, percent: i64) -> i64 {
    let percent = percent.clamp(0, 100);

    if percent == 0 || price >= UNAFFORDABLE {
        return price;
    }

    (price * (100 - percent) + 50) / 100
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StoreResource {
    Cpu,
    Memory,
    Disk,
}

impl StoreResource {
    pub const ALL: [Self; 3] = [Self::Cpu, Self::Memory, Self::Disk];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pricing {
    /// The amount sold at once per resource (CPU %, memory and disk MiB), 0 when not for sale.
    pub steps: [i64; 3],
    /// Credits for one step at the limit.
    pub prices: [i64; 3],
    /// Where a step reaches its full price.
    pub limits: [i64; 3],
    pub floor_percent: f64,
    pub soft_limit_percent: f64,
    pub soft_multiplier: f64,
    pub doubling_percent: f64,
}

impl Pricing {
    pub fn from_settings(settings: &crate::settings::economy::AppSettingsEconomy) -> Self {
        Self {
            steps: [
                settings.cpu_step as i64,
                settings.memory_step as i64,
                settings.disk_step as i64,
            ],
            prices: [
                settings.cpu_price as i64,
                settings.memory_price as i64,
                settings.disk_price as i64,
            ],
            limits: [
                settings.limit_cpu as i64,
                settings.limit_memory as i64,
                settings.limit_disk as i64,
            ],
            floor_percent: settings.price_floor as f64,
            soft_limit_percent: settings.soft_limit as f64,
            soft_multiplier: settings.soft_multiplier as f64,
            doubling_percent: settings.doubling as f64,
        }
    }

    fn index(resource: StoreResource) -> usize {
        match resource {
            StoreResource::Cpu => 0,
            StoreResource::Memory => 1,
            StoreResource::Disk => 2,
        }
    }

    pub fn step(&self, resource: StoreResource) -> i64 {
        self.steps[Self::index(resource)].max(0)
    }

    pub fn step_price(&self, resource: StoreResource) -> i64 {
        self.prices[Self::index(resource)].max(0)
    }

    pub fn limit(&self, resource: StoreResource) -> i64 {
        self.limits[Self::index(resource)].max(1)
    }

    pub fn soft_limit(&self, limit: i64) -> i64 {
        (limit.max(1) as f64 * self.soft_factor()).round() as i64
    }

    /// Credits for growing a resource from `from` by `amount` on an account whose limit is
    /// `limit`.
    pub fn cost(&self, resource: StoreResource, from: i64, amount: i64, limit: i64) -> i64 {
        let step = self.step(resource);

        if amount <= 0 || step <= 0 {
            return 0;
        }

        let limit = limit.max(1) as f64;
        let from = from.max(0) as f64;
        let credits = self.step_price(resource) as f64 / step as f64
            * (self.area(from + amount as f64, limit) - self.area(from, limit));

        if !credits.is_finite() || credits >= UNAFFORDABLE as f64 {
            return UNAFFORDABLE;
        }

        // Rounded first so float noise (100.0000000001) does not cost a whole extra credit.
        ((credits * 1e6).round() / 1e6).ceil().max(0.0) as i64
    }

    /// The price of a step at `at`, as a multiple of the step price.
    pub fn multiplier(&self, at: i64, limit: i64) -> f64 {
        let limit = limit.max(1) as f64;
        let at = at as f64;
        let soft = limit * self.soft_factor();

        if at <= limit {
            return self.floor().max(at / limit);
        }

        if at <= soft {
            return self.growth().powf((at - limit) / (soft - limit));
        }

        (self.peak() * 2f64.powf((at - soft) / (limit * self.doubling()))).min(UNAFFORDABLE as f64)
    }

    /// The curve's area from 0 to `x`, in step prices per resource unit.
    fn area(&self, x: f64, limit: f64) -> f64 {
        let floor = self.floor();
        let knee = floor * limit;

        // Up to the limit: max(floor, x / limit).
        if x <= knee {
            return floor * x;
        }

        let to_limit = |y: f64| (y.powi(2) - knee.powi(2)) / (2.0 * limit) + floor * knee;

        if x <= limit {
            return to_limit(x);
        }

        // Limit to soft limit: growth ** ((x - limit) / (soft - limit)).
        let soft = limit * self.soft_factor();
        let band = soft - limit;
        let growth = self.growth();
        let rise = |y: f64| {
            if band <= 0.0 {
                0.0
            } else if growth <= 1.0 {
                y - limit
            } else {
                band / growth.ln() * (growth.powf((y - limit) / band) - 1.0)
            }
        };

        if x <= soft {
            return to_limit(limit) + rise(x);
        }

        // Past the soft limit: doubling every `doubling` of the limit.
        let doubling = limit * self.doubling();

        to_limit(limit)
            + rise(soft)
            + self.peak() * doubling / std::f64::consts::LN_2
                * (2f64.powf((x - soft) / doubling) - 1.0)
    }

    fn floor(&self) -> f64 {
        self.floor_percent.clamp(0.0, 100.0) / 100.0
    }

    fn soft_factor(&self) -> f64 {
        self.soft_limit_percent.max(100.0) / 100.0
    }

    fn growth(&self) -> f64 {
        self.soft_multiplier.max(1.0)
    }

    /// The multiple a step costs at the soft limit, where the doubling starts.
    fn peak(&self) -> f64 {
        if self.soft_factor() > 1.0 {
            self.growth()
        } else {
            1.0
        }
    }

    fn doubling(&self) -> f64 {
        self.doubling_percent.max(1.0) / 100.0
    }
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct Offer {
    pub steps: i64,
    pub amount: i64,
    /// What it costs now, the discount taken off.
    pub price: i64,
    /// What it costs without the discount; `price` when there is none. noprofit.space answers
    /// without it when it applied none (filled in by [`Offer::fill_full_price`]).
    #[serde(default)]
    pub full_price: i64,
}

impl Offer {
    /// A discount never raises a price: an offer without a full price costs it in full.
    pub fn fill_full_price(&mut self) {
        self.full_price = self.full_price.max(self.price);
    }
}

/// A resource in the store, in the shape noprofit.space's store answers with.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug)]
pub struct CatalogEntry {
    pub resource: StoreResource,
    pub step: i64,
    /// What the account has of it for pricing (the allowance, bought resources included).
    pub total: i64,
    pub limit: i64,
    pub soft_limit: i64,
    pub offers: Vec<Offer>,
}

impl Pricing {
    /// The store's offers for an account that has `total` of each resource, `discount` percent
    /// off.
    pub fn catalog(&self, totals: [i64; 3], discount: i64) -> Vec<CatalogEntry> {
        StoreResource::ALL
            .into_iter()
            .map(|resource| {
                let step = self.step(resource);
                let total = totals[Self::index(resource)].max(0);
                let limit = self.limit(resource);

                CatalogEntry {
                    resource,
                    step,
                    total,
                    limit,
                    soft_limit: self.soft_limit(limit),
                    offers: if step <= 0 {
                        Vec::new()
                    } else {
                        OFFER_STEPS
                            .into_iter()
                            .map(|steps| {
                                let full_price = self.cost(resource, total, steps * step, limit);

                                Offer {
                                    steps,
                                    amount: steps * step,
                                    price: discounted(full_price, discount),
                                    full_price,
                                }
                            })
                            .collect()
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing() -> Pricing {
        Pricing::from_settings(&crate::settings::economy::AppSettingsEconomy::default())
    }

    #[test]
    fn a_step_costs_the_floor_early_and_the_full_price_at_the_limit() {
        let pricing = pricing();

        // 50 % CPU from nothing: the floor, half of 100 credits.
        assert_eq!(pricing.cost(StoreResource::Cpu, 0, 50, 600), 50);
        // The last step up to the limit: just under the full price.
        assert_eq!(pricing.cost(StoreResource::Cpu, 550, 50, 600), 96);
        assert!((pricing.multiplier(600, 600) - 1.0).abs() < 1e-9);
        assert!((pricing.multiplier(0, 600) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn several_steps_cost_what_single_steps_would() {
        let pricing = pricing();

        for from in [0, 100, 400, 600, 700, 900] {
            let at_once = pricing.cost(StoreResource::Cpu, from, 200, 600);
            let one_by_one: i64 = (0..4)
                .map(|i| pricing.cost(StoreResource::Cpu, from + i * 50, 50, 600))
                .sum();

            // Each single price is rounded up on its own, so they may add up to a little more.
            assert!(one_by_one >= at_once && one_by_one - at_once <= 4, "{from}");
        }
    }

    #[test]
    fn the_price_climbs_past_the_limit_and_doubles_past_the_soft_limit() {
        let pricing = pricing();
        let soft = pricing.soft_limit(600);

        assert_eq!(soft, 750);
        assert!((pricing.multiplier(soft, 600) - 4.0).abs() < 1e-9);
        // 5 % of the limit (30) past the soft limit: twice the peak.
        assert!((pricing.multiplier(soft + 30, 600) - 8.0).abs() < 1e-9);

        let below = pricing.cost(StoreResource::Cpu, 500, 50, 600);
        let past = pricing.cost(StoreResource::Cpu, 650, 50, 600);
        let steep = pricing.cost(StoreResource::Cpu, 800, 50, 600);
        assert!(below < past && past < steep);
    }

    #[test]
    fn the_far_end_is_unaffordable_not_infinite() {
        let pricing = pricing();

        assert_eq!(
            pricing.cost(StoreResource::Cpu, 100_000, 50, 600),
            UNAFFORDABLE
        );
    }

    #[test]
    fn nothing_is_sold_without_a_step() {
        let mut pricing = pricing();
        pricing.steps[1] = 0;

        assert_eq!(pricing.cost(StoreResource::Memory, 0, 1024, 24576), 0);
        let catalog = pricing.catalog([0, 0, 0], 0);
        assert!(catalog[1].offers.is_empty());
        assert_eq!(catalog[0].offers.len(), OFFER_STEPS.len());
        assert_eq!(catalog[0].offers[2].amount, 200);
    }

    #[test]
    fn a_flat_curve_still_prices() {
        let mut pricing = pricing();
        pricing.soft_limit_percent = 100.0;
        pricing.soft_multiplier = 1.0;

        let at_limit = pricing.cost(StoreResource::Disk, 102_400, 5120, 102_400);
        assert!(at_limit >= 75);
        assert!(at_limit < UNAFFORDABLE);
    }

    #[test]
    fn a_discount_rounds_half_up_and_leaves_the_unaffordable_alone() {
        assert_eq!(discounted(100, 0), 100);
        assert_eq!(discounted(100, 25), 75);
        assert_eq!(discounted(75, 50), 38);
        assert_eq!(discounted(1, 50), 1);
        assert_eq!(discounted(1, 51), 0);
        assert_eq!(discounted(150, 100), 0);
        assert_eq!(discounted(150, 150), 0);
        assert_eq!(discounted(150, -5), 150);
        assert_eq!(discounted(UNAFFORDABLE, 50), UNAFFORDABLE);
    }

    #[test]
    fn a_discounted_offer_keeps_its_full_price() {
        let pricing = pricing();

        let full = pricing.catalog([0, 0, 0], 0);
        let sale = pricing.catalog([0, 0, 0], 25);
        for (full, sale) in full.iter().zip(&sale) {
            for (full, sale) in full.offers.iter().zip(&sale.offers) {
                assert_eq!(full.price, full.full_price);
                assert_eq!(sale.full_price, full.price);
                assert_eq!(sale.price, discounted(full.price, 25));
            }
        }
        // 50 % CPU from nothing costs 50 credits, 38 at a quarter off
        assert_eq!(sale[0].offers[0].price, 38);
    }

    #[test]
    fn an_offer_from_noprofit_without_a_full_price_costs_it_in_full() {
        let mut offer: Offer =
            serde_json::from_value(serde_json::json!({"steps": 1, "amount": 50, "price": 40}))
                .unwrap();
        offer.fill_full_price();

        assert_eq!(offer.full_price, 40);
    }
}
