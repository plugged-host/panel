// plugged.host: the reviews settings section (Admin › Settings › Accounts › Reviews, docs/features/reviews.md).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::{CompactString, ToCompactString};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Where players are asked to review the host: a prompt on the server list, shown to every
/// player the same way once their account is old enough and has a server, and a link in the
/// account menu. Off while the URL is empty.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsReviews {
    /// The page where a review is written (a review site's "write a review" link); empty: no
    /// prompt and no link.
    pub url: CompactString,
    /// The review site's name players see ("Trustpilot"); empty: the URL's host.
    pub site: CompactString,
    /// How old an account must be before the prompt shows, in days.
    pub delay_days: i32,
    /// How long the prompt stays away after it was closed or followed, in days; 0: it never
    /// comes back.
    pub interval_days: i32,
}

impl Default for AppSettingsReviews {
    fn default() -> Self {
        Self {
            url: CompactString::default(),
            site: CompactString::default(),
            delay_days: 3,
            interval_days: 180,
        }
    }
}

impl AppSettingsReviews {
    /// Whether the prompt and the link are offered.
    #[inline]
    pub fn active(&self) -> bool {
        !self.url.is_empty()
    }

    /// The site's name as players see it: the configured name, else the URL's host without a
    /// leading `www.`; empty while reviews are off.
    pub fn site_name(&self) -> CompactString {
        if !self.active() {
            return CompactString::default();
        }
        if !self.site.is_empty() {
            return self.site.clone();
        }

        reqwest::Url::parse(&self.url)
            .ok()
            .and_then(|url| url.host_str().map(|host| host.to_compact_string()))
            .map(|host| match host.strip_prefix("www.") {
                Some(rest) if !rest.is_empty() => rest.to_compact_string(),
                _ => host,
            })
            .unwrap_or_default()
    }
}

/// Why the reviews tab was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PayloadError {
    Url,
}

impl PayloadError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Url => "the review page must be an https address",
        }
    }
}

/// The reviews tab of Admin › Settings: every field optional, `null` leaves it alone.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadReviews {
    #[garde(inner(length(chars, max = 512)))]
    url: Option<CompactString>,
    #[garde(inner(length(chars, max = 64)))]
    site: Option<CompactString>,
    #[garde(inner(range(min = 0, max = 365)))]
    delay_days: Option<i32>,
    #[garde(inner(range(min = 0, max = 3650)))]
    interval_days: Option<i32>,
}

impl PayloadReviews {
    /// Applies the payload; a refused one changes nothing.
    pub fn apply(self, settings: &mut AppSettingsReviews) -> Result<(), PayloadError> {
        let url = self.url.as_ref().map(|url| url.trim().to_compact_string());
        if let Some(url) = &url
            && !url.is_empty()
            && (url.contains(char::is_whitespace)
                || !reqwest::Url::parse(url)
                    .is_ok_and(|parsed| parsed.scheme() == "https" && parsed.host().is_some()))
        {
            return Err(PayloadError::Url);
        }

        if let Some(url) = url {
            settings.url = url;
        }
        if let Some(site) = self.site {
            settings.site = site.trim().to_compact_string();
        }
        if let Some(delay_days) = self.delay_days {
            settings.delay_days = delay_days;
        }
        if let Some(interval_days) = self.interval_days {
            settings.interval_days = interval_days;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsReviews {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        Ok(serializer
            .write_raw_setting("url", &*self.url)
            .write_raw_setting("site", &*self.site)
            .write_raw_setting("delay_days", self.delay_days.to_compact_string())
            .write_raw_setting("interval_days", self.interval_days.to_compact_string()))
    }
}

pub struct AppSettingsReviewsDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsReviewsDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsReviews::default();

        Ok(Box::new(AppSettingsReviews {
            url: deserializer.take_raw_setting("url").unwrap_or_default(),
            site: deserializer.take_raw_setting("site").unwrap_or_default(),
            delay_days: deserializer
                .take_raw_setting("delay_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.delay_days),
            interval_days: deserializer
                .take_raw_setting("interval_days")
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.interval_days),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> PayloadReviews {
        PayloadReviews {
            url: None,
            site: None,
            delay_days: None,
            interval_days: None,
        }
    }

    fn with_url(url: &str) -> AppSettingsReviews {
        AppSettingsReviews {
            url: url.into(),
            ..AppSettingsReviews::default()
        }
    }

    #[test]
    fn ships_off() {
        let settings = AppSettingsReviews::default();
        assert!(!settings.active());
        assert_eq!(settings.site_name(), "");
    }

    #[test]
    fn the_site_is_named_after_the_url_without_www() {
        assert_eq!(
            with_url("https://www.reviews.example.com/evaluate/panel.example.com").site_name(),
            "reviews.example.com"
        );
        assert_eq!(
            with_url("https://reviews.example.com/write").site_name(),
            "reviews.example.com"
        );
    }

    #[test]
    fn a_configured_site_name_wins() {
        let settings = AppSettingsReviews {
            site: "Example Reviews".into(),
            ..with_url("https://www.reviews.example.com/write")
        };
        assert_eq!(settings.site_name(), "Example Reviews");
    }

    #[test]
    fn a_site_name_alone_turns_nothing_on() {
        let settings = AppSettingsReviews {
            site: "Example Reviews".into(),
            ..AppSettingsReviews::default()
        };
        assert!(!settings.active());
        assert_eq!(settings.site_name(), "");
    }

    #[test]
    fn a_payload_changes_only_what_it_names() {
        let mut settings = with_url("https://reviews.example.com/write");
        PayloadReviews {
            interval_days: Some(0),
            ..payload()
        }
        .apply(&mut settings)
        .unwrap();

        assert_eq!(settings.url, "https://reviews.example.com/write");
        assert_eq!(settings.delay_days, 3);
        assert_eq!(settings.interval_days, 0);
    }

    #[test]
    fn the_url_is_trimmed_and_an_empty_one_turns_reviews_off() {
        let mut settings = AppSettingsReviews::default();
        PayloadReviews {
            url: Some("  https://reviews.example.com/write  ".into()),
            site: Some(" Example Reviews ".into()),
            ..payload()
        }
        .apply(&mut settings)
        .unwrap();
        assert_eq!(settings.url, "https://reviews.example.com/write");
        assert_eq!(settings.site, "Example Reviews");

        PayloadReviews {
            url: Some("   ".into()),
            ..payload()
        }
        .apply(&mut settings)
        .unwrap();
        assert!(!settings.active());
    }

    #[test]
    fn only_https_addresses_are_taken_and_a_refusal_changes_nothing() {
        for url in [
            "http://reviews.example.com/write",
            "javascript:alert(1)",
            "reviews.example.com/write",
            "https://reviews.example.com/write a review",
            "https://",
        ] {
            let mut settings = with_url("https://reviews.example.com/write");
            let refused = PayloadReviews {
                url: Some(url.into()),
                delay_days: Some(10),
                ..payload()
            }
            .apply(&mut settings);

            assert_eq!(refused, Err(PayloadError::Url), "{url}");
            assert_eq!(settings.url, "https://reviews.example.com/write");
            assert_eq!(settings.delay_days, 3);
        }
    }
}
