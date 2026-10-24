//! plugged.host: neo's chats as stored (tables `neo_conversations`, `neo_messages`, `neo_images`,
//! `user_neo_settings`) and the person's own switches.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// What a conversation is doing.
pub mod status {
    pub const IDLE: &str = "idle";
    pub const RUNNING: &str = "running";
    pub const AWAITING_APPROVAL: &str = "awaiting_approval";
    pub const LIMITED: &str = "limited";
    pub const UNLINKED: &str = "unlinked";
    pub const ERROR: &str = "error";
}

/// Who wrote a message.
pub mod role {
    pub const USER: &str = "user";
    pub const ASSISTANT: &str = "assistant";
    pub const TOOL: &str = "tool";
    /// A line from the panel itself (a limit reached, neo unavailable); never sent to a model.
    pub const NOTICE: &str = "notice";
}

/// Where a tool call stands.
pub mod tool_status {
    pub const PENDING_APPROVAL: &str = "pending_approval";
    pub const APPROVED: &str = "approved";
    pub const QUEUED: &str = "queued";
    pub const EXECUTING: &str = "executing";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    pub const DENIED: &str = "denied";
    pub const SKIPPED: &str = "skipped";

    /// Still to run or to decide.
    pub const WAITING: &[&str] = &[PENDING_APPROVAL, QUEUED, APPROVED];
}

#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, sqlx::FromRow)]
pub struct Conversation {
    pub uuid: uuid::Uuid,
    pub user_uuid: uuid::Uuid,
    pub server_uuid: Option<uuid::Uuid>,
    pub title: Option<String>,
    pub status: String,
    pub step_count: i16,
    pub tokens_used: i64,
    pub last_provider: Option<String>,
    pub last_error: Option<String>,
    pub last_activity: chrono::NaiveDateTime,
    pub archived: Option<chrono::NaiveDateTime>,
    pub created: chrono::NaiveDateTime,
}

const CONVERSATION_COLUMNS: &str = "uuid, user_uuid, server_uuid, title, status, step_count, tokens_used, last_provider, last_error, last_activity, archived, created";

#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, sqlx::FromRow)]
pub struct Message {
    pub uuid: uuid::Uuid,
    pub conversation_uuid: uuid::Uuid,
    pub seq: i32,
    pub role: String,
    pub content: Option<String>,
    #[schema(value_type = Option<Vec<Object>>)]
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub meta: Option<Value>,
    pub created: chrono::NaiveDateTime,
}

const MESSAGE_COLUMNS: &str = "uuid, conversation_uuid, seq, role, content, tool_calls, tool_call_id, tool_name, status, meta, created";

impl Message {
    /// A tool row's arguments, as the model gave them.
    pub fn arguments(&self) -> Value {
        self.meta
            .as_ref()
            .and_then(|meta| meta.get("arguments"))
            .filter(|arguments| arguments.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()))
    }

    /// The uuids of the images a user message came with.
    pub fn image_uuids(&self) -> Vec<uuid::Uuid> {
        self.meta
            .as_ref()
            .and_then(|meta| meta.get("images"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|image| image.as_str().and_then(|uuid| uuid.parse().ok()))
            .collect()
    }
}

/// What a new message carries besides its role.
#[derive(Default)]
pub struct NewMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: Option<&'static str>,
    pub meta: Option<Value>,
}

impl Conversation {
    /// The open conversation for this user in this place (a server, or the account when
    /// `server_uuid` is `None`), created on first use.
    pub async fn open_for(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        server_uuid: Option<uuid::Uuid>,
    ) -> Result<Self, sqlx::Error> {
        if let Some(conversation) = sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {CONVERSATION_COLUMNS} FROM neo_conversations
            WHERE user_uuid = $1 AND server_uuid IS NOT DISTINCT FROM $2 AND archived IS NULL
            ORDER BY created DESC LIMIT 1"
        )))
        .bind(user_uuid)
        .bind(server_uuid)
        .fetch_optional(database.write())
        .await?
        {
            return Ok(conversation);
        }

        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "INSERT INTO neo_conversations (user_uuid, server_uuid) VALUES ($1, $2)
            RETURNING {CONVERSATION_COLUMNS}"
        )))
        .bind(user_uuid)
        .bind(server_uuid)
        .fetch_one(database.write())
        .await
    }

    /// One of the user's conversations in this place.
    pub async fn own(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
        user_uuid: uuid::Uuid,
        server_uuid: Option<uuid::Uuid>,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {CONVERSATION_COLUMNS} FROM neo_conversations
            WHERE uuid = $1 AND user_uuid = $2 AND server_uuid IS NOT DISTINCT FROM $3"
        )))
        .bind(uuid)
        .bind(user_uuid)
        .bind(server_uuid)
        .fetch_optional(database.write())
        .await
    }

    /// Read again (the primary: steps write as they go).
    pub async fn refresh(&self, database: &crate::database::Database) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {CONVERSATION_COLUMNS} FROM neo_conversations WHERE uuid = $1"
        )))
        .bind(self.uuid)
        .fetch_one(database.write())
        .await
    }

    /// This place's earlier chats with at least one user message, newest first, with how many
    /// messages the user wrote in each.
    pub async fn history(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        server_uuid: Option<uuid::Uuid>,
    ) -> Result<Vec<(Self, i64)>, sqlx::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {}, counts.user_messages FROM neo_conversations
            JOIN (
                SELECT conversation_uuid, COUNT(*) AS user_messages FROM neo_messages
                WHERE role = 'user' GROUP BY conversation_uuid
            ) counts ON counts.conversation_uuid = neo_conversations.uuid
            WHERE user_uuid = $1 AND server_uuid IS NOT DISTINCT FROM $2
            ORDER BY last_activity DESC LIMIT 50",
            CONVERSATION_COLUMNS
                .split(", ")
                .map(|column| format!("neo_conversations.{column}"))
                .collect::<Vec<_>>()
                .join(", ")
        )))
        .bind(user_uuid)
        .bind(server_uuid)
        .fetch_all(database.read())
        .await?;

        rows.into_iter()
            .map(|row| {
                use sqlx::{FromRow, Row};

                Ok((Self::from_row(&row)?, row.try_get("user_messages")?))
            })
            .collect()
    }

    pub async fn archive(&self, database: &crate::database::Database) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE neo_conversations SET archived = NOW() WHERE uuid = $1")
            .bind(self.uuid)
            .execute(database.write())
            .await?;

        Ok(())
    }

    /// Brought back from the history as the open conversation.
    pub async fn unarchive(&self, database: &crate::database::Database) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE neo_conversations SET archived = NULL, last_activity = NOW() WHERE uuid = $1",
        )
        .bind(self.uuid)
        .execute(database.write())
        .await?;

        Ok(())
    }

    pub async fn delete(&self, database: &crate::database::Database) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM neo_conversations WHERE uuid = $1")
            .bind(self.uuid)
            .execute(database.write())
            .await?;

        Ok(())
    }

    /// Every chat of the user, everywhere.
    pub async fn delete_all(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<u64, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM neo_conversations WHERE user_uuid = $1")
                .bind(user_uuid)
                .execute(database.write())
                .await?
                .rows_affected(),
        )
    }

    /// Chats untouched for `days` days (the retention setting).
    pub async fn prune(
        database: &crate::database::Database,
        days: i32,
    ) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query(
            "DELETE FROM neo_conversations WHERE last_activity < NOW() - make_interval(days => $1)",
        )
        .bind(days.max(1))
        .execute(database.write())
        .await?
        .rows_affected())
    }

    pub async fn set_status(
        &mut self,
        database: &crate::database::Database,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE neo_conversations SET status = $2, last_activity = NOW() WHERE uuid = $1",
        )
        .bind(self.uuid)
        .bind(status)
        .execute(database.write())
        .await?;
        self.status = status.to_string();

        Ok(())
    }

    pub async fn has_messages(
        &self,
        database: &crate::database::Database,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM neo_messages WHERE conversation_uuid = $1)",
        )
        .bind(self.uuid)
        .fetch_one(database.write())
        .await
    }

    /// Every message, oldest first.
    pub async fn messages(
        &self,
        database: &crate::database::Database,
    ) -> Result<Vec<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM neo_messages WHERE conversation_uuid = $1 ORDER BY seq"
        )))
        .bind(self.uuid)
        .fetch_all(database.write())
        .await
    }

    /// The newest `limit` messages, oldest first (what the chat shows).
    pub async fn recent_messages(
        &self,
        database: &crate::database::Database,
        limit: i64,
    ) -> Result<Vec<Message>, sqlx::Error> {
        let mut messages = sqlx::query_as::<_, Message>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM neo_messages WHERE conversation_uuid = $1
            ORDER BY seq DESC LIMIT $2"
        )))
        .bind(self.uuid)
        .bind(limit)
        .fetch_all(database.write())
        .await?;
        messages.reverse();

        Ok(messages)
    }

    /// The tool calls still waiting, oldest first; `statuses` narrows them.
    pub async fn waiting_tools(
        &self,
        database: &crate::database::Database,
        statuses: &[&str],
    ) -> Result<Vec<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM neo_messages
            WHERE conversation_uuid = $1 AND role = 'tool' AND status = ANY($2)
            ORDER BY seq"
        )))
        .bind(self.uuid)
        .bind(statuses)
        .fetch_all(database.write())
        .await
    }

    /// Answers every call still waiting (providers reject a transcript with an unanswered call).
    pub async fn skip_waiting(
        &self,
        database: &crate::database::Database,
        reason: &str,
    ) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query(
            "UPDATE neo_messages SET status = 'skipped', content = $2
            WHERE conversation_uuid = $1 AND role = 'tool' AND status = ANY($3)",
        )
        .bind(self.uuid)
        .bind(reason)
        .bind(tool_status::WAITING)
        .execute(database.write())
        .await?
        .rows_affected())
    }

    /// Appends a message after the last one.
    pub async fn append(
        &self,
        executor: impl sqlx::PgExecutor<'_>,
        role: &str,
        message: NewMessage,
    ) -> Result<Message, sqlx::Error> {
        sqlx::query_as::<_, Message>(sqlx::AssertSqlSafe(format!(
            "INSERT INTO neo_messages
                (conversation_uuid, seq, role, content, tool_calls, tool_call_id, tool_name, status, meta)
            SELECT $1, COALESCE(MAX(seq), 0) + 1, $2, $3, $4, $5, $6, $7, $8
            FROM neo_messages WHERE conversation_uuid = $1
            RETURNING {MESSAGE_COLUMNS}"
        )))
        .bind(self.uuid)
        .bind(role)
        .bind(message.content)
        .bind(message.tool_calls)
        .bind(message.tool_call_id)
        .bind(message.tool_name)
        .bind(message.status)
        .bind(message.meta)
        .fetch_one(executor)
        .await
    }
}

impl Message {
    /// Writes the outcome of a tool row (or a decision on it).
    pub async fn update_tool(
        &mut self,
        executor: impl sqlx::PgExecutor<'_>,
        status: &str,
        content: Option<String>,
        meta: Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE neo_messages SET status = $2, content = COALESCE($3, content), meta = $4
            WHERE uuid = $1",
        )
        .bind(self.uuid)
        .bind(status)
        .bind(&content)
        .bind(&meta)
        .execute(executor)
        .await?;
        self.status = Some(status.to_string());
        if content.is_some() {
            self.content = content;
        }
        self.meta = Some(meta);

        Ok(())
    }

    /// `meta` with `fields` merged in.
    pub fn meta_with(&self, fields: Value) -> Value {
        let mut meta = match self.meta.clone() {
            Some(Value::Object(meta)) => meta,
            _ => Default::default(),
        };
        if let Value::Object(fields) = fields {
            meta.extend(fields);
        }

        Value::Object(meta)
    }
}

/// A person's neo switches. A missing row reads as the defaults: on, asks first, the default
/// model, not yet acknowledged.
#[derive(ToSchema, Serialize, Deserialize, Clone, Debug, sqlx::FromRow)]
pub struct Preferences {
    pub enabled: bool,
    pub auto_approve: bool,
    /// The model picked in the chat (its key), `None` for the default model.
    pub model: Option<String>,
    pub acknowledged: Option<chrono::NaiveDateTime>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_approve: false,
            model: None,
            acknowledged: None,
        }
    }
}

/// One change to a person's switches.
pub enum PreferenceChange {
    Enabled(bool),
    AutoApprove(bool),
    /// `None` goes back to the default model.
    Model(Option<String>),
    Acknowledged,
}

impl Preferences {
    pub async fn of(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<Self, sqlx::Error> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT enabled, auto_approve, model, acknowledged FROM user_neo_settings WHERE user_uuid = $1",
        )
        .bind(user_uuid)
        .fetch_optional(database.write())
        .await?
        .unwrap_or_default())
    }

    pub async fn change(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        change: PreferenceChange,
    ) -> Result<Self, sqlx::Error> {
        let query = match change {
            PreferenceChange::Enabled(enabled) => sqlx::query(
                "INSERT INTO user_neo_settings (user_uuid, enabled) VALUES ($1, $2)
                ON CONFLICT (user_uuid) DO UPDATE SET enabled = EXCLUDED.enabled",
            )
            .bind(user_uuid)
            .bind(enabled),
            PreferenceChange::AutoApprove(auto_approve) => sqlx::query(
                "INSERT INTO user_neo_settings (user_uuid, auto_approve) VALUES ($1, $2)
                ON CONFLICT (user_uuid) DO UPDATE SET auto_approve = EXCLUDED.auto_approve",
            )
            .bind(user_uuid)
            .bind(auto_approve),
            PreferenceChange::Model(model) => sqlx::query(
                "INSERT INTO user_neo_settings (user_uuid, model) VALUES ($1, $2)
                ON CONFLICT (user_uuid) DO UPDATE SET model = EXCLUDED.model",
            )
            .bind(user_uuid)
            .bind(model),
            PreferenceChange::Acknowledged => sqlx::query(
                "INSERT INTO user_neo_settings (user_uuid, acknowledged) VALUES ($1, NOW())
                ON CONFLICT (user_uuid) DO UPDATE
                SET acknowledged = COALESCE(user_neo_settings.acknowledged, NOW())",
            )
            .bind(user_uuid),
        };
        query.execute(database.write()).await?;

        Self::of(database, user_uuid).await
    }
}

/// The longest tool output a chat shows (the model reads up to its own limit).
const SHOWN_TOOL_OUTPUT: usize = 2000;

/// A message as the chat shows it.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiNeoMessage {
    pub uuid: uuid::Uuid,
    pub seq: i32,
    /// user, assistant, tool or notice.
    pub role: String,
    /// The text; for a tool, its output (shortened); for a notice, its code.
    pub content: Option<String>,
    pub tool: Option<String>,
    /// A tool's status (pending_approval, queued, approved, executing, done, failed, denied,
    /// skipped).
    pub status: Option<String>,
    /// What a tool call does, in a line.
    pub summary: Option<String>,
    /// read, write, destructive or critical.
    pub risk: Option<String>,
    /// How it was approved: auto, user, denied.
    pub approval: Option<String>,
    /// What a content-carrying call writes or sends (redacted).
    pub preview: Option<String>,
    /// What a finished call did, in a line.
    pub display: Option<String>,
    pub images: Vec<uuid::Uuid>,
    /// What the model thought before answering.
    pub thinking: Option<String>,
    /// The model that answered.
    pub model: Option<String>,
    /// A notice's code, for the chat to say in the person's language.
    pub code: Option<String>,
    /// Values for the notice's text (`retry_at`, `model`, `access`).
    #[schema(value_type = Object)]
    pub params: Value,
    pub created: chrono::DateTime<chrono::Utc>,
}

/// A conversation as the chat shows it.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiNeoConversation {
    pub uuid: uuid::Uuid,
    pub server_uuid: Option<uuid::Uuid>,
    pub title: Option<String>,
    /// idle, running, awaiting_approval, limited, unlinked or error.
    pub status: String,
    pub archived: bool,
    pub messages: Vec<ApiNeoMessage>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub created: chrono::DateTime<chrono::Utc>,
}

/// An earlier chat in the history list.
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct ApiNeoHistoryEntry {
    pub uuid: uuid::Uuid,
    pub title: Option<String>,
    pub archived: bool,
    pub user_messages: i64,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn into_api(self) -> ApiNeoMessage {
        let meta = self.meta.unwrap_or(Value::Null);
        let text = |key: &str| meta.get(key).and_then(Value::as_str).map(str::to_string);
        let images = meta
            .get("images")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|image| image.as_str().and_then(|uuid| uuid.parse().ok()))
            .collect();

        let (content, code, params) = if self.role == role::NOTICE {
            let mut params = meta.clone();
            if let Some(params) = params.as_object_mut() {
                params.remove("kind");
                params.remove("code");
            }

            (None, text("code").or(self.content), params)
        } else if self.role == role::TOOL {
            let content = self.content.map(|content| {
                if content.chars().count() > SHOWN_TOOL_OUTPUT {
                    format!(
                        "{}…",
                        content.chars().take(SHOWN_TOOL_OUTPUT).collect::<String>()
                    )
                } else {
                    content
                }
            });

            (content, None, Value::Object(Default::default()))
        } else {
            (self.content, None, Value::Object(Default::default()))
        };

        ApiNeoMessage {
            uuid: self.uuid,
            seq: self.seq,
            role: self.role,
            content,
            tool: self.tool_name,
            status: self.status,
            summary: text("summary"),
            risk: text("risk"),
            approval: text("approval"),
            preview: text("preview"),
            display: text("display"),
            images,
            thinking: text("thinking"),
            model: text("provider"),
            code,
            params,
            created: self.created.and_utc(),
        }
    }
}

impl Conversation {
    /// The conversation with what the chat shows of it: no seeded calls' assistant rows without
    /// text, nothing the model only reads.
    pub async fn into_api(
        self,
        database: &crate::database::Database,
    ) -> Result<ApiNeoConversation, sqlx::Error> {
        let messages = self
            .messages(database)
            .await?
            .into_iter()
            .filter(|message| {
                message.role != role::ASSISTANT
                    || message
                        .content
                        .as_deref()
                        .is_some_and(|text| !text.is_empty())
                    || !message.image_uuids().is_empty()
                    || message
                        .meta
                        .as_ref()
                        .is_some_and(|meta| meta.get("thinking").is_some())
            })
            .map(Message::into_api)
            .collect();

        Ok(ApiNeoConversation {
            uuid: self.uuid,
            server_uuid: self.server_uuid,
            title: self.title,
            status: self.status,
            archived: self.archived.is_some(),
            messages,
            last_activity: self.last_activity.and_utc(),
            created: self.created.and_utc(),
        })
    }

    pub fn into_history_entry(self, user_messages: i64) -> ApiNeoHistoryEntry {
        ApiNeoHistoryEntry {
            uuid: self.uuid,
            title: self.title,
            archived: self.archived.is_some(),
            user_messages,
            last_activity: self.last_activity.and_utc(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(meta: Option<Value>) -> Message {
        Message {
            uuid: uuid::Uuid::nil(),
            conversation_uuid: uuid::Uuid::nil(),
            seq: 1,
            role: role::TOOL.into(),
            content: None,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            status: None,
            meta,
            created: chrono::NaiveDateTime::default(),
        }
    }

    #[test]
    fn reads_arguments_and_images() {
        assert_eq!(message(None).arguments(), json!({}));
        assert_eq!(
            message(Some(json!({"arguments": {"path": "/a"}}))).arguments(),
            json!({"path": "/a"})
        );
        assert_eq!(
            message(Some(json!({"arguments": "x"}))).arguments(),
            json!({})
        );

        let uuid = uuid::Uuid::new_v4();
        assert_eq!(
            message(Some(json!({"images": [uuid.to_string(), "junk"]}))).image_uuids(),
            vec![uuid]
        );
    }

    #[test]
    fn merges_meta() {
        let row = message(Some(json!({"summary": "x", "risk": "read"})));
        assert_eq!(
            row.meta_with(json!({"risk": "write", "ms": 5})),
            json!({"summary": "x", "risk": "write", "ms": 5})
        );
        assert_eq!(message(None).meta_with(json!({"a": 1})), json!({"a": 1}));
    }
}
