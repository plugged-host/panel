//! plugged.host: neo on Discord's short memory, the `neo_discord_messages` table
//! (docs/features/neo-discord.md).
//!
//! A `user` row is a message someone sent the bot; while it waits for its answer it is `pending`.
//! The rows of one answer (the messages it answers, the bot's own turns and its lookups) share the
//! answer's turn: the uuid of the first message it answers. `position` keeps the order inside an
//! answer. Rows are deleted after neo's retention days, like the panel's chats.

use serde_json::Value;

pub mod role {
    pub const USER: &str = "user";
    pub const ASSISTANT: &str = "assistant";
    pub const TOOL: &str = "tool";
}

pub mod status {
    pub const PENDING: &str = "pending";
    pub const RUNNING: &str = "running";
    pub const DONE: &str = "done";
    /// Answered with a fixed line (a limit, an outage), or given up: never replayed to the model.
    pub const FAILED: &str = "failed";
}

/// A message still waiting after this long has lost its worker for good: given up.
pub const GIVE_UP_MINUTES: i64 = 15;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Message {
    pub uuid: uuid::Uuid,
    pub conversation: String,
    pub turn_uuid: Option<uuid::Uuid>,
    pub position: i16,
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

const COLUMNS: &str = "uuid, conversation, turn_uuid, position, discord_message_id, channel_id, guild_id, author_id, author_name, role, content, tool_calls, tool_call_id, tool_name, status, meta, created";

impl Message {
    /// A DM, not a channel of the server.
    pub fn is_direct(&self) -> bool {
        self.guild_id.is_none()
    }
}

/// Stores a message for neo to answer (`pending`).
pub async fn insert_incoming(
    database: &crate::database::Database,
    incoming: &super::inbox::Incoming,
) -> Result<uuid::Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO neo_discord_messages (conversation, discord_message_id, channel_id, guild_id,
            author_id, author_name, role, content, status, meta)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING uuid",
    )
    .bind(incoming.conversation())
    .bind(&incoming.discord_message_id)
    .bind(&incoming.channel_id)
    .bind(&incoming.guild_id)
    .bind(&incoming.author_id)
    .bind(&incoming.author_name)
    .bind(role::USER)
    .bind(&incoming.content)
    .bind(status::PENDING)
    .bind(&incoming.meta)
    .fetch_one(database.write())
    .await
}

/// What a turn writes after the messages it answers.
pub struct Reply<'a> {
    pub role: &'a str,
    pub content: Option<&'a str>,
    pub tool_calls: Option<&'a Value>,
    pub tool_call_id: Option<&'a str>,
    pub tool_name: Option<&'a str>,
    pub status: &'a str,
    pub meta: Option<&'a Value>,
}

/// Stores one of the bot's turns or lookups in the answer `first` starts.
pub async fn insert_reply(
    database: &crate::database::Database,
    first: &Message,
    position: i16,
    reply: Reply<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO neo_discord_messages (conversation, turn_uuid, position, channel_id, guild_id,
            author_id, role, content, tool_calls, tool_call_id, tool_name, status, meta)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(&first.conversation)
    .bind(first.uuid)
    .bind(position)
    .bind(&first.channel_id)
    .bind(&first.guild_id)
    .bind(&first.author_id)
    .bind(reply.role)
    .bind(reply.content)
    .bind(reply.tool_calls)
    .bind(
        reply
            .tool_call_id
            .map(|id| id.chars().take(64).collect::<String>()),
    )
    .bind(
        reply
            .tool_name
            .map(|name| name.chars().take(64).collect::<String>()),
    )
    .bind(reply.status)
    .bind(reply.meta)
    .execute(database.write())
    .await?;

    Ok(())
}

/// Takes every waiting message of the conversation for one answer (`running`), oldest first;
/// `None` when nothing waits. Messages older than [`GIVE_UP_MINUTES`] are left for [`give_up`].
/// The caller holds the conversation's lock.
pub async fn claim(
    database: &crate::database::Database,
    conversation: &str,
) -> Result<Option<Vec<Message>>, sqlx::Error> {
    let mut rows: Vec<Message> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM neo_discord_messages
        WHERE conversation = $1 AND role = $2 AND status = $3
            AND created >= NOW() - make_interval(mins => $4)
        ORDER BY created, uuid"
    )))
    .bind(conversation)
    .bind(role::USER)
    .bind(status::PENDING)
    .bind(GIVE_UP_MINUTES as i32)
    .fetch_all(database.write())
    .await?;
    let Some(first) = rows.first().map(|row| row.uuid) else {
        return Ok(None);
    };

    let uuids: Vec<uuid::Uuid> = rows.iter().map(|row| row.uuid).collect();
    sqlx::query(
        "UPDATE neo_discord_messages SET status = $2, turn_uuid = $3
        WHERE uuid = ANY($1) AND status = $4",
    )
    .bind(&uuids)
    .bind(status::RUNNING)
    .bind(first)
    .bind(status::PENDING)
    .execute(database.write())
    .await?;

    for row in &mut rows {
        row.status = status::RUNNING.into();
        row.turn_uuid = Some(first);
    }

    Ok(Some(rows))
}

/// Sets the status of an answer's messages and adds `meta` to theirs.
pub async fn finish(
    database: &crate::database::Database,
    rows: &[Message],
    status: &str,
    meta: Option<&Value>,
) -> Result<(), sqlx::Error> {
    let uuids: Vec<uuid::Uuid> = rows.iter().map(|row| row.uuid).collect();

    sqlx::query(
        "UPDATE neo_discord_messages
        SET status = $2, meta = CASE WHEN $3::jsonb IS NULL THEN meta ELSE COALESCE(meta, '{}'::jsonb) || $3::jsonb END
        WHERE uuid = ANY($1)",
    )
    .bind(&uuids)
    .bind(status)
    .bind(meta)
    .execute(database.write())
    .await?;

    Ok(())
}

/// The answers given in this conversation before `first`, at most `turns` of them and none older
/// than `minutes`, oldest first, each with its rows in order. Only answers that finished normally:
/// every tool call in them has its result.
pub async fn history(
    database: &crate::database::Database,
    first: &Message,
    minutes: i64,
    turns: i64,
) -> Result<Vec<Vec<Message>>, sqlx::Error> {
    if turns <= 0 {
        return Ok(Vec::new());
    }

    let since = first.created - chrono::Duration::minutes(minutes.max(1));
    let mut turn_uuids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT uuid FROM neo_discord_messages
        WHERE conversation = $1 AND role = $2 AND status = $3 AND uuid = turn_uuid
            AND created < $4 AND created >= $5
        ORDER BY created DESC, uuid DESC
        LIMIT $6",
    )
    .bind(&first.conversation)
    .bind(role::USER)
    .bind(status::DONE)
    .bind(first.created)
    .bind(since)
    .bind(turns)
    .fetch_all(database.read())
    .await?;
    if turn_uuids.is_empty() {
        return Ok(Vec::new());
    }
    turn_uuids.reverse();

    let rows: Vec<Message> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM neo_discord_messages
        WHERE turn_uuid = ANY($1)
        ORDER BY position, created, uuid"
    )))
    .bind(&turn_uuids)
    .fetch_all(database.read())
    .await?;

    Ok(group_turns(&turn_uuids, rows))
}

/// Rows grouped by their turn, in the order of `turns`, each turn's rows in `position` order (the
/// messages it answers first, oldest first).
fn group_turns(turns: &[uuid::Uuid], rows: Vec<Message>) -> Vec<Vec<Message>> {
    let mut grouped: Vec<Vec<Message>> = turns.iter().map(|_| Vec::new()).collect();
    for row in rows {
        if let Some(index) = turns.iter().position(|turn| Some(*turn) == row.turn_uuid) {
            grouped[index].push(row);
        }
    }
    for turn in &mut grouped {
        turn.sort_by_key(|row| (row.position, row.created));
    }

    grouped
}

/// The conversations with a message still waiting for its answer (after a restart or a
/// reconnect, their workers are started again).
pub async fn waiting_conversations(
    database: &crate::database::Database,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT conversation FROM neo_discord_messages
        WHERE role = $1 AND status = $2 AND created >= NOW() - make_interval(mins => $3)",
    )
    .bind(role::USER)
    .bind(status::PENDING)
    .bind(GIVE_UP_MINUTES as i32)
    .fetch_all(database.read())
    .await
}

/// The conversations with a message that has waited `seconds` or more (but less than
/// [`GIVE_UP_MINUTES`]): its worker is gone.
pub async fn stuck_conversations(
    database: &crate::database::Database,
    seconds: i32,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT conversation FROM neo_discord_messages
        WHERE role = $1 AND status = $2
            AND created >= NOW() - make_interval(mins => $3)
            AND created < NOW() - make_interval(secs => $4)",
    )
    .bind(role::USER)
    .bind(status::PENDING)
    .bind(GIVE_UP_MINUTES as i32)
    .bind(f64::from(seconds))
    .fetch_all(database.read())
    .await
}

/// Gives up messages that waited (or were being answered) for longer than [`GIVE_UP_MINUTES`]:
/// `failed`, never answered or replayed. Returns how many.
pub async fn give_up(database: &crate::database::Database) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE neo_discord_messages SET status = $1
        WHERE role = $2 AND status IN ($3, $4) AND created < NOW() - make_interval(mins => $5)",
    )
    .bind(status::FAILED)
    .bind(role::USER)
    .bind(status::PENDING)
    .bind(status::RUNNING)
    .bind(GIVE_UP_MINUTES as i32)
    .execute(database.write())
    .await?
    .rows_affected())
}

/// Deletes what is older than `retention_days`. Returns the rows deleted.
pub async fn prune(
    database: &crate::database::Database,
    retention_days: i32,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM neo_discord_messages WHERE created < NOW() - make_interval(days => $1)",
    )
    .bind(retention_days.max(1))
    .execute(database.write())
    .await?
    .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(turn: uuid::Uuid, position: i16) -> Message {
        Message {
            uuid: uuid::Uuid::new_v4(),
            conversation: "1:2".into(),
            turn_uuid: Some(turn),
            position,
            discord_message_id: None,
            channel_id: "1".into(),
            guild_id: None,
            author_id: "2".into(),
            author_name: None,
            role: role::USER.into(),
            content: None,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            status: status::DONE.into(),
            meta: None,
            created: chrono::Utc::now().naive_utc(),
        }
    }

    #[test]
    fn rows_are_grouped_by_turn_in_order() {
        let (a, b) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let rows = vec![
            row(b, 0),
            row(a, 2),
            row(a, 0),
            row(b, 1),
            row(a, 1),
            row(uuid::Uuid::nil(), 0),
        ];

        let grouped = group_turns(&[a, b], rows);
        assert_eq!(grouped.len(), 2);
        assert_eq!(
            grouped[0].iter().map(|r| r.position).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(grouped[1].iter().all(|r| r.turn_uuid == Some(b)));
        assert!(grouped[0][0].is_direct());
    }
}
