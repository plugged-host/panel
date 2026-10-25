//! plugged.host: the chat between a server's owner and its subusers
//! (docs/features/server-chat.md). Every message names its author; an account that has since been
//! deleted leaves the message behind without one.

use crate::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use std::{collections::BTreeMap, sync::LazyLock};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize)]
pub struct ServerChatMessage {
    pub uuid: uuid::Uuid,
    pub user: Option<super::user::User>,
    pub message: String,
    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for ServerChatMessage {
    const NAME: &'static str = "server_chat_message";

    fn get_extension_list() -> &'static super::ModelExtensionList {
        static EXTENSIONS: LazyLock<super::ModelExtensionList> =
            LazyLock::new(|| parking_lot::RwLock::new(Vec::new()));

        &EXTENSIONS
    }

    fn get_extension_data(&self) -> &super::ModelExtensionData {
        &self.extension_data
    }

    #[inline]
    fn base_columns(prefix: Option<&str>) -> BTreeMap<&'static str, compact_str::CompactString> {
        let prefix = prefix.unwrap_or_default();

        let mut columns = BTreeMap::from([
            (
                "server_chat_messages.uuid",
                compact_str::format_compact!("{prefix}uuid"),
            ),
            (
                "server_chat_messages.message",
                compact_str::format_compact!("{prefix}message"),
            ),
            (
                "server_chat_messages.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ]);

        columns.extend(super::user::User::base_columns(Some("user_")));

        columns
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            user: if row
                .try_get::<uuid::Uuid, _>(
                    compact_str::format_compact!("{prefix}user_uuid").as_str(),
                )
                .is_ok()
            {
                Some(super::user::User::map(Some("user_"), row)?)
            } else {
                None
            },
            message: row.try_get(compact_str::format_compact!("{prefix}message").as_str())?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

impl ServerChatMessage {
    /// A page of the server's chat, newest first. The page is reversed before it is shown, so a
    /// chat reads top to bottom while the newest page is the one that opens.
    pub async fn by_server_uuid_with_pagination(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            r#"
            FROM server_chat_messages
            LEFT JOIN users ON users.uuid = server_chat_messages.user_uuid
            LEFT JOIN roles ON roles.uuid = users.role_uuid
            WHERE server_chat_messages.server_uuid = $1
            "#,
            "server_chat_messages.created DESC, server_chat_messages.uuid DESC",
        )
        .bind(server_uuid)
        .fetch(database, page, per_page)
        .await
    }

    /// Writes a message and returns the row, so the caller can answer with what it stored.
    pub async fn create(
        database: &crate::database::Database,
        server_uuid: uuid::Uuid,
        user_uuid: uuid::Uuid,
        message: &str,
    ) -> Result<uuid::Uuid, crate::database::DatabaseError> {
        let uuid = sqlx::query_scalar(
            "INSERT INTO server_chat_messages (server_uuid, user_uuid, message)
            VALUES ($1, $2, $3) RETURNING uuid",
        )
        .bind(server_uuid)
        .bind(user_uuid)
        .bind(message)
        .fetch_one(database.write())
        .await?;

        Ok(uuid)
    }

    /// The message just written, with its author.
    pub async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {} FROM server_chat_messages
            LEFT JOIN users ON users.uuid = server_chat_messages.user_uuid
            LEFT JOIN roles ON roles.uuid = users.role_uuid
            WHERE server_chat_messages.uuid = $1",
            Self::columns_sql(None)
        )))
        .bind(uuid)
        .fetch_optional(database.write())
        .await?;

        row.map(|row| Self::map(None, &row)).transpose()
    }
}

#[derive(ToSchema, Serialize)]
pub struct ApiServerChatMessage {
    pub uuid: uuid::Uuid,
    pub user: Option<super::user::ApiUser>,
    pub message: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

impl ApiServerChatMessage {
    pub fn from_message(message: ServerChatMessage, user: Option<super::user::ApiUser>) -> Self {
        Self {
            uuid: message.uuid,
            user,
            message: message.message,
            created: message.created.and_utc(),
        }
    }
}
