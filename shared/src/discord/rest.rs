//! plugged.host: the Discord REST calls the panel makes with a bot token: a member, the server's
//! invites, a role on a member, slash commands and interaction replies.

use serde::Deserialize;
use std::time::Duration;

pub const API: &str = "https://discord.com/api/v10";
const TIMEOUT: Duration = Duration::from_secs(10);
/// The longest wait for a rate limit inside one call; a longer one fails the call.
const MAX_WAIT: Duration = Duration::from_secs(10);
/// Discord's error codes for a member or user that does not exist.
const UNKNOWN_MEMBER: i64 = 10007;
const UNKNOWN_USER: i64 = 10013;

#[derive(Deserialize, Clone, Debug, Default)]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

impl User {
    /// The name people see: the display name, else the username.
    pub fn display_name(&self) -> Option<&str> {
        self.global_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(self.username.as_deref())
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Member {
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub premium_since: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub joined_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The member's permissions in the channel, only on an interaction's member.
    #[serde(default)]
    pub permissions: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Invite {
    pub code: String,
    #[serde(default)]
    pub uses: i64,
    #[serde(default)]
    pub inviter: Option<User>,
}

/// What Discord said about someone.
#[derive(Debug)]
pub enum Lookup<T> {
    Found(T),
    /// Discord said there is no such member: the only answer that counts as having left.
    Absent,
    /// No answer that says either way (an error, a rate limit, a wrong server id).
    Unknown,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct RateLimitBody {
    #[serde(default)]
    retry_after: f64,
}

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
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response, anyhow::Error> {
        for _ in 0..3 {
            let mut request = self
                .http
                .request(method.clone(), format!("{API}{path}"))
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

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let header = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<f64>().ok());
                let body = response
                    .json::<RateLimitBody>()
                    .await
                    .map(|body| body.retry_after)
                    .ok();
                let wait =
                    Duration::from_secs_f64(body.or(header).unwrap_or(1.0).clamp(0.0, 3600.0));
                if wait > MAX_WAIT {
                    anyhow::bail!("rate limited by Discord for {}s", wait.as_secs());
                }
                tokio::time::sleep(wait).await;
                continue;
            }

            // the bucket is empty: wait for it here rather than running into the limit next call
            let remaining = response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u32>().ok());
            let reset = response
                .headers()
                .get("x-ratelimit-reset-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<f64>().ok());
            if remaining == Some(0)
                && let Some(reset) = reset
            {
                tokio::time::sleep(Duration::from_secs_f64(
                    reset.clamp(0.0, MAX_WAIT.as_secs_f64()),
                ))
                .await;
            }

            return Ok(response);
        }

        anyhow::bail!("rate limited by Discord")
    }

    async fn expect_success(
        response: reqwest::Response,
    ) -> Result<reqwest::Response, anyhow::Error> {
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let error = response.json::<ErrorBody>().await.ok();
        match error {
            Some(error) => anyhow::bail!(
                "Discord answered {status}: {} ({})",
                error.message,
                error.code
            ),
            None => anyhow::bail!("Discord answered {status}"),
        }
    }

    /// A member of the server.
    pub async fn member(&self, guild_id: &str, user_id: &str) -> Lookup<Member> {
        if !super::is_snowflake(guild_id) || !super::is_snowflake(user_id) {
            return Lookup::Unknown;
        }

        let response = match self
            .send(
                reqwest::Method::GET,
                &format!("/guilds/{guild_id}/members/{user_id}"),
                None,
            )
            .await
        {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(discord = %user_id, "Discord member lookup failed: {err:#}");
                return Lookup::Unknown;
            }
        };

        match response.status() {
            status if status.is_success() => match response.json::<Member>().await {
                Ok(member) => Lookup::Found(member),
                Err(err) => {
                    tracing::warn!(discord = %user_id, "unreadable Discord member: {err:#}");
                    Lookup::Unknown
                }
            },
            reqwest::StatusCode::NOT_FOUND => match response.json::<ErrorBody>().await {
                Ok(error) if error.code == UNKNOWN_MEMBER || error.code == UNKNOWN_USER => {
                    Lookup::Absent
                }
                Ok(error) => {
                    tracing::warn!(
                        discord = %user_id,
                        "Discord member lookup: {} ({})",
                        error.message,
                        error.code
                    );
                    Lookup::Unknown
                }
                Err(_) => Lookup::Unknown,
            },
            status => {
                tracing::warn!(discord = %user_id, "Discord member lookup answered {status}");
                Lookup::Unknown
            }
        }
    }

    /// The server's invites with their uses (the bot needs Manage Server).
    pub async fn invites(&self, guild_id: &str) -> Result<Vec<Invite>, anyhow::Error> {
        let response = self
            .send(
                reqwest::Method::GET,
                &format!("/guilds/{guild_id}/invites"),
                None,
            )
            .await?;

        Ok(Self::expect_success(response).await?.json().await?)
    }

    /// Gives a member a role (`add`) or takes it away.
    pub async fn set_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        add: bool,
    ) -> Result<(), anyhow::Error> {
        let method = if add {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        };
        let response = self
            .send(
                method,
                &format!("/guilds/{guild_id}/members/{user_id}/roles/{role_id}"),
                None,
            )
            .await?;
        Self::expect_success(response).await?;

        Ok(())
    }

    /// Creates or updates one slash command on the server (others, made by anything else on the
    /// same application, stay).
    pub async fn upsert_command(
        &self,
        application_id: &str,
        guild_id: &str,
        command: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let response = self
            .send(
                reqwest::Method::POST,
                &format!("/applications/{application_id}/guilds/{guild_id}/commands"),
                Some(command),
            )
            .await?;
        Self::expect_success(response).await?;

        Ok(())
    }

    /// Answers an interaction: `kind` 4 is a message, 5 "thinking" (answered later with
    /// [`Self::edit_reply`]).
    pub async fn reply(
        &self,
        interaction_id: &str,
        interaction_token: &str,
        kind: u8,
        data: Option<serde_json::Value>,
    ) -> Result<(), anyhow::Error> {
        let mut body = serde_json::json!({ "type": kind });
        if let Some(data) = data {
            body["data"] = data;
        }
        let response = self
            .send(
                reqwest::Method::POST,
                &format!("/interactions/{interaction_id}/{interaction_token}/callback"),
                Some(&body),
            )
            .await?;
        Self::expect_success(response).await?;

        Ok(())
    }

    /// Replaces the answer to an interaction that was answered with "thinking".
    pub async fn edit_reply(
        &self,
        application_id: &str,
        interaction_token: &str,
        data: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let response = self
            .send(
                reqwest::Method::PATCH,
                &format!("/webhooks/{application_id}/{interaction_token}/messages/@original"),
                Some(data),
            )
            .await?;
        Self::expect_success(response).await?;

        Ok(())
    }
}
