//! plugged.host: a person's notification inbox (docs/features/notifications.md). A notification is a
//! `kind` (the text, drawn in the reader's language) and its `data`; [`crate::notifications`] sends
//! them, the account's `notifications` routes list, read and delete them.

use crate::{models::InsertQueryBuilder, prelude::*};
use garde::Validate;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone)]
pub struct UserNotification {
    pub uuid: uuid::Uuid,
    /// The server it is about, while that server exists, with its short id for links.
    pub server_uuid: Option<uuid::Uuid>,
    pub server_uuid_short: Option<i32>,

    pub kind: compact_str::CompactString,
    pub data: serde_json::Value,

    pub read: Option<chrono::NaiveDateTime>,
    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for UserNotification {
    const NAME: &'static str = "user_notification";

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

        BTreeMap::from([
            (
                "user_notifications.uuid",
                compact_str::format_compact!("{prefix}uuid"),
            ),
            (
                "user_notifications.server_uuid",
                compact_str::format_compact!("{prefix}server_uuid"),
            ),
            (
                "servers.uuid_short",
                compact_str::format_compact!("{prefix}server_uuid_short"),
            ),
            (
                "user_notifications.kind",
                compact_str::format_compact!("{prefix}kind"),
            ),
            (
                "user_notifications.data",
                compact_str::format_compact!("{prefix}data"),
            ),
            (
                "user_notifications.read",
                compact_str::format_compact!("{prefix}read"),
            ),
            (
                "user_notifications.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ])
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            server_uuid: row
                .try_get(compact_str::format_compact!("{prefix}server_uuid").as_str())?,
            server_uuid_short: row
                .try_get(compact_str::format_compact!("{prefix}server_uuid_short").as_str())?,
            kind: row.try_get(compact_str::format_compact!("{prefix}kind").as_str())?,
            data: row.try_get(compact_str::format_compact!("{prefix}data").as_str())?,
            read: row.try_get(compact_str::format_compact!("{prefix}read").as_str())?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

/// The tables every read of a notification joins: the server for its link.
const FROM: &str = "FROM user_notifications
    LEFT JOIN servers ON servers.uuid = user_notifications.server_uuid";

impl UserNotification {
    /// A page of the person's notifications, newest first.
    pub async fn by_user_uuid_with_pagination(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!("{FROM} WHERE user_notifications.user_uuid = $1"),
            "user_notifications.created DESC, user_notifications.uuid DESC",
        )
        .bind(user_uuid)
        .fetch(database, page, per_page)
        .await
    }

    pub async fn by_user_uuid_uuid(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {} {FROM}
            WHERE user_notifications.user_uuid = $1 AND user_notifications.uuid = $2",
            Self::columns_sql(None)
        )))
        .bind(user_uuid)
        .bind(uuid)
        .fetch_optional(database.write())
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    /// How many of the person's notifications are unread (the bell's badge).
    pub async fn count_unread_by_user_uuid(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_notifications
            WHERE user_notifications.user_uuid = $1 AND user_notifications.read IS NULL",
        )
        .bind(user_uuid)
        .fetch_one(database.read())
        .await
    }

    /// Marks one notification read, or every unread one (`uuid` none). Returns how many changed.
    pub async fn mark_read(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        uuid: Option<uuid::Uuid>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE user_notifications SET read = NOW()
            WHERE user_notifications.user_uuid = $1 AND user_notifications.read IS NULL
                AND ($2::uuid IS NULL OR user_notifications.uuid = $2)",
        )
        .bind(user_uuid)
        .bind(uuid)
        .execute(database.write())
        .await?;

        Ok(result.rows_affected())
    }

    /// Deletes every notification of the person (the inbox's "Clear").
    pub async fn delete_by_user_uuid(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM user_notifications WHERE user_notifications.user_uuid = $1")
                .bind(user_uuid)
                .execute(database.write())
                .await?;

        Ok(result.rows_affected())
    }

    /// Whether the person was sent a notification of `kind` about the server since `since`.
    pub async fn sent_since(
        database: &crate::database::Database,
        user_uuid: uuid::Uuid,
        server_uuid: uuid::Uuid,
        kind: &str,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM user_notifications
                WHERE user_notifications.user_uuid = $1 AND user_notifications.server_uuid = $2
                    AND user_notifications.kind = $3 AND user_notifications.created >= $4
            )",
        )
        .bind(user_uuid)
        .bind(server_uuid)
        .bind(kind)
        .bind(since.naive_utc())
        .fetch_one(database.write())
        .await
    }

    /// Deletes read notifications older than `read_before` and any older than `before`.
    pub async fn delete_older_than(
        database: &crate::database::Database,
        read_before: chrono::DateTime<chrono::Utc>,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM user_notifications
            WHERE (user_notifications.read IS NOT NULL AND user_notifications.created < $1)
                OR user_notifications.created < $2",
        )
        .bind(read_before.naive_utc())
        .bind(before.naive_utc())
        .execute(database.write())
        .await?;

        Ok(result.rows_affected())
    }
}

#[async_trait::async_trait]
impl IntoApiObject for UserNotification {
    type ApiObject = ApiUserNotification;
    type ExtraArgs<'a> = ();

    async fn into_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::ApiObject, crate::database::DatabaseError> {
        let api_object = ApiUserNotification::init_hooks(&self, state).await?;

        let server = match (self.server_uuid, self.server_uuid_short) {
            (Some(uuid), Some(uuid_short)) => Some(ApiUserNotificationServer {
                uuid,
                uuid_short: compact_str::format_compact!("{:08x}", uuid_short),
            }),
            _ => None,
        };

        let api_object = finish_extendible!(
            ApiUserNotification {
                uuid: self.uuid,
                kind: self.kind,
                data: self.data,
                server,
                read: self.read.map(|read| read.and_utc()),
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreateUserNotificationOptions {
    #[garde(skip)]
    pub user_uuid: uuid::Uuid,
    #[garde(skip)]
    pub server_uuid: Option<uuid::Uuid>,

    #[garde(length(chars, min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub kind: compact_str::CompactString,
    #[garde(skip)]
    pub data: serde_json::Value,
}

#[async_trait::async_trait]
impl CreatableModel for UserNotification {
    type CreateOptions<'a> = CreateUserNotificationOptions;
    type CreateResult = ();

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<UserNotification>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self::CreateResult, crate::database::DatabaseError> {
        options.validate()?;

        let mut query_builder = InsertQueryBuilder::new("user_notifications");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("user_uuid", options.user_uuid)
            .set("server_uuid", options.server_uuid)
            .set("kind", &options.kind)
            .set("data", &options.data);

        query_builder.execute(&mut **transaction).await?;

        Self::run_after_create_handlers(&mut (), &options, state, transaction).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeletableModel for UserNotification {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<UserNotification>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &DELETE_LISTENERS
    }

    async fn delete_with_transaction(
        &self,
        state: &crate::State,
        options: Self::DeleteOptions,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), anyhow::Error> {
        self.run_delete_handlers(&options, state, transaction)
            .await?;

        sqlx::query("DELETE FROM user_notifications WHERE user_notifications.uuid = $1")
            .bind(self.uuid)
            .execute(&mut **transaction)
            .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

/// The server a notification is about, for a link to it.
#[derive(ToSchema, Serialize)]
pub struct ApiUserNotificationServer {
    pub uuid: uuid::Uuid,
    pub uuid_short: compact_str::CompactString,
}

#[schema_extension_derive::extendible]
#[init_args(UserNotification, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "UserNotification")]
pub struct ApiUserNotification {
    pub uuid: uuid::Uuid,

    /// Which notification it is (`transferCompleted`, `admin.transferFailed`, ...); the panel
    /// draws its text from this and `data`.
    pub kind: compact_str::CompactString,
    /// The values the text is drawn with (a server's name, a new address, ...).
    pub data: serde_json::Value,
    /// The server it is about, while the server exists.
    pub server: Option<ApiUserNotificationServer>,

    pub read: Option<chrono::DateTime<chrono::Utc>>,
    pub created: chrono::DateTime<chrono::Utc>,
}
