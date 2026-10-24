// plugged.host: neo's model slots (Admin › Settings › AI › neo): their status and a test call, docs/features/neo.md.

use super::State;
use shared::settings::neo::{NeoProvider, header_is_secret};
use utoipa_axum::router::OpenApiRouter;

mod providers;
mod test;

/// The most of a provider's answer or error the admin sees.
const EXCERPT_CHARS: usize = 300;

/// A provider's text for the admin: on one line, cut to [`EXCERPT_CHARS`], with any Google key,
/// the slot's key and its secret extra header values masked (providers echo them in errors).
fn excerpt(text: &str, provider: &NeoProvider) -> String {
    let mut text = shared::neo::redact_keys(text);
    let secrets = provider.api_key.iter().map(|key| key.as_str()).chain(
        provider
            .extra_headers
            .iter()
            .filter(|(name, _)| header_is_secret(name))
            .map(|(_, value)| value.as_str()),
    );
    for secret in secrets {
        let secret = secret.trim();
        // a short one could be a common word
        if secret.len() >= 8 {
            text = text.replace(secret, "[redacted]");
        }
    }

    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > EXCERPT_CHARS {
        format!(
            "{}…",
            line.chars()
                .take(EXCERPT_CHARS)
                .collect::<String>()
                .trim_end()
        )
    } else {
        line
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/providers", providers::router(state))
        .nest("/test", test::router(state))
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(key: Option<&str>) -> NeoProvider {
        NeoProvider {
            slot: 1,
            enabled: true,
            label: "Model".into(),
            description: Default::default(),
            driver: Default::default(),
            endpoint: "https://api.example.com/v1".into(),
            api_key: key.map(Into::into),
            model: "model".into(),
            max_tokens: 1024,
            timeout: 30,
            daily_token_cap: 0,
            extra_headers: Default::default(),
            extra_body: Default::default(),
            access: Default::default(),
            multiplier: 1,
            vision: None,
        }
    }

    #[test]
    fn excerpts_mask_secrets_and_stay_short() {
        let mut slot = provider(Some("sk-test-12345678"));
        slot.extra_headers.insert(
            "cf-aig-authorization".into(),
            "Bearer gateway-secret".into(),
        );
        slot.extra_headers
            .insert("X-Title".into(), "Panel title".into());

        assert_eq!(
            excerpt(
                "bad key sk-test-12345678 for\n\nthis  model (Bearer gateway-secret)",
                &slot
            ),
            "bad key [redacted] for this model ([redacted])"
        );
        // a header that is no secret stays readable
        assert_eq!(excerpt("from Panel title", &slot), "from Panel title");
        assert_eq!(
            excerpt(
                "key AIzaSyA1234567890abcdefghijklmno rejected",
                &provider(None)
            ),
            "key [redacted] rejected"
        );
        // too short to be a real key: left alone rather than masking common words
        assert_eq!(excerpt("OK", &provider(Some("OK"))), "OK");

        let long = excerpt(&"word ".repeat(200), &provider(None));
        assert!(long.ends_with('…'));
        assert!(long.chars().count() <= EXCERPT_CHARS + 1);
    }
}
