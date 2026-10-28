// plugged.host: the economy settings section (Admin › Settings › Economy, shared/src/economy.rs).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use crate::prelude::StringExt;
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The script Linkvertise's Full Script mode is served from.
pub const LINKVERTISE_SCRIPT_URL: &str = "https://publisher.linkvertise.com/cdn/linkvertise.js";

/// The two currencies, the memberships they buy and the Resource Store.
///
/// Credits buy CPU, memory and disk in the Resource Store; with noprofit.space connected the
/// balance, the store and its prices are noprofit.space's, and the store fields here are not
/// used. Tokens are earned from sponsored links and buy the memberships. The memberships' names
/// are what players see; the tiers behind them are always `plus` and `plusplus`.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsEconomy {
    pub credits_enabled: bool,
    /// Credits a new account starts with (without noprofit.space).
    pub starting_credits: i64,
    pub tokens_enabled: bool,
    /// Tokens every account gets once a day, the first time it looks at its balance.
    pub daily_tokens: i32,
    /// Whether the panel itself costs tokens (shared/src/panel_access.rs). Memberships and
    /// administrators never pay.
    pub panel_access_enabled: bool,
    /// Tokens a month of the panel costs.
    pub panel_access_tokens: i32,
    /// Days one payment buys.
    pub panel_access_days: i32,

    pub membership_enabled: bool,
    pub plus_name: compact_str::CompactString,
    pub plusplus_name: compact_str::CompactString,
    /// Tokens the first membership costs for `plus_days` days.
    pub plus_tokens: i32,
    pub plus_days: i32,
    pub plusplus_tokens: i32,
    pub plusplus_token_days: i32,
    /// Credits PlusPlus costs; 0 does not sell it for credits.
    pub plusplus_credits: i32,
    pub plusplus_credit_days: i32,
    /// Burst cores on each of the account's servers with the first membership: CPU above the
    /// paid limit (100 % per core) a server may use while the node has it to spare.
    pub plus_burst_cores: i32,
    /// The same with the second; the total, not added to the first's.
    pub plusplus_burst_cores: i32,

    /// The Resource Store without noprofit.space: the amount sold at once (CPU %, memory and disk
    /// MiB; 0 does not sell it) and the credits a step costs at the limit.
    pub cpu_step: i32,
    pub cpu_price: i32,
    pub memory_step: i32,
    pub memory_price: i32,
    pub disk_step: i32,
    pub disk_price: i32,
    /// Where a step reaches its full price (shared/src/pricing.rs).
    pub limit_cpu: i32,
    pub limit_memory: i32,
    pub limit_disk: i32,
    /// The least a step costs on the way to the limit, in percent of its price.
    pub price_floor: i32,
    /// The soft limit, in percent of the limit.
    pub soft_limit: i32,
    /// What a step costs at the soft limit, as a multiple of its price.
    pub soft_multiplier: i32,
    /// Past the soft limit the price doubles every this many percent of the limit.
    pub doubling: i32,
    /// A sale: percent off every price in the Resource Store (0: none), with or without
    /// noprofit.space, between the start and the end (either may be empty: open on that side).
    pub store_discount: i32,
    pub store_discount_starts: Option<chrono::DateTime<chrono::Utc>>,
    pub store_discount_ends: Option<chrono::DateTime<chrono::Utc>>,
    /// What the store calls the sale, such as "Halloween special"; empty: a plain "Sale".
    pub store_discount_name: Option<compact_str::CompactString>,

    /// Minutes a player has to come back from a sponsored link.
    pub claim_minutes: i32,
    /// Each further link a person completes on the same provider the same day pays this many percent
    /// less than the one before (advertisers pay less for a person they already showed ads to).
    pub link_decay_percent: i32,
    /// The lowest a link pays after that decay, in percent of its full reward.
    pub link_floor_percent: i32,
    pub linkvertise_enabled: bool,
    /// The publisher id for Linkvertise's Full Script mode; without it the link below is used.
    pub linkvertise_publisher: Option<compact_str::CompactString>,
    /// Patterns the Full Script rewrites links for, comma separated (`lvr=` when empty).
    pub linkvertise_whitelist: Option<compact_str::CompactString>,
    /// A Linkvertise link to the panel, with `{return}` or `{return_b64}` for the way back.
    pub linkvertise_link: Option<compact_str::CompactString>,
    /// The anti-bypass API token. Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub linkvertise_token: Option<compact_str::CompactString>,
    /// Tokens per link, up to 2 decimals (1.5: a whole token now and then, the rest saved per account).
    pub linkvertise_reward: f64,
    pub linkvertise_daily_cap: i32,
    pub vylo_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub vylo_api_key: Option<compact_str::CompactString>,
    pub vylo_reward: f64,
    pub vylo_daily_cap: i32,
    /// Seconds a Vylo trip must at least take.
    pub vylo_min_seconds: i32,
    pub cuty_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub cuty_api_key: Option<compact_str::CompactString>,
    pub cuty_reward: f64,
    pub cuty_daily_cap: i32,
    /// Seconds a Cuty trip must at least take.
    pub cuty_min_seconds: i32,
    pub shrinkearn_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub shrinkearn_api_key: Option<compact_str::CompactString>,
    pub shrinkearn_reward: f64,
    pub shrinkearn_daily_cap: i32,
    /// Seconds a ShrinkEarn trip must at least take.
    pub shrinkearn_min_seconds: i32,
    pub shrinkme_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub shrinkme_api_key: Option<compact_str::CompactString>,
    pub shrinkme_reward: f64,
    pub shrinkme_daily_cap: i32,
    /// Seconds a ShrinkMe trip must at least take.
    pub shrinkme_min_seconds: i32,
    pub adfocus_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub adfocus_api_key: Option<compact_str::CompactString>,
    pub adfocus_reward: f64,
    pub adfocus_daily_cap: i32,
    /// Seconds an AdFoc.us trip must at least take.
    pub adfocus_min_seconds: i32,
    /// CPX Research's survey wall (shared/src/earn.rs): each survey pays what CPX shows for it,
    /// worked out from the Currency Factor in CPX's dashboard, told by its postback.
    pub cpx_research_enabled: bool,
    pub cpx_research_app_id: Option<compact_str::CompactString>,
    /// The app's secure hash, which signs the wall's link and the postbacks. Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub cpx_research_secret: Option<compact_str::CompactString>,
    /// Finished surveys a day that pay; screen-outs and bonuses do not count. 0: no cap.
    pub cpx_research_daily_cap: i32,
}

impl Default for AppSettingsEconomy {
    fn default() -> Self {
        Self {
            credits_enabled: true,
            starting_credits: 0,
            tokens_enabled: true,
            daily_tokens: 0,
            panel_access_enabled: true,
            panel_access_tokens: 5,
            panel_access_days: 30,
            membership_enabled: true,
            plus_name: "Plus".into(),
            plusplus_name: "Premium".into(),
            plus_tokens: 15,
            plus_days: 30,
            plusplus_tokens: 30,
            plusplus_token_days: 30,
            plusplus_credits: 0,
            plusplus_credit_days: 30,
            plus_burst_cores: 1,
            plusplus_burst_cores: 2,
            cpu_step: 50,
            cpu_price: 100,
            memory_step: 1024,
            memory_price: 150,
            disk_step: 5120,
            disk_price: 75,
            limit_cpu: 600,
            limit_memory: 24576,
            limit_disk: 102400,
            price_floor: 50,
            soft_limit: 125,
            soft_multiplier: 4,
            doubling: 5,
            store_discount: 0,
            store_discount_starts: None,
            store_discount_ends: None,
            store_discount_name: None,
            claim_minutes: 30,
            link_decay_percent: 15,
            link_floor_percent: 25,
            linkvertise_enabled: false,
            linkvertise_publisher: None,
            linkvertise_whitelist: None,
            linkvertise_link: None,
            linkvertise_token: None,
            linkvertise_reward: 1.0,
            linkvertise_daily_cap: 5,
            vylo_enabled: false,
            vylo_api_key: None,
            vylo_reward: 1.0,
            vylo_daily_cap: 5,
            vylo_min_seconds: 10,
            cuty_enabled: false,
            cuty_api_key: None,
            cuty_reward: 1.0,
            cuty_daily_cap: 5,
            cuty_min_seconds: 10,
            shrinkearn_enabled: false,
            shrinkearn_api_key: None,
            shrinkearn_reward: 1.0,
            shrinkearn_daily_cap: 5,
            shrinkearn_min_seconds: 10,
            shrinkme_enabled: false,
            shrinkme_api_key: None,
            shrinkme_reward: 1.0,
            shrinkme_daily_cap: 5,
            shrinkme_min_seconds: 10,
            adfocus_enabled: false,
            adfocus_api_key: None,
            adfocus_reward: 1.0,
            adfocus_daily_cap: 5,
            adfocus_min_seconds: 10,
            cpx_research_enabled: false,
            cpx_research_app_id: None,
            cpx_research_secret: None,
            cpx_research_daily_cap: 1,
        }
    }
}

impl AppSettingsEconomy {
    /// The percent off the Resource Store's prices at `now`: the discount while its window is
    /// open, else 0.
    pub fn store_discount_at(&self, now: chrono::DateTime<chrono::Utc>) -> i64 {
        let started = self
            .store_discount_starts
            .is_none_or(|starts| now >= starts);
        let running = self.store_discount_ends.is_none_or(|ends| now < ends);

        if started && running {
            i64::from(self.store_discount.clamp(0, 100))
        } else {
            0
        }
    }

    /// Linkvertise's Full Script, which the Tokens page loads: on when Linkvertise is on with a
    /// publisher id and an API token.
    pub fn linkvertise_full_script(&self) -> bool {
        self.linkvertise_enabled
            && self.linkvertise_token.is_some()
            && self.linkvertise_publisher.is_some()
    }

    /// What `script-src` has to allow for the Tokens page.
    pub fn to_csp_script_src(&self) -> &'static str {
        if self.linkvertise_full_script() {
            " https://publisher.linkvertise.com"
        } else {
            ""
        }
    }
}

/// The economy tab of Admin › Settings: every field optional, `null` leaves it alone. A secret
/// sent empty or as the placeholder keeps the stored one; `null` removes it.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadEconomy {
    #[garde(skip)]
    credits_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    starting_credits: Option<i64>,
    #[garde(skip)]
    tokens_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100)))]
    daily_tokens: Option<i32>,
    #[garde(skip)]
    panel_access_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 100_000)))]
    panel_access_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 3650)))]
    panel_access_days: Option<i32>,

    #[garde(skip)]
    membership_enabled: Option<bool>,
    #[garde(inner(length(chars, min = 1, max = 32)))]
    plus_name: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, min = 1, max = 32)))]
    plusplus_name: Option<compact_str::CompactString>,
    #[garde(inner(range(min = 0, max = 100_000)))]
    plus_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    plus_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 100_000)))]
    plusplus_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    plusplus_token_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    plusplus_credits: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    plusplus_credit_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 16)))]
    plus_burst_cores: Option<i32>,
    #[garde(inner(range(min = 0, max = 16)))]
    plusplus_burst_cores: Option<i32>,

    #[garde(inner(range(min = 0, max = 10_000)))]
    cpu_step: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    cpu_price: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_048_576)))]
    memory_step: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    memory_price: Option<i32>,
    #[garde(inner(range(min = 0, max = 10_485_760)))]
    disk_step: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    disk_price: Option<i32>,
    #[garde(inner(range(min = 1, max = 100_000)))]
    limit_cpu: Option<i32>,
    #[garde(inner(range(min = 1, max = 10_485_760)))]
    limit_memory: Option<i32>,
    #[garde(inner(range(min = 1, max = 104_857_600)))]
    limit_disk: Option<i32>,
    #[garde(inner(range(min = 0, max = 100)))]
    price_floor: Option<i32>,
    #[garde(inner(range(min = 100, max = 1000)))]
    soft_limit: Option<i32>,
    #[garde(inner(range(min = 1, max = 100)))]
    soft_multiplier: Option<i32>,
    #[garde(inner(range(min = 1, max = 100)))]
    doubling: Option<i32>,
    #[garde(inner(range(min = 0, max = 100)))]
    store_discount: Option<i32>,
    /// `null` clears it; absent leaves it alone.
    #[garde(skip)]
    #[serde(default, with = "::serde_with::rust::double_option")]
    store_discount_starts: Option<Option<chrono::DateTime<chrono::Utc>>>,
    #[garde(skip)]
    #[serde(default, with = "::serde_with::rust::double_option")]
    store_discount_ends: Option<Option<chrono::DateTime<chrono::Utc>>>,
    #[garde(inner(inner(length(chars, max = 64))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    store_discount_name: Option<Option<compact_str::CompactString>>,

    #[garde(inner(range(min = 1, max = 1440)))]
    claim_minutes: Option<i32>,
    #[garde(inner(range(min = 0, max = 90)))]
    link_decay_percent: Option<i32>,
    #[garde(inner(range(min = 1, max = 100)))]
    link_floor_percent: Option<i32>,
    #[garde(skip)]
    linkvertise_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 20))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    linkvertise_publisher: Option<Option<compact_str::CompactString>>,
    #[garde(inner(inner(length(chars, max = 255))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    linkvertise_whitelist: Option<Option<compact_str::CompactString>>,
    #[garde(inner(inner(length(chars, max = 1024))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    linkvertise_link: Option<Option<compact_str::CompactString>>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    linkvertise_token: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    linkvertise_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    linkvertise_daily_cap: Option<i32>,
    #[garde(skip)]
    vylo_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    vylo_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    vylo_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    vylo_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    vylo_min_seconds: Option<i32>,
    #[garde(skip)]
    cuty_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    cuty_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    cuty_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    cuty_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    cuty_min_seconds: Option<i32>,
    #[garde(skip)]
    shrinkearn_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    shrinkearn_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    shrinkearn_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    shrinkearn_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    shrinkearn_min_seconds: Option<i32>,
    #[garde(skip)]
    shrinkme_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    shrinkme_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    shrinkme_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    shrinkme_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    shrinkme_min_seconds: Option<i32>,
    #[garde(skip)]
    adfocus_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    adfocus_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0.01, max = 1000.0)))]
    adfocus_reward: Option<f64>,
    #[garde(inner(range(min = 0, max = 1000)))]
    adfocus_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    adfocus_min_seconds: Option<i32>,
    #[garde(skip)]
    cpx_research_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 20))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    cpx_research_app_id: Option<Option<compact_str::CompactString>>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    cpx_research_secret: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 0, max = 1000)))]
    cpx_research_daily_cap: Option<i32>,
}

/// An optional text field: empty clears it.
fn text(value: Option<compact_str::CompactString>) -> Option<compact_str::CompactString> {
    value.and_then(|value| value.trim().to_compact_string().into_optional())
}

/// A secret: empty or the placeholder keeps the stored one, `null` removes it.
fn secret(
    value: Option<Option<compact_str::CompactString>>,
    stored: &mut Option<compact_str::CompactString>,
) {
    match value {
        Some(Some(value))
            if value.trim().is_empty() || value == crate::censor::CENSORED_PLACEHOLDER => {}
        Some(Some(value)) => *stored = Some(value.trim().into()),
        Some(None) => *stored = None,
        None => {}
    }
}

impl PayloadEconomy {
    /// Applies the payload. Fails when a Linkvertise link or publisher id is not usable.
    pub fn apply(self, settings: &mut AppSettingsEconomy) -> Result<(), &'static str> {
        macro_rules! set {
            ($($field:ident),* $(,)?) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }

        set!(
            credits_enabled,
            starting_credits,
            tokens_enabled,
            daily_tokens,
            panel_access_enabled,
            panel_access_tokens,
            panel_access_days,
            membership_enabled,
            plus_tokens,
            plus_days,
            plusplus_tokens,
            plusplus_token_days,
            plusplus_credits,
            plusplus_credit_days,
            plus_burst_cores,
            plusplus_burst_cores,
            cpu_step,
            cpu_price,
            memory_step,
            memory_price,
            disk_step,
            disk_price,
            limit_cpu,
            limit_memory,
            limit_disk,
            price_floor,
            soft_limit,
            soft_multiplier,
            doubling,
            store_discount,
            claim_minutes,
            link_decay_percent,
            link_floor_percent,
            linkvertise_enabled,
            linkvertise_reward,
            linkvertise_daily_cap,
            vylo_enabled,
            vylo_reward,
            vylo_daily_cap,
            vylo_min_seconds,
            cuty_enabled,
            cuty_reward,
            cuty_daily_cap,
            cuty_min_seconds,
            shrinkearn_enabled,
            shrinkearn_reward,
            shrinkearn_daily_cap,
            shrinkearn_min_seconds,
            shrinkme_enabled,
            shrinkme_reward,
            shrinkme_daily_cap,
            shrinkme_min_seconds,
            adfocus_enabled,
            adfocus_reward,
            adfocus_daily_cap,
            adfocus_min_seconds,
            cpx_research_enabled,
            cpx_research_daily_cap,
        );

        if let Some(name) = self.plus_name {
            settings.plus_name = name.trim().into();
        }
        if let Some(name) = self.plusplus_name {
            settings.plusplus_name = name.trim().into();
        }

        if let Some(starts) = self.store_discount_starts {
            settings.store_discount_starts = starts;
        }
        if let Some(ends) = self.store_discount_ends {
            settings.store_discount_ends = ends;
        }
        if let (Some(starts), Some(ends)) =
            (settings.store_discount_starts, settings.store_discount_ends)
            && ends <= starts
        {
            return Err("the discount must end after it starts");
        }
        if let Some(name) = self.store_discount_name {
            settings.store_discount_name = text(name);
        }

        if let Some(publisher) = self.linkvertise_publisher {
            let publisher = text(publisher);
            if publisher
                .as_ref()
                .is_some_and(|publisher| !publisher.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err("the linkvertise publisher id must be a number");
            }
            settings.linkvertise_publisher = publisher;
        }
        if let Some(whitelist) = self.linkvertise_whitelist {
            settings.linkvertise_whitelist = text(whitelist);
        }
        if let Some(link) = self.linkvertise_link {
            let link = text(link);
            if link
                .as_ref()
                .is_some_and(|link| !link.to_ascii_lowercase().starts_with("https://"))
            {
                return Err("the linkvertise link must start with https://");
            }
            settings.linkvertise_link = link;
        }
        secret(self.linkvertise_token, &mut settings.linkvertise_token);
        secret(self.vylo_api_key, &mut settings.vylo_api_key);
        secret(self.cuty_api_key, &mut settings.cuty_api_key);
        secret(self.shrinkearn_api_key, &mut settings.shrinkearn_api_key);
        secret(self.shrinkme_api_key, &mut settings.shrinkme_api_key);
        secret(self.adfocus_api_key, &mut settings.adfocus_api_key);
        if let Some(app_id) = self.cpx_research_app_id {
            let app_id = text(app_id);
            if app_id
                .as_ref()
                .is_some_and(|app_id| !app_id.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err("the cpx research app id must be a number");
            }
            settings.cpx_research_app_id = app_id;
        }
        secret(self.cpx_research_secret, &mut settings.cpx_research_secret);

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsEconomy {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        let mut serializer = serializer
            .write_raw_setting("credits_enabled", self.credits_enabled.to_compact_string())
            .write_raw_setting(
                "starting_credits",
                self.starting_credits.to_compact_string(),
            )
            .write_raw_setting("tokens_enabled", self.tokens_enabled.to_compact_string())
            .write_raw_setting("daily_tokens", self.daily_tokens.to_compact_string())
            .write_raw_setting(
                "panel_access_enabled",
                self.panel_access_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "panel_access_tokens",
                self.panel_access_tokens.to_compact_string(),
            )
            .write_raw_setting(
                "panel_access_days",
                self.panel_access_days.to_compact_string(),
            )
            .write_raw_setting(
                "membership_enabled",
                self.membership_enabled.to_compact_string(),
            )
            .write_raw_setting("plus_name", &*self.plus_name)
            .write_raw_setting("plusplus_name", &*self.plusplus_name);

        for (key, value) in [
            ("plus_tokens", self.plus_tokens),
            ("plus_days", self.plus_days),
            ("plusplus_tokens", self.plusplus_tokens),
            ("plusplus_token_days", self.plusplus_token_days),
            ("plusplus_credits", self.plusplus_credits),
            ("plusplus_credit_days", self.plusplus_credit_days),
            ("plus_burst_cores", self.plus_burst_cores),
            ("plusplus_burst_cores", self.plusplus_burst_cores),
            ("cpu_step", self.cpu_step),
            ("cpu_price", self.cpu_price),
            ("memory_step", self.memory_step),
            ("memory_price", self.memory_price),
            ("disk_step", self.disk_step),
            ("disk_price", self.disk_price),
            ("limit_cpu", self.limit_cpu),
            ("limit_memory", self.limit_memory),
            ("limit_disk", self.limit_disk),
            ("price_floor", self.price_floor),
            ("soft_limit", self.soft_limit),
            ("soft_multiplier", self.soft_multiplier),
            ("doubling", self.doubling),
            ("store_discount", self.store_discount),
            ("claim_minutes", self.claim_minutes),
            ("link_decay_percent", self.link_decay_percent),
            ("link_floor_percent", self.link_floor_percent),
            ("linkvertise_daily_cap", self.linkvertise_daily_cap),
            ("vylo_daily_cap", self.vylo_daily_cap),
            ("vylo_min_seconds", self.vylo_min_seconds),
            ("cuty_daily_cap", self.cuty_daily_cap),
            ("cuty_min_seconds", self.cuty_min_seconds),
            ("shrinkearn_daily_cap", self.shrinkearn_daily_cap),
            ("shrinkearn_min_seconds", self.shrinkearn_min_seconds),
            ("shrinkme_daily_cap", self.shrinkme_daily_cap),
            ("shrinkme_min_seconds", self.shrinkme_min_seconds),
            ("adfocus_daily_cap", self.adfocus_daily_cap),
            ("adfocus_min_seconds", self.adfocus_min_seconds),
            ("cpx_research_daily_cap", self.cpx_research_daily_cap),
        ] {
            serializer = serializer.write_raw_setting(key, value.to_compact_string());
        }

        // plugged.host: tokens per link, up to 2 decimals
        for (key, value) in [
            ("linkvertise_reward", self.linkvertise_reward),
            ("vylo_reward", self.vylo_reward),
            ("cuty_reward", self.cuty_reward),
            ("shrinkearn_reward", self.shrinkearn_reward),
            ("shrinkme_reward", self.shrinkme_reward),
            ("adfocus_reward", self.adfocus_reward),
        ] {
            serializer = serializer.write_raw_setting(
                key,
                crate::earn::tokens_text(crate::earn::hundredths(value)),
            );
        }

        for (key, time) in [
            ("store_discount_starts", self.store_discount_starts),
            ("store_discount_ends", self.store_discount_ends),
        ] {
            serializer = serializer.write_raw_setting(
                key,
                time.map(|time| time.to_rfc3339().to_compact_string())
                    .unwrap_or_default(),
            );
        }

        serializer = serializer
            .write_raw_setting(
                "store_discount_name",
                self.store_discount_name.clone().unwrap_or_default(),
            )
            .write_raw_setting(
                "linkvertise_enabled",
                self.linkvertise_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "linkvertise_publisher",
                self.linkvertise_publisher.clone().unwrap_or_default(),
            )
            .write_raw_setting(
                "linkvertise_whitelist",
                self.linkvertise_whitelist.clone().unwrap_or_default(),
            )
            .write_raw_setting(
                "linkvertise_link",
                self.linkvertise_link.clone().unwrap_or_default(),
            )
            .write_raw_setting("vylo_enabled", self.vylo_enabled.to_compact_string())
            .write_raw_setting("cuty_enabled", self.cuty_enabled.to_compact_string())
            .write_raw_setting(
                "shrinkearn_enabled",
                self.shrinkearn_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "shrinkme_enabled",
                self.shrinkme_enabled.to_compact_string(),
            )
            .write_raw_setting("adfocus_enabled", self.adfocus_enabled.to_compact_string())
            .write_raw_setting(
                "cpx_research_enabled",
                self.cpx_research_enabled.to_compact_string(),
            )
            .write_raw_setting(
                "cpx_research_app_id",
                self.cpx_research_app_id.clone().unwrap_or_default(),
            );

        for (key, secret) in [
            ("linkvertise_token", &self.linkvertise_token),
            ("vylo_api_key", &self.vylo_api_key),
            ("cuty_api_key", &self.cuty_api_key),
            ("shrinkearn_api_key", &self.shrinkearn_api_key),
            ("shrinkme_api_key", &self.shrinkme_api_key),
            ("adfocus_api_key", &self.adfocus_api_key),
            ("cpx_research_secret", &self.cpx_research_secret),
        ] {
            serializer = match secret {
                Some(secret) => {
                    serializer
                        .write_raw_encrypted_setting(key, secret.clone())
                        .await?
                }
                None => serializer.write_raw_setting(key, ""),
            };
        }

        Ok(serializer)
    }
}

pub struct AppSettingsEconomyDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsEconomyDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsEconomy::default();

        let mut secrets = Vec::with_capacity(7);
        for key in [
            "linkvertise_token",
            "vylo_api_key",
            "cuty_api_key",
            "shrinkearn_api_key",
            "shrinkme_api_key",
            "adfocus_api_key",
            "cpx_research_secret",
        ] {
            secrets.push(match deserializer.take_raw_setting(key) {
                Some(secret) if secret.is_empty() => None,
                Some(secret) => Some(deserializer.database.decrypt_base64(secret).await?),
                None => None,
            });
        }
        let cpx_research_secret = secrets.pop().flatten();
        let adfocus_api_key = secrets.pop().flatten();
        let shrinkme_api_key = secrets.pop().flatten();
        let shrinkearn_api_key = secrets.pop().flatten();
        let cuty_api_key = secrets.pop().flatten();
        let vylo_api_key = secrets.pop().flatten();
        let linkvertise_token = secrets.pop().flatten();

        macro_rules! read {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(defaults.$field)
            };
        }
        macro_rules! flag {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .map(|s| s == "true")
                    .unwrap_or(defaults.$field)
            };
        }
        macro_rules! text {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| s.into_optional())
            };
        }
        macro_rules! time {
            ($field:ident) => {
                deserializer
                    .take_raw_setting(stringify!($field))
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|time| time.with_timezone(&chrono::Utc))
            };
        }

        Ok(Box::new(AppSettingsEconomy {
            credits_enabled: flag!(credits_enabled),
            starting_credits: read!(starting_credits),
            tokens_enabled: flag!(tokens_enabled),
            daily_tokens: read!(daily_tokens),
            panel_access_enabled: flag!(panel_access_enabled),
            panel_access_tokens: read!(panel_access_tokens),
            panel_access_days: read!(panel_access_days),
            membership_enabled: flag!(membership_enabled),
            plus_name: text!(plus_name).unwrap_or(defaults.plus_name),
            plusplus_name: text!(plusplus_name).unwrap_or(defaults.plusplus_name),
            plus_tokens: read!(plus_tokens),
            plus_days: read!(plus_days),
            plusplus_tokens: read!(plusplus_tokens),
            plusplus_token_days: read!(plusplus_token_days),
            plusplus_credits: read!(plusplus_credits),
            plusplus_credit_days: read!(plusplus_credit_days),
            plus_burst_cores: read!(plus_burst_cores),
            plusplus_burst_cores: read!(plusplus_burst_cores),
            cpu_step: read!(cpu_step),
            cpu_price: read!(cpu_price),
            memory_step: read!(memory_step),
            memory_price: read!(memory_price),
            disk_step: read!(disk_step),
            disk_price: read!(disk_price),
            limit_cpu: read!(limit_cpu),
            limit_memory: read!(limit_memory),
            limit_disk: read!(limit_disk),
            price_floor: read!(price_floor),
            soft_limit: read!(soft_limit),
            soft_multiplier: read!(soft_multiplier),
            doubling: read!(doubling),
            store_discount: read!(store_discount),
            store_discount_starts: time!(store_discount_starts),
            store_discount_ends: time!(store_discount_ends),
            store_discount_name: text!(store_discount_name),
            claim_minutes: read!(claim_minutes),
            link_decay_percent: read!(link_decay_percent),
            link_floor_percent: read!(link_floor_percent),
            linkvertise_enabled: flag!(linkvertise_enabled),
            linkvertise_publisher: text!(linkvertise_publisher),
            linkvertise_whitelist: text!(linkvertise_whitelist),
            linkvertise_link: text!(linkvertise_link),
            linkvertise_token: linkvertise_token.and_then(|s| s.into_optional()),
            linkvertise_reward: read!(linkvertise_reward),
            linkvertise_daily_cap: read!(linkvertise_daily_cap),
            vylo_enabled: flag!(vylo_enabled),
            vylo_api_key: vylo_api_key.and_then(|s| s.into_optional()),
            vylo_reward: read!(vylo_reward),
            vylo_daily_cap: read!(vylo_daily_cap),
            vylo_min_seconds: read!(vylo_min_seconds),
            cuty_enabled: flag!(cuty_enabled),
            cuty_api_key: cuty_api_key.and_then(|s| s.into_optional()),
            cuty_reward: read!(cuty_reward),
            cuty_daily_cap: read!(cuty_daily_cap),
            cuty_min_seconds: read!(cuty_min_seconds),
            shrinkearn_enabled: flag!(shrinkearn_enabled),
            shrinkearn_api_key: shrinkearn_api_key.and_then(|s| s.into_optional()),
            shrinkearn_reward: read!(shrinkearn_reward),
            shrinkearn_daily_cap: read!(shrinkearn_daily_cap),
            shrinkearn_min_seconds: read!(shrinkearn_min_seconds),
            shrinkme_enabled: flag!(shrinkme_enabled),
            shrinkme_api_key: shrinkme_api_key.and_then(|s| s.into_optional()),
            shrinkme_reward: read!(shrinkme_reward),
            shrinkme_daily_cap: read!(shrinkme_daily_cap),
            shrinkme_min_seconds: read!(shrinkme_min_seconds),
            adfocus_enabled: flag!(adfocus_enabled),
            adfocus_api_key: adfocus_api_key.and_then(|s| s.into_optional()),
            adfocus_reward: read!(adfocus_reward),
            adfocus_daily_cap: read!(adfocus_daily_cap),
            adfocus_min_seconds: read!(adfocus_min_seconds),
            cpx_research_enabled: flag!(cpx_research_enabled),
            cpx_research_app_id: text!(cpx_research_app_id),
            cpx_research_secret: cpx_research_secret.and_then(|s| s.into_optional()),
            cpx_research_daily_cap: read!(cpx_research_daily_cap),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(1_800_000_000 + hours * 3600, 0).unwrap()
    }

    #[test]
    fn the_discount_runs_only_inside_its_window() {
        let mut economy = AppSettingsEconomy {
            store_discount: 25,
            ..Default::default()
        };
        assert_eq!(economy.store_discount_at(at(0)), 25);

        economy.store_discount_starts = Some(at(10));
        economy.store_discount_ends = Some(at(20));
        assert_eq!(economy.store_discount_at(at(9)), 0);
        assert_eq!(economy.store_discount_at(at(10)), 25);
        assert_eq!(economy.store_discount_at(at(19)), 25);
        assert_eq!(economy.store_discount_at(at(20)), 0);

        economy.store_discount = 0;
        assert_eq!(economy.store_discount_at(at(15)), 0);
    }

    #[test]
    fn a_window_must_end_after_it_starts() {
        let payload = |starts: i64, ends: i64| PayloadEconomy {
            store_discount_starts: Some(Some(at(starts))),
            store_discount_ends: Some(Some(at(ends))),
            ..serde_json::from_value(serde_json::json!({})).unwrap()
        };
        let mut economy = AppSettingsEconomy::default();

        assert!(payload(20, 10).apply(&mut economy).is_err());
        assert!(payload(10, 10).apply(&mut economy).is_err());
        assert!(payload(10, 20).apply(&mut economy).is_ok());
        assert_eq!(economy.store_discount_ends, Some(at(20)));
    }

    #[test]
    fn the_cpx_research_app_id_is_a_number_and_its_secret_is_kept() {
        let payload =
            |value: serde_json::Value| -> PayloadEconomy { serde_json::from_value(value).unwrap() };
        let mut economy = AppSettingsEconomy::default();
        assert_eq!(economy.cpx_research_daily_cap, 1);

        assert!(
            payload(serde_json::json!({ "cpx_research_app_id": "12a" }))
                .apply(&mut economy)
                .is_err()
        );
        assert!(
            payload(serde_json::json!({
                "cpx_research_app_id": " 12345 ",
                "cpx_research_secret": "<secret>",
            }))
            .apply(&mut economy)
            .is_ok()
        );
        assert_eq!(economy.cpx_research_app_id.as_deref(), Some("12345"));
        assert_eq!(economy.cpx_research_secret.as_deref(), Some("<secret>"));

        // sent empty, as the form does, the stored secret stays; null removes it
        payload(serde_json::json!({ "cpx_research_secret": "" }))
            .apply(&mut economy)
            .unwrap();
        assert_eq!(economy.cpx_research_secret.as_deref(), Some("<secret>"));
        payload(serde_json::json!({ "cpx_research_secret": null }))
            .apply(&mut economy)
            .unwrap();
        assert_eq!(economy.cpx_research_secret, None);
    }
}
