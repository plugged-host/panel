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

    pub membership_enabled: bool,
    pub plus_name: compact_str::CompactString,
    pub plusplus_name: compact_str::CompactString,
    /// Tokens Plus costs, once.
    pub plus_tokens: i32,
    pub plusplus_tokens: i32,
    pub plusplus_token_days: i32,
    /// Credits PlusPlus costs; 0 does not sell it for credits.
    pub plusplus_credits: i32,
    pub plusplus_credit_days: i32,
    /// CPU (%) PlusPlus adds to the account's allowance while it runs.
    pub plusplus_cpu: i32,

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

    /// Minutes a player has to come back from a sponsored link.
    pub claim_minutes: i32,
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
    pub linkvertise_reward: i32,
    pub linkvertise_daily_cap: i32,
    pub vylo_enabled: bool,
    /// Stored encrypted.
    #[serde(serialize_with = "super::noprofit::serialize_secret")]
    #[schema(value_type = Option<String>)]
    pub vylo_api_key: Option<compact_str::CompactString>,
    pub vylo_reward: i32,
    pub vylo_daily_cap: i32,
    /// Seconds a Vylo trip must at least take.
    pub vylo_min_seconds: i32,
}

impl Default for AppSettingsEconomy {
    fn default() -> Self {
        Self {
            credits_enabled: true,
            starting_credits: 0,
            tokens_enabled: true,
            daily_tokens: 0,
            membership_enabled: true,
            plus_name: "Plus".into(),
            plusplus_name: "PlusPlus".into(),
            plus_tokens: 25,
            plusplus_tokens: 10,
            plusplus_token_days: 7,
            plusplus_credits: 150,
            plusplus_credit_days: 14,
            plusplus_cpu: 50,
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
            claim_minutes: 30,
            linkvertise_enabled: false,
            linkvertise_publisher: None,
            linkvertise_whitelist: None,
            linkvertise_link: None,
            linkvertise_token: None,
            linkvertise_reward: 1,
            linkvertise_daily_cap: 5,
            vylo_enabled: false,
            vylo_api_key: None,
            vylo_reward: 1,
            vylo_daily_cap: 5,
            vylo_min_seconds: 10,
        }
    }
}

impl AppSettingsEconomy {
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
    membership_enabled: Option<bool>,
    #[garde(inner(length(chars, min = 1, max = 32)))]
    plus_name: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, min = 1, max = 32)))]
    plusplus_name: Option<compact_str::CompactString>,
    #[garde(inner(range(min = 0, max = 100_000)))]
    plus_tokens: Option<i32>,
    #[garde(inner(range(min = 0, max = 100_000)))]
    plusplus_tokens: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    plusplus_token_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    plusplus_credits: Option<i32>,
    #[garde(inner(range(min = 1, max = 365)))]
    plusplus_credit_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000)))]
    plusplus_cpu: Option<i32>,

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

    #[garde(inner(range(min = 1, max = 1440)))]
    claim_minutes: Option<i32>,
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
    #[garde(inner(range(min = 1, max = 1000)))]
    linkvertise_reward: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000)))]
    linkvertise_daily_cap: Option<i32>,
    #[garde(skip)]
    vylo_enabled: Option<bool>,
    #[garde(inner(inner(length(chars, max = 512))))]
    #[serde(default, with = "::serde_with::rust::double_option")]
    vylo_api_key: Option<Option<compact_str::CompactString>>,
    #[garde(inner(range(min = 1, max = 1000)))]
    vylo_reward: Option<i32>,
    #[garde(inner(range(min = 0, max = 1000)))]
    vylo_daily_cap: Option<i32>,
    #[garde(inner(range(min = 0, max = 600)))]
    vylo_min_seconds: Option<i32>,
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
            membership_enabled,
            plus_tokens,
            plusplus_tokens,
            plusplus_token_days,
            plusplus_credits,
            plusplus_credit_days,
            plusplus_cpu,
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
            claim_minutes,
            linkvertise_enabled,
            linkvertise_reward,
            linkvertise_daily_cap,
            vylo_enabled,
            vylo_reward,
            vylo_daily_cap,
            vylo_min_seconds,
        );

        if let Some(name) = self.plus_name {
            settings.plus_name = name.trim().into();
        }
        if let Some(name) = self.plusplus_name {
            settings.plusplus_name = name.trim().into();
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
                "membership_enabled",
                self.membership_enabled.to_compact_string(),
            )
            .write_raw_setting("plus_name", &*self.plus_name)
            .write_raw_setting("plusplus_name", &*self.plusplus_name);

        for (key, value) in [
            ("plus_tokens", self.plus_tokens),
            ("plusplus_tokens", self.plusplus_tokens),
            ("plusplus_token_days", self.plusplus_token_days),
            ("plusplus_credits", self.plusplus_credits),
            ("plusplus_credit_days", self.plusplus_credit_days),
            ("plusplus_cpu", self.plusplus_cpu),
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
            ("claim_minutes", self.claim_minutes),
            ("linkvertise_reward", self.linkvertise_reward),
            ("linkvertise_daily_cap", self.linkvertise_daily_cap),
            ("vylo_reward", self.vylo_reward),
            ("vylo_daily_cap", self.vylo_daily_cap),
            ("vylo_min_seconds", self.vylo_min_seconds),
        ] {
            serializer = serializer.write_raw_setting(key, value.to_compact_string());
        }

        serializer = serializer
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
            .write_raw_setting("vylo_enabled", self.vylo_enabled.to_compact_string());

        for (key, secret) in [
            ("linkvertise_token", &self.linkvertise_token),
            ("vylo_api_key", &self.vylo_api_key),
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

        let mut secrets = Vec::with_capacity(2);
        for key in ["linkvertise_token", "vylo_api_key"] {
            secrets.push(match deserializer.take_raw_setting(key) {
                Some(secret) if secret.is_empty() => None,
                Some(secret) => Some(deserializer.database.decrypt_base64(secret).await?),
                None => None,
            });
        }
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

        Ok(Box::new(AppSettingsEconomy {
            credits_enabled: flag!(credits_enabled),
            starting_credits: read!(starting_credits),
            tokens_enabled: flag!(tokens_enabled),
            daily_tokens: read!(daily_tokens),
            membership_enabled: flag!(membership_enabled),
            plus_name: text!(plus_name).unwrap_or(defaults.plus_name),
            plusplus_name: text!(plusplus_name).unwrap_or(defaults.plusplus_name),
            plus_tokens: read!(plus_tokens),
            plusplus_tokens: read!(plusplus_tokens),
            plusplus_token_days: read!(plusplus_token_days),
            plusplus_credits: read!(plusplus_credits),
            plusplus_credit_days: read!(plusplus_credit_days),
            plusplus_cpu: read!(plusplus_cpu),
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
            claim_minutes: read!(claim_minutes),
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
        }))
    }
}
