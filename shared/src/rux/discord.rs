//! plugged.host: Discord's REST API (v10) with rux's own token (the Discord section's moderation
//! bot token): messages, DMs, reactions, interactions, members, roles, channels and bans.
//!
//! A 429 is waited out and retried when Discord asks for 10 s or less; anything else that is not
//! a success becomes a [`DiscordError`] whose message the model can pass on. Moderation calls
//! carry the order in `X-Audit-Log-Reason`, so Discord's audit log says who asked rux. A
//! server's channels and roles are cached for a minute, its owner and name for an hour.

use serde_json::{Value, json};
use std::time::Duration;

pub const API: &str = "https://discord.com/api/v10";
const TIMEOUT: Duration = Duration::from_secs(15);
/// The longest wait for a rate limit inside one call.
const MAX_WAIT: f64 = 10.0;
/// Discord keeps audit log reasons to 512 bytes; this leaves room for the encoding.
const MAX_REASON_CHARS: usize = 450;

/// Channel types a message can be posted in: text, announcement.
pub const POSTABLE: [i64; 2] = [0, 5];
/// Thread channel types (announcement, public and private threads).
pub const THREADS: [i64; 3] = [10, 11, 12];

/// Discord answered with an error, or could not be reached.
#[derive(Debug, Clone)]
pub struct DiscordError {
    /// The HTTP status; 0 when Discord could not be reached.
    pub status: u16,
    /// Discord's own error code.
    pub code: Option<i64>,
    /// What Discord said and what usually causes it, for the model (and so the admin).
    pub message: String,
}

impl std::fmt::Display for DiscordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DiscordError {}

impl DiscordError {
    pub fn unreachable(reason: impl std::fmt::Display) -> Self {
        Self {
            status: 0,
            code: None,
            message: format!("Discord could not be reached: {reason}"),
        }
    }

    /// An error answer, with a hint for the codes people run into.
    pub fn from_answer(status: u16, body: &Value) -> Self {
        let code = body["code"].as_i64();
        let said = body["message"]
            .as_str()
            .map(str::trim)
            .filter(|said| !said.is_empty())
            .map_or_else(|| format!("HTTP {status}"), str::to_string);
        let hint = match (code, status) {
            (Some(50013), _) => {
                " rux is missing a permission there, or its role is not above the member's highest role."
            }
            (Some(50001), _) => " rux cannot see that channel.",
            (Some(50007), _) => " They do not accept DMs from rux.",
            (Some(50034), _) => " Messages older than 14 days cannot be bulk deleted.",
            (Some(10007), _) => " That member is not in the server.",
            (Some(10013), _) => " There is no such Discord user.",
            (Some(10026), _) => " That user is not banned.",
            (Some(10003), _) => " There is no such channel.",
            (Some(10011), _) => " There is no such role.",
            (Some(10008), _) => " That message no longer exists.",
            (_, 401) => " The rux bot token is invalid.",
            _ => "",
        };

        Self {
            status,
            code,
            message: match code {
                Some(code) => format!("Discord refused: {said} ({code}).{hint}"),
                None => format!("Discord refused: {said}.{hint}"),
            },
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.status == 404
    }
}

/// `reason` as Discord's audit log header takes it.
pub fn audit_reason(reason: &str) -> String {
    let reason: String = reason.trim().chars().take(MAX_REASON_CHARS).collect();

    urlencoding::encode(&reason).into_owned()
}

/// Whether `path` names one endpoint as written. Ids in paths can come from what the model wrote,
/// so a segment that is empty, `.` or `..` (also percent-encoded, which URLs treat the same), or a
/// query, fragment or backslash, is refused before it can point the call at another endpoint.
pub fn safe_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if path.contains(['?', '#', '\\'])
        || lower.contains("%2e")
        || lower.contains("%2f")
        || lower.contains("%5c")
    {
        return false;
    }

    path.trim_start_matches('/')
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// Discord's REST API with rux's token.
#[derive(Clone)]
pub struct Rest {
    state: crate::State,
    token: String,
}

impl Rest {
    pub fn new(state: &crate::State, token: &str) -> Self {
        Self {
            state: state.clone(),
            token: token.trim().to_string(),
        }
    }

    /// With the Discord section's moderation bot token; `None` while there is none.
    pub async fn load(state: &crate::State) -> Result<Option<Self>, anyhow::Error> {
        let token = {
            let settings = state.settings.get().await?;
            settings
                .discord
                .moderation_bot_token
                .as_deref()
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_string)
        };

        Ok(token.map(|token| Self::new(state, &token)))
    }

    /// One call; `reason` goes in the audit log.
    pub async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
        reason: Option<&str>,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response, DiscordError> {
        if !safe_path(path) {
            return Err(DiscordError {
                status: 400,
                code: None,
                message: "That is not a valid Discord id.".into(),
            });
        }
        let url = format!("{API}/{}", path.trim_start_matches('/'));

        for attempt in 1..=3 {
            let mut request = self
                .state
                .client
                .request(method.clone(), &url)
                .timeout(TIMEOUT)
                .header(
                    reqwest::header::USER_AGENT,
                    concat!(
                        "DiscordBot (https://github.com/calagopus/panel, ",
                        env!("CARGO_PKG_VERSION"),
                        ") rux"
                    ),
                )
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bot {}", self.token),
                );
            if let Some(reason) = reason.filter(|reason| !reason.trim().is_empty()) {
                request = request.header("X-Audit-Log-Reason", audit_reason(reason));
            }
            if !query.is_empty() {
                request = request.query(query);
            }
            if let Some(body) = body {
                request = request.json(body);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(err) if attempt < 2 => {
                    tracing::debug!("rux: Discord call failed, trying again: {err}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(err) => return Err(DiscordError::unreachable(err)),
            };
            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                let header = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<f64>().ok());
                let body = response.json::<Value>().await.unwrap_or(Value::Null);
                let wait = body["retry_after"].as_f64().or(header).unwrap_or(1.0);
                if wait <= MAX_WAIT {
                    tokio::time::sleep(Duration::from_secs_f64(wait.max(0.05))).await;
                    continue;
                }

                return Err(DiscordError::from_answer(status.as_u16(), &body));
            }
            if status.is_server_error() && attempt < 2 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            if !status.is_success() {
                let body = response.json::<Value>().await.unwrap_or(Value::Null);
                return Err(DiscordError::from_answer(status.as_u16(), &body));
            }

            return Ok(response);
        }

        Err(DiscordError::unreachable("rate limited"))
    }

    /// One call and its JSON answer (`null` for an empty one).
    pub async fn json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
        reason: Option<&str>,
        query: &[(&str, String)],
    ) -> Result<Value, DiscordError> {
        let response = self.call(method, path, body, reason, query).await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }

        Ok(response.json::<Value>().await.unwrap_or(Value::Null))
    }

    async fn optional(&self, path: &str) -> Result<Option<Value>, DiscordError> {
        match self.json(reqwest::Method::GET, path, None, None, &[]).await {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.is_not_found() => Ok(None),
            Err(err) => Err(err),
        }
    }

    async fn list(&self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>, DiscordError> {
        Ok(
            match self
                .json(reqwest::Method::GET, path, None, None, query)
                .await?
            {
                Value::Array(items) => items,
                _ => Vec::new(),
            },
        )
    }

    /// A list kept for `ttl` seconds under `key`.
    async fn cached_list(
        &self,
        key: &str,
        ttl: u64,
        fresh: bool,
        path: &str,
    ) -> Result<Vec<Value>, DiscordError> {
        if !fresh
            && let Ok(Some(cached)) = self.state.cache.get::<String>(key).await
            && let Ok(items) = serde_json::from_str::<Vec<Value>>(&cached)
        {
            return Ok(items);
        }

        let items = self.list(path, &[]).await?;
        if let Ok(json) = serde_json::to_string(&items) {
            let _ = self.state.cache.set(key, ttl, &json).await;
        }

        Ok(items)
    }

    // --- messages -------------------------------------------------------------------------

    pub async fn create_message(
        &self,
        channel_id: &str,
        payload: &Value,
    ) -> Result<Value, DiscordError> {
        self.json(
            reqwest::Method::POST,
            &format!("channels/{channel_id}/messages"),
            Some(payload),
            None,
            &[],
        )
        .await
    }

    pub async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        payload: &Value,
    ) -> Result<Value, DiscordError> {
        self.json(
            reqwest::Method::PATCH,
            &format!("channels/{channel_id}/messages/{message_id}"),
            Some(payload),
            None,
            &[],
        )
        .await
    }

    /// Deletes 1 to 100 messages of a channel (more than one: none older than 14 days).
    pub async fn delete_messages(
        &self,
        channel_id: &str,
        message_ids: &[String],
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        match message_ids {
            [] => Ok(()),
            [id] => self
                .call(
                    reqwest::Method::DELETE,
                    &format!("channels/{channel_id}/messages/{id}"),
                    None,
                    reason,
                    &[],
                )
                .await
                .map(|_| ()),
            ids => self
                .call(
                    reqwest::Method::POST,
                    &format!("channels/{channel_id}/messages/bulk-delete"),
                    Some(&json!({ "messages": ids })),
                    reason,
                    &[],
                )
                .await
                .map(|_| ()),
        }
    }

    /// "rux is typing…"; only a courtesy, so it never fails.
    pub async fn typing(&self, channel_id: &str) {
        if let Err(err) = self
            .call(
                reqwest::Method::POST,
                &format!("channels/{channel_id}/typing"),
                None,
                None,
                &[],
            )
            .await
        {
            tracing::debug!("rux: typing: {err}");
        }
    }

    /// Adds rux's reaction (`emoji`: a unicode emoji, or `name:id` for a custom one).
    pub async fn react(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::PUT,
            &format!(
                "channels/{channel_id}/messages/{message_id}/reactions/{}/@me",
                urlencoding::encode(emoji)
            ),
            None,
            None,
            &[],
        )
        .await
        .map(|_| ())
    }

    pub async fn unreact(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::DELETE,
            &format!(
                "channels/{channel_id}/messages/{message_id}/reactions/{}/@me",
                urlencoding::encode(emoji)
            ),
            None,
            None,
            &[],
        )
        .await
        .map(|_| ())
    }

    /// The DM channel with a user (kept a day).
    pub async fn dm_channel(&self, user_id: &str) -> Result<String, DiscordError> {
        let key = format!("rux::discord::dm::{user_id}");
        if let Ok(Some(channel)) = self.state.cache.get::<String>(&key).await {
            return Ok(channel);
        }

        let channel = self
            .json(
                reqwest::Method::POST,
                "users/@me/channels",
                Some(&json!({ "recipient_id": user_id })),
                None,
                &[],
            )
            .await?;
        let Some(id) = channel["id"].as_str().map(str::to_string) else {
            return Err(DiscordError::unreachable("Discord gave no DM channel"));
        };
        let _ = self.state.cache.set(&key, 86400, &id).await;

        Ok(id)
    }

    /// A DM, if the user accepts them. Never fails: a closed DM must not stop the action it
    /// tells them about.
    pub async fn send_dm(&self, user_id: &str, payload: &Value) -> bool {
        let channel = match self.dm_channel(user_id).await {
            Ok(channel) => channel,
            Err(err) => {
                tracing::debug!("rux: no DM channel for {user_id}: {err}");
                return false;
            }
        };

        match self.create_message(&channel, payload).await {
            Ok(_) => true,
            Err(err) => {
                tracing::debug!("rux: DM to {user_id} refused: {err}");
                false
            }
        }
    }

    /// Answers an interaction (`type` 4 a message, 5 "thinking", 6 "seen", 7 an update of the
    /// message the button is on).
    pub async fn interaction_callback(
        &self,
        interaction_id: &str,
        interaction_token: &str,
        payload: &Value,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::POST,
            &format!("interactions/{interaction_id}/{interaction_token}/callback"),
            Some(payload),
            None,
            &[],
        )
        .await
        .map(|_| ())
    }

    /// Replaces the answer to an interaction that was answered with "thinking".
    pub async fn edit_interaction_reply(
        &self,
        application_id: &str,
        interaction_token: &str,
        payload: &Value,
    ) -> Result<Value, DiscordError> {
        self.json(
            reqwest::Method::PATCH,
            &format!("webhooks/{application_id}/{interaction_token}/messages/@original"),
            Some(payload),
            None,
            &[],
        )
        .await
    }

    pub async fn messages(
        &self,
        channel_id: &str,
        query: &[(&str, String)],
    ) -> Result<Vec<Value>, DiscordError> {
        self.list(&format!("channels/{channel_id}/messages"), query)
            .await
    }

    /// A message; `None` when it does not exist (any more).
    pub async fn message(
        &self,
        channel_id: &str,
        message_id: &str,
    ) -> Result<Option<Value>, DiscordError> {
        self.optional(&format!("channels/{channel_id}/messages/{message_id}"))
            .await
    }

    pub async fn pin(
        &self,
        channel_id: &str,
        message_id: &str,
        pinned: bool,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        let method = if pinned {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        };

        self.call(
            method,
            &format!("channels/{channel_id}/messages/pins/{message_id}"),
            None,
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    // --- the server -------------------------------------------------------------------------

    /// A member of the server; `None` when they are not in it.
    pub async fn member(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<Value>, DiscordError> {
        if !crate::discord::is_snowflake(guild_id) || !crate::discord::is_snowflake(user_id) {
            return Ok(None);
        }

        self.optional(&format!("guilds/{guild_id}/members/{user_id}"))
            .await
    }

    /// A member's role ids; `None` when they are not in the server.
    pub async fn member_roles(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<String>>, DiscordError> {
        Ok(self.member(guild_id, user_id).await?.map(|member| {
            member["roles"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|role| role.as_str().map(str::to_string))
                .collect()
        }))
    }

    pub async fn search_members(
        &self,
        guild_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Value>, DiscordError> {
        self.list(
            &format!("guilds/{guild_id}/members/search"),
            &[
                ("query", query.to_string()),
                ("limit", limit.clamp(1, 1000).to_string()),
            ],
        )
        .await
    }

    /// Any Discord user; `None` when there is no such user.
    pub async fn user(&self, user_id: &str) -> Result<Option<Value>, DiscordError> {
        if !crate::discord::is_snowflake(user_id) {
            return Ok(None);
        }

        self.optional(&format!("users/{user_id}")).await
    }

    /// The server itself (its owner and name), kept an hour.
    pub async fn guild(&self, guild_id: &str) -> Result<Value, DiscordError> {
        let key = format!("rux::discord::guild::{guild_id}");
        if let Ok(Some(cached)) = self.state.cache.get::<String>(&key).await
            && let Ok(guild) = serde_json::from_str::<Value>(&cached)
        {
            return Ok(guild);
        }

        let guild = self
            .json(
                reqwest::Method::GET,
                &format!("guilds/{guild_id}"),
                None,
                None,
                &[],
            )
            .await?;
        let kept = json!({
            "id": guild["id"],
            "name": guild["name"],
            "owner_id": guild["owner_id"],
        });
        if let Ok(json) = serde_json::to_string(&kept) {
            let _ = self.state.cache.set(&key, 3600, &json).await;
        }

        Ok(kept)
    }

    /// The server's channels (threads not included), kept a minute.
    pub async fn channels(&self, guild_id: &str, fresh: bool) -> Result<Vec<Value>, DiscordError> {
        self.cached_list(
            &format!("rux::discord::channels::{guild_id}"),
            60,
            fresh,
            &format!("guilds/{guild_id}/channels"),
        )
        .await
    }

    /// The server's roles, kept a minute.
    pub async fn roles(&self, guild_id: &str, fresh: bool) -> Result<Vec<Value>, DiscordError> {
        self.cached_list(
            &format!("rux::discord::roles::{guild_id}"),
            60,
            fresh,
            &format!("guilds/{guild_id}/roles"),
        )
        .await
    }

    pub async fn channel(&self, channel_id: &str) -> Result<Value, DiscordError> {
        self.json(
            reqwest::Method::GET,
            &format!("channels/{channel_id}"),
            None,
            None,
            &[],
        )
        .await
    }

    pub async fn modify_channel(
        &self,
        channel_id: &str,
        changes: &Value,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::PATCH,
            &format!("channels/{channel_id}"),
            Some(changes),
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    /// `kind` 0 a role, 1 a member; `allow` and `deny` as decimal strings.
    pub async fn set_permission_overwrite(
        &self,
        channel_id: &str,
        overwrite_id: &str,
        kind: u8,
        allow: &str,
        deny: &str,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::PUT,
            &format!("channels/{channel_id}/permissions/{overwrite_id}"),
            Some(&json!({ "type": kind, "allow": allow, "deny": deny })),
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    /// Changes a member: `nick`, `communication_disabled_until` (a timeout), roles.
    pub async fn modify_member(
        &self,
        guild_id: &str,
        user_id: &str,
        changes: &Value,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::PATCH,
            &format!("guilds/{guild_id}/members/{user_id}"),
            Some(changes),
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    pub async fn kick(
        &self,
        guild_id: &str,
        user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::DELETE,
            &format!("guilds/{guild_id}/members/{user_id}"),
            None,
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    pub async fn ban(
        &self,
        guild_id: &str,
        user_id: &str,
        delete_message_seconds: u32,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::PUT,
            &format!("guilds/{guild_id}/bans/{user_id}"),
            Some(&json!({ "delete_message_seconds": delete_message_seconds.min(604_800) })),
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    pub async fn unban(
        &self,
        guild_id: &str,
        user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        self.call(
            reqwest::Method::DELETE,
            &format!("guilds/{guild_id}/bans/{user_id}"),
            None,
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }

    /// The server's bans, up to `max` of them.
    pub async fn bans(&self, guild_id: &str, max: usize) -> Result<Vec<Value>, DiscordError> {
        let mut bans: Vec<Value> = Vec::new();
        let mut after: Option<String> = None;

        while bans.len() < max {
            let mut query = vec![("limit", "1000".to_string())];
            if let Some(after) = &after {
                query.push(("after", after.clone()));
            }
            let page = self
                .list(&format!("guilds/{guild_id}/bans"), &query)
                .await?;
            let full = page.len() >= 1000;
            after = page
                .last()
                .and_then(|ban| ban["user"]["id"].as_str())
                .map(str::to_string);
            bans.extend(page);
            if !full || after.is_none() {
                break;
            }
        }
        bans.truncate(max);

        Ok(bans)
    }

    pub async fn modify_role(
        &self,
        guild_id: &str,
        role_id: &str,
        changes: &Value,
        reason: Option<&str>,
    ) -> Result<Value, DiscordError> {
        let role = self
            .json(
                reqwest::Method::PATCH,
                &format!("guilds/{guild_id}/roles/{role_id}"),
                Some(changes),
                reason,
                &[],
            )
            .await?;
        let _ = self
            .state
            .cache
            .invalidate(&format!("rux::discord::roles::{guild_id}"))
            .await;

        Ok(role)
    }

    /// Gives a member a role (`add`) or takes it away.
    pub async fn set_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        add: bool,
        reason: Option<&str>,
    ) -> Result<(), DiscordError> {
        let method = if add {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        };

        self.call(
            method,
            &format!("guilds/{guild_id}/members/{user_id}/roles/{role_id}"),
            None,
            reason,
            &[],
        )
        .await
        .map(|_| ())
    }
}

/// The name a member goes by: their nickname, display name or username.
pub fn member_name(member: &Value) -> String {
    [
        &member["nick"],
        &member["user"]["global_name"],
        &member["user"]["username"],
        &member["user"]["id"],
    ]
    .into_iter()
    .filter_map(|value| value.as_str())
    .find(|name| !name.is_empty())
    .unwrap_or("unknown")
    .to_string()
}

/// A user's display name, else their username.
pub fn user_name(user: &Value) -> String {
    [&user["global_name"], &user["username"], &user["id"]]
        .into_iter()
        .filter_map(|value| value.as_str())
        .find(|name| !name.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_say_what_usually_causes_them() {
        let error = DiscordError::from_answer(
            403,
            &json!({"code": 50013, "message": "Missing Permissions"}),
        );
        assert_eq!(error.code, Some(50013));
        assert!(
            error
                .message
                .starts_with("Discord refused: Missing Permissions (50013).")
        );
        assert!(error.message.contains("missing a permission"));

        let error = DiscordError::from_answer(401, &Value::Null);
        assert_eq!(
            error.message,
            "Discord refused: HTTP 401. The rux bot token is invalid."
        );

        assert!(DiscordError::from_answer(404, &json!({"code": 10008})).is_not_found());
    }

    #[test]
    fn paths_reach_only_the_endpoint_they_name() {
        assert!(safe_path("channels/100000000000000001/messages"));
        assert!(safe_path(&format!(
            "channels/1/messages/2/reactions/{}/@me",
            urlencoding::encode("👍")
        )));
        assert!(!safe_path("guilds/1/members/../../channels/2"));
        assert!(!safe_path("guilds/1/members/%2E%2E/x"));
        assert!(!safe_path("guilds/1/members/2?with_counts=true"));
        assert!(!safe_path("guilds/1/members/"));
        assert!(!safe_path("guilds/1/members/a%2fb"));
        assert!(!safe_path("guilds/1\\members"));
    }

    #[test]
    fn audit_reasons_are_encoded_and_cut() {
        assert_eq!(
            audit_reason(" rux for Ann: spam "),
            "rux%20for%20Ann%3A%20spam"
        );
        assert_eq!(
            urlencoding::decode(&audit_reason(&"é".repeat(600)))
                .unwrap()
                .chars()
                .count(),
            MAX_REASON_CHARS
        );
    }

    #[test]
    fn names() {
        assert_eq!(
            member_name(
                &json!({"nick": null, "user": {"id": "1", "username": "ann", "global_name": "Ann"}})
            ),
            "Ann"
        );
        assert_eq!(
            member_name(&json!({"nick": "A", "user": {"username": "ann"}})),
            "A"
        );
        assert_eq!(user_name(&json!({"id": "1"})), "1");
        assert_eq!(user_name(&Value::Null), "unknown");
    }
}
