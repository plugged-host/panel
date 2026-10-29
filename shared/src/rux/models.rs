//! plugged.host: rux's rows (tables `rux_*`, database/src/schema.ts). The structs are the rows as
//! they are; each part of rux adds what it needs in its own `impl` blocks next to its code (an
//! inherent impl can live in any module of the crate), so this file stays the one list of
//! columns. Runtime queries only, values bound.

use serde::Serialize;
use serde_json::Value;

/// `rux_messages.role`
pub mod role {
    pub const USER: &str = "user";
    pub const ASSISTANT: &str = "assistant";
    pub const TOOL: &str = "tool";
}

/// `rux_messages.status`
pub mod status {
    /// An order waiting for its worker.
    pub const PENDING: &str = "pending";
    pub const RUNNING: &str = "running";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    /// Not allowed, or a server rux does not work on: never shown to the model.
    pub const IGNORED: &str = "ignored";
}

/// A row of rux's short memory. A `user` row is an order (`pending` until its worker claims it);
/// the rows of one order share its `turn_id` (the order's own id).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub conversation: String,
    pub turn_id: Option<i64>,
    pub discord_message_id: Option<String>,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author_id: String,
    pub author_name: Option<String>,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: String,
    pub meta: Option<Value>,
    pub created: chrono::NaiveDateTime,
}

pub const MESSAGE_COLUMNS: &str = "id, conversation, turn_id, discord_message_id, channel_id, guild_id, author_id, author_name, role, content, tool_calls, tool_call_id, tool_name, status, meta, created";

/// A new order, as the inbox stores it.
#[derive(Debug, Clone, Default)]
pub struct NewOrder {
    pub conversation: String,
    pub discord_message_id: Option<String>,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author_id: String,
    pub author_name: Option<String>,
    pub content: String,
    pub meta: Option<Value>,
}

/// Another row of an order's turn: the model's answer or a tool's output.
#[derive(Debug, Clone, Default)]
pub struct TurnRow {
    pub role: &'static str,
    pub content: Option<String>,
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: Option<&'static str>,
    pub meta: Option<Value>,
}

/// `<channel id>:<author id>`: one person in one channel (or DM).
pub fn conversation_key(channel_id: &str, author_id: &str) -> String {
    format!("{channel_id}:{author_id}")
}

impl Message {
    pub fn is_direct(&self) -> bool {
        self.guild_id.is_none()
    }

    pub async fn by_id(
        database: &crate::database::Database,
        id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM rux_messages WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(database.read())
        .await
    }

    /// Stores an order (`pending`, its own turn) and answers its id.
    pub async fn insert_order(
        database: &crate::database::Database,
        order: &NewOrder,
    ) -> Result<i64, sqlx::Error> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO rux_messages
                (conversation, discord_message_id, channel_id, guild_id, author_id, author_name, role, content, status, meta)
            VALUES ($1, $2, $3, $4, $5, $6, 'user', $7, 'pending', $8)
            RETURNING id",
        )
        .bind(&order.conversation)
        .bind(&order.discord_message_id)
        .bind(&order.channel_id)
        .bind(&order.guild_id)
        .bind(&order.author_id)
        .bind(&order.author_name)
        .bind(&order.content)
        .bind(&order.meta)
        .fetch_one(database.write())
        .await?;

        sqlx::query("UPDATE rux_messages SET turn_id = id WHERE id = $1")
            .bind(id)
            .execute(database.write())
            .await?;

        Ok(id)
    }

    /// Moves an order from `from` to `to` when it still is `from`; `false` when someone else
    /// got there first.
    pub async fn claim(
        database: &crate::database::Database,
        id: i64,
        from: &str,
        to: &str,
    ) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("UPDATE rux_messages SET status = $3 WHERE id = $1 AND status = $2")
                .bind(id)
                .bind(from)
                .bind(to)
                .execute(database.write())
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// Sets the order's status and merges `meta` into its meta.
    pub async fn finish(
        database: &crate::database::Database,
        id: i64,
        status: &str,
        meta: Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE rux_messages SET status = $2, meta = COALESCE(meta, '{}'::jsonb) || $3::jsonb
            WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .bind(if meta.is_object() {
            meta
        } else {
            Value::Object(Default::default())
        })
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// Adds a row to this order's turn.
    pub async fn store(
        &self,
        database: &crate::database::Database,
        row: TurnRow,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO rux_messages
                (conversation, turn_id, channel_id, guild_id, author_id, role, content, tool_calls, tool_call_id, tool_name, status, meta)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(&self.conversation)
        .bind(self.id)
        .bind(&self.channel_id)
        .bind(&self.guild_id)
        .bind(&self.author_id)
        .bind(row.role)
        .bind(row.content)
        .bind(row.tool_calls)
        .bind(row.tool_call_id)
        .bind(row.tool_name)
        .bind(row.status.unwrap_or(status::DONE))
        .bind(row.meta)
        .execute(database.write())
        .await?;

        Ok(())
    }

    /// The done orders of this conversation before `before`, since `since`, newest `limit`,
    /// oldest first.
    pub async fn earlier_orders(
        database: &crate::database::Database,
        conversation: &str,
        before: i64,
        since: chrono::NaiveDateTime,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut rows = sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM rux_messages
            WHERE conversation = $1 AND role = 'user' AND status = 'done' AND id < $2 AND created >= $3
            ORDER BY id DESC LIMIT $4"
        )))
        .bind(conversation)
        .bind(before)
        .bind(since)
        .bind(limit)
        .fetch_all(database.read())
        .await?;
        rows.reverse();

        Ok(rows)
    }

    /// The other rows of an order's turn, in order.
    pub async fn turn(
        database: &crate::database::Database,
        turn_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MESSAGE_COLUMNS} FROM rux_messages WHERE turn_id = $1 AND id <> $1 ORDER BY id"
        )))
        .bind(turn_id)
        .fetch_all(database.read())
        .await
    }

    /// Whether the same person gave another order in this conversation after `id`.
    pub async fn superseded(
        database: &crate::database::Database,
        conversation: &str,
        id: i64,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM rux_messages WHERE conversation = $1 AND role = 'user' AND id > $2)",
        )
        .bind(conversation)
        .bind(id)
        .fetch_one(database.read())
        .await
    }
}

/// `rux_cases`: one moderation action.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Case {
    pub id: i64,
    pub guild_id: String,
    /// warn, timeout, untimeout, kick, ban, unban, delete, purge, clear_warnings, …
    pub action: String,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub channel_id: Option<String>,
    pub moderator_discord_id: String,
    pub moderator_account_id: Option<i64>,
    pub moderator_user_uuid: Option<uuid::Uuid>,
    pub reason: Option<String>,
    pub duration_seconds: Option<i32>,
    pub automatic: bool,
    pub rule_id: Option<i64>,
    pub expires: Option<chrono::NaiveDateTime>,
    pub resolved: Option<chrono::NaiveDateTime>,
    pub revoked: Option<chrono::NaiveDateTime>,
    pub meta: Option<Value>,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const CASE_COLUMNS: &str = "id, guild_id, action, target_id, target_name, channel_id, moderator_discord_id, moderator_account_id, moderator_user_uuid, reason, duration_seconds, automatic, rule_id, expires, resolved, revoked, meta, created, updated";

/// `rux_giveaways.status` and `payout_status`
pub mod giveaway_status {
    pub const ACTIVE: &str = "active";
    pub const DRAWING: &str = "drawing";
    pub const ENDED: &str = "ended";
    pub const CANCELLED: &str = "cancelled";
    pub const PAYOUT_NONE: &str = "none";
    pub const PAYOUT_PENDING: &str = "pending";
    pub const PAYOUT_PAID: &str = "paid";
    pub const PAYOUT_FAILED: &str = "failed";
}

/// What a giveaway pays each winner. Credits go where the credits live (noprofit.space's rux
/// routes while it is connected); tokens and the memberships are always the panel's own.
pub mod prize_kind {
    pub const CREDITS: &str = "credits";
    pub const TOKENS: &str = "tokens";
    pub const PLUS_DAYS: &str = "plus_days";
    pub const PLUSPLUS_DAYS: &str = "plusplus_days";
    pub const ALL: [&str; 4] = [CREDITS, TOKENS, PLUS_DAYS, PLUSPLUS_DAYS];

    /// Paid by the panel itself whatever is connected.
    pub fn panel_only(kind: &str) -> bool {
        kind != CREDITS
    }
}

/// `rux_giveaways`: a giveaway of credits, tokens or membership days.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Giveaway {
    pub id: i64,
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: Option<String>,
    pub prize: String,
    /// What each winner gets: credits, tokens or days, by [`Self::prize_kind`].
    pub credits: i64,
    /// `credits`, `tokens`, `plus_days` or `plusplus_days` ([`prize_kind`]).
    pub prize_kind: String,
    pub winners_count: i16,
    pub ends: chrono::NaiveDateTime,
    pub exclude_recent: i16,
    pub excluded: Option<Value>,
    pub status: String,
    pub created_by_discord_id: String,
    pub created_by_account_id: Option<i64>,
    pub created_by_user_uuid: Option<uuid::Uuid>,
    pub ended: Option<chrono::NaiveDateTime>,
    pub payout_status: String,
    pub payout_attempts: i32,
    pub payout_error: Option<String>,
    pub payout_reference: Option<String>,
    pub rerolls: i16,
    pub rendered_entries: i32,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const GIVEAWAY_COLUMNS: &str = "id, guild_id, channel_id, message_id, prize, credits, prize_kind, winners_count, ends, exclude_recent, excluded, status, created_by_discord_id, created_by_account_id, created_by_user_uuid, ended, payout_status, payout_attempts, payout_error, payout_reference, rerolls, rendered_entries, created, updated";

/// `rux_giveaway_entries`: someone in a giveaway.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct GiveawayEntry {
    pub id: i64,
    pub giveaway_id: i64,
    pub discord_id: String,
    pub username: Option<String>,
    pub won: Option<chrono::NaiveDateTime>,
    pub draw: Option<i16>,
    pub payout: Option<String>,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const GIVEAWAY_ENTRY_COLUMNS: &str =
    "id, giveaway_id, discord_id, username, won, draw, payout, created, updated";

/// `rux_moderator_roles`: a role whose members may give moderation orders on its server.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ModeratorRole {
    pub id: i64,
    pub guild_id: String,
    pub role_id: String,
    pub role_name: Option<String>,
    pub granted_by_discord_id: String,
    pub granted_by_account_id: Option<i64>,
    pub granted_by_user_uuid: Option<uuid::Uuid>,
    /// Discord permission bits rux added to the role, as a decimal string.
    pub permissions_added: String,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const MODERATOR_ROLE_COLUMNS: &str = "id, guild_id, role_id, role_name, granted_by_discord_id, granted_by_account_id, granted_by_user_uuid, permissions_added, created, updated";

impl ModeratorRole {
    /// The first of these roles that is a moderator role of the server.
    pub async fn among(
        database: &crate::database::Database,
        guild_id: &str,
        role_ids: &[String],
    ) -> Result<Option<Self>, sqlx::Error> {
        if role_ids.is_empty() {
            return Ok(None);
        }

        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MODERATOR_ROLE_COLUMNS} FROM rux_moderator_roles
            WHERE guild_id = $1 AND role_id = ANY($2) ORDER BY id LIMIT 1"
        )))
        .bind(guild_id)
        .bind(role_ids)
        .fetch_optional(database.read())
        .await
    }

    pub async fn by_role(
        database: &crate::database::Database,
        guild_id: &str,
        role_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MODERATOR_ROLE_COLUMNS} FROM rux_moderator_roles WHERE guild_id = $1 AND role_id = $2"
        )))
        .bind(guild_id)
        .bind(role_id)
        .fetch_optional(database.read())
        .await
    }

    pub async fn all(
        database: &crate::database::Database,
        guild_id: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(sqlx::AssertSqlSafe(format!(
            "SELECT {MODERATOR_ROLE_COLUMNS} FROM rux_moderator_roles WHERE guild_id = $1 ORDER BY id"
        )))
        .bind(guild_id)
        .fetch_all(database.read())
        .await
    }
}

/// `rux_rules`: a standing rule.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Rule {
    pub id: i64,
    pub guild_id: String,
    /// What a member's message must not do ("when").
    pub description: String,
    /// What looks close but is fine ("not when").
    pub exceptions: Option<String>,
    pub keywords: Value,
    /// warn, timeout, delete, kick, ban
    pub action: String,
    pub duration_seconds: Option<i32>,
    pub delete_message: bool,
    pub reason: Option<String>,
    pub channel_ids: Option<Value>,
    pub active: bool,
    pub created_by_discord_id: String,
    pub created_by_account_id: Option<i64>,
    pub created_by_user_uuid: Option<uuid::Uuid>,
    pub hits: i32,
    pub actions: i32,
    pub last_action: Option<chrono::NaiveDateTime>,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const RULE_COLUMNS: &str = "id, guild_id, description, exceptions, keywords, action, duration_seconds, delete_message, reason, channel_ids, active, created_by_discord_id, created_by_account_id, created_by_user_uuid, hits, actions, last_action, created, updated";

/// `rux_watches`: a channel (name or id) where every message is checked.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Watch {
    pub id: i64,
    pub guild_id: String,
    pub channel: String,
    pub added_by_discord_id: Option<String>,
    pub created: chrono::NaiveDateTime,
    pub updated: chrono::NaiveDateTime,
}

pub const WATCH_COLUMNS: &str = "id, guild_id, channel, added_by_discord_id, created, updated";

/// `rux_watch_items`: a message waiting for its check.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WatchItem {
    pub id: i64,
    pub guild_id: Option<String>,
    pub message_id: String,
    pub channel_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub roles: Option<Value>,
    pub content: String,
    pub replied_to: Option<String>,
    pub created: chrono::NaiveDateTime,
}

pub const WATCH_ITEM_COLUMNS: &str = "id, guild_id, message_id, channel_id, author_id, author_name, roles, content, replied_to, created";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversations_are_per_person_and_channel() {
        assert_eq!(conversation_key("1", "2"), "1:2");
    }
}
