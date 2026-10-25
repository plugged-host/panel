//! plugged.host: the few Discord calls neo on Discord makes with the assistant bot's token: typing,
//! posting an answer, and fetching an attached image from Discord's CDN.

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

const API: &str = crate::discord::rest::API;
const TIMEOUT: Duration = Duration::from_secs(10);
/// The longest wait for a rate limit inside one call; a longer one fails the call.
const MAX_WAIT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: String,
}

impl Client {
    pub fn new(http: &reqwest::Client, token: &str) -> Self {
        Self {
            http: http.clone(),
            token: token.to_string(),
        }
    }

    async fn send(
        &self,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(), anyhow::Error> {
        for _ in 0..3 {
            let mut request = self
                .http
                .post(format!("{API}{path}"))
                .timeout(TIMEOUT)
                .header(
                    reqwest::header::USER_AGENT,
                    concat!(
                        "DiscordBot (https://github.com/calagopus/panel, ",
                        env!("CARGO_PKG_VERSION"),
                        ")"
                    ),
                )
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bot {}", self.token),
                );
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await?;
            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let wait = response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|body| body["retry_after"].as_f64())
                    .unwrap_or(1.0)
                    .clamp(0.0, 3600.0);
                let wait = Duration::from_secs_f64(wait);
                if wait > MAX_WAIT {
                    anyhow::bail!("rate limited by Discord for {}s", wait.as_secs());
                }
                tokio::time::sleep(wait).await;
                continue;
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!(
                    "Discord answered {status}: {}",
                    body.chars().take(200).collect::<String>()
                );
            }

            return Ok(());
        }

        anyhow::bail!("rate limited by Discord")
    }

    /// "neo is typing…" in the channel, for about ten seconds.
    pub async fn typing(&self, channel_id: &str) -> Result<(), anyhow::Error> {
        if !crate::discord::is_snowflake(channel_id) {
            anyhow::bail!("not a channel id");
        }

        self.send(&format!("/channels/{channel_id}/typing"), None)
            .await
    }

    /// Posts a message (Discord's message object: content, allowed_mentions, message_reference).
    pub async fn create_message(
        &self,
        channel_id: &str,
        body: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        if !crate::discord::is_snowflake(channel_id) {
            anyhow::bail!("not a channel id");
        }

        self.send(&format!("/channels/{channel_id}/messages"), Some(body))
            .await
    }
}

static DOWNLOADS: OnceLock<reqwest::Client> = OnceLock::new();

/// The client for attachments: no redirects, no internal addresses.
fn downloads(env: &Arc<crate::env::Env>) -> &'static reqwest::Client {
    DOWNLOADS.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("calagopus-panel {}", crate::VERSION))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .no_proxy()
            .dns_resolver(Arc::new(crate::net::BlockedIpResolver::new(
                env,
                "Discord attachment",
            )))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to create the Discord attachment client")
    })
}

/// An attachment's bytes, when it is on Discord's CDN, answers and is no larger than
/// [`super::inbox::MAX_IMAGE_BYTES`]. Discord's links expire after about a day, so this runs when
/// the message is answered.
pub async fn download(env: &Arc<crate::env::Env>, url: &str) -> Option<Vec<u8>> {
    use futures_util::StreamExt;

    if !super::inbox::is_discord_cdn(url) {
        return None;
    }

    let response = downloads(env).get(url).send().await.ok()?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > super::inbox::MAX_IMAGE_BYTES)
    {
        return None;
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.ok()?);
        if bytes.len() as u64 > super::inbox::MAX_IMAGE_BYTES {
            return None;
        }
    }

    Some(bytes)
}
