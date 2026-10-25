// plugged.host: the partner program settings section (docs/features/partners.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::ToCompactString;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Discord communities that apply to be partners and get credits every month once an admin
/// approves them (shared/src/partners.rs).
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsPartners {
    /// Players see Account › Partners and may apply (it also needs credits on, the moderation
    /// bot's token and a way to know their Discord account).
    pub enabled: bool,
    /// The fewest members a Discord server needs (Discord's approximate count).
    pub min_members: i32,
    /// The range neo's suggested monthly amount is kept in. An admin gives 0 up to the maximum a
    /// month.
    pub min_credits: i32,
    pub max_credits: i32,
    /// Days after a rejection or a revoked partnership before the person, or anyone for that
    /// Discord server, may apply again.
    pub cooldown_days: i32,
    /// Days a partnership whose monthly check failed keeps its held payment before it is
    /// suspended.
    pub grace_days: i32,
    /// Partners need a channel about the host that everyone on their server can see.
    pub channel_required: bool,
    /// More names the channel's name or topic may carry besides the panel's name (one per line).
    pub channel_names: compact_str::CompactString,
    /// Links the channel's topic must hold one of (one per line, like `example.com`); empty: no
    /// link needed.
    pub channel_links: compact_str::CompactString,
    /// neo assesses each application when it is sent (it only suggests; an admin decides).
    pub neo_assessment: bool,
    /// What neo should weigh, in the admin's words; empty: only the built-in criteria.
    pub guidance: compact_str::CompactString,
}

impl Default for AppSettingsPartners {
    fn default() -> Self {
        Self {
            enabled: false,
            min_members: 500,
            min_credits: 500,
            max_credits: 5000,
            cooldown_days: 30,
            grace_days: 7,
            channel_required: true,
            channel_names: compact_str::CompactString::default(),
            channel_links: compact_str::CompactString::default(),
            neo_assessment: true,
            guidance: compact_str::CompactString::default(),
        }
    }
}

impl AppSettingsPartners {
    /// The bounds a monthly amount is kept in: `(min, max)`, never upside down and never below 0.
    pub fn bounds(&self) -> (i64, i64) {
        let min = self.min_credits.max(0) as i64;

        (min, (self.max_credits.max(0) as i64).max(min))
    }

    /// The extra names the channel may carry, as the admin wrote them.
    pub fn channel_names(&self) -> Vec<String> {
        lines(&self.channel_names)
    }

    /// The links the channel's topic must hold one of, as the admin wrote them.
    pub fn channel_links(&self) -> Vec<String> {
        lines(&self.channel_links)
    }
}

/// The entries of a one-per-line (or comma-separated) list: trimmed, without empty ones or
/// repeats.
fn lines(text: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    for entry in text.split(['\n', ',']).map(str::trim) {
        if !entry.is_empty() && entries.len() < 20 && !entries.iter().any(|known| known == entry) {
            entries.push(entry.chars().take(100).collect());
        }
    }

    entries
}

/// The partners tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadPartners {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(inner(range(min = 1, max = 10_000_000)))]
    min_members: Option<i32>,
    #[garde(inner(range(min = 0, max = 10_000_000)))]
    min_credits: Option<i32>,
    #[garde(inner(range(min = 0, max = 10_000_000)))]
    max_credits: Option<i32>,
    #[garde(inner(range(min = 0, max = 365)))]
    cooldown_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 60)))]
    grace_days: Option<i32>,
    #[garde(skip)]
    channel_required: Option<bool>,
    #[garde(inner(length(chars, max = 500)))]
    channel_names: Option<compact_str::CompactString>,
    #[garde(inner(length(chars, max = 500)))]
    channel_links: Option<compact_str::CompactString>,
    #[garde(skip)]
    neo_assessment: Option<bool>,
    #[garde(inner(length(chars, max = 2000)))]
    guidance: Option<compact_str::CompactString>,
}

impl PayloadPartners {
    /// Applies the fields that were sent; refuses a minimum above the maximum.
    pub fn apply(self, settings: &mut AppSettingsPartners) -> Result<(), &'static str> {
        let min_credits = self.min_credits.unwrap_or(settings.min_credits);
        let max_credits = self.max_credits.unwrap_or(settings.max_credits);
        if min_credits > max_credits {
            return Err("the smallest amount of credits must not be above the largest");
        }

        if let Some(enabled) = self.enabled {
            settings.enabled = enabled;
        }
        if let Some(min_members) = self.min_members {
            settings.min_members = min_members;
        }
        settings.min_credits = min_credits;
        settings.max_credits = max_credits;
        if let Some(cooldown_days) = self.cooldown_days {
            settings.cooldown_days = cooldown_days;
        }
        if let Some(grace_days) = self.grace_days {
            settings.grace_days = grace_days;
        }
        if let Some(channel_required) = self.channel_required {
            settings.channel_required = channel_required;
        }
        if let Some(channel_names) = self.channel_names {
            settings.channel_names = lines(&channel_names).join("\n").into();
        }
        if let Some(channel_links) = self.channel_links {
            settings.channel_links = lines(&channel_links).join("\n").into();
        }
        if let Some(neo_assessment) = self.neo_assessment {
            settings.neo_assessment = neo_assessment;
        }
        if let Some(guidance) = self.guidance {
            settings.guidance = guidance.trim().into();
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsPartners {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("enabled", self.enabled.to_compact_string())
            .write_raw_setting("min_members", self.min_members.to_compact_string())
            .write_raw_setting("min_credits", self.min_credits.to_compact_string())
            .write_raw_setting("max_credits", self.max_credits.to_compact_string())
            .write_raw_setting("cooldown_days", self.cooldown_days.to_compact_string())
            .write_raw_setting("grace_days", self.grace_days.to_compact_string())
            .write_raw_setting(
                "channel_required",
                self.channel_required.to_compact_string(),
            )
            .write_raw_setting("channel_names", self.channel_names.clone())
            .write_raw_setting("channel_links", self.channel_links.clone())
            .write_raw_setting("neo_assessment", self.neo_assessment.to_compact_string())
            .write_raw_setting("guidance", self.guidance.clone()))
    }
}

pub struct AppSettingsPartnersDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsPartnersDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsPartners::default();
        let mut number = |key: &str, default: i32| {
            deserializer
                .take_raw_setting(key)
                .and_then(|s| s.parse().ok())
                .unwrap_or(default)
        };
        let min_members = number("min_members", defaults.min_members);
        let min_credits = number("min_credits", defaults.min_credits);
        let max_credits = number("max_credits", defaults.max_credits);
        let cooldown_days = number("cooldown_days", defaults.cooldown_days);
        let grace_days = number("grace_days", defaults.grace_days);

        Ok(Box::new(AppSettingsPartners {
            enabled: deserializer
                .take_raw_setting("enabled")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.enabled),
            min_members,
            min_credits,
            max_credits,
            cooldown_days,
            grace_days,
            channel_required: deserializer
                .take_raw_setting("channel_required")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.channel_required),
            channel_names: deserializer
                .take_raw_setting("channel_names")
                .unwrap_or(defaults.channel_names),
            channel_links: deserializer
                .take_raw_setting("channel_links")
                .unwrap_or(defaults.channel_links),
            neo_assessment: deserializer
                .take_raw_setting("neo_assessment")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.neo_assessment),
            guidance: deserializer
                .take_raw_setting("guidance")
                .unwrap_or(defaults.guidance),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_never_upside_down() {
        let mut settings = AppSettingsPartners::default();
        assert_eq!(settings.bounds(), (500, 5000));

        settings.min_credits = 800;
        settings.max_credits = 100;
        assert_eq!(settings.bounds(), (800, 800));

        settings.min_credits = -5;
        settings.max_credits = -1;
        assert_eq!(settings.bounds(), (0, 0));
    }

    #[test]
    fn a_minimum_above_the_maximum_is_refused() {
        let mut settings = AppSettingsPartners::default();
        let payload = |min: Option<i32>, max: Option<i32>| PayloadPartners {
            enabled: Some(true),
            min_members: None,
            min_credits: min,
            max_credits: max,
            cooldown_days: None,
            grace_days: None,
            channel_required: None,
            channel_names: Some(" partners,\n\nhosting \npartners".into()),
            channel_links: None,
            neo_assessment: None,
            guidance: Some("  big FiveM servers first \n".into()),
        };

        assert!(payload(Some(6000), None).apply(&mut settings).is_err());
        // nothing changed
        assert!(!settings.enabled);

        assert!(payload(Some(100), Some(200)).apply(&mut settings).is_ok());
        assert!(settings.enabled);
        assert_eq!(settings.bounds(), (100, 200));
        assert_eq!(settings.guidance, "big FiveM servers first");
        assert_eq!(settings.channel_names, "partners\nhosting");
        assert_eq!(settings.channel_names(), vec!["partners", "hosting"]);
        assert!(settings.channel_links().is_empty());
    }
}
