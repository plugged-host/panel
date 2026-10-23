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
pub struct NodeDatabaseHost {
    pub node: Fetchable<super::node::Node>,
    pub database_host: super::database_host::DatabaseHost,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for NodeDatabaseHost {
    const NAME: &'static str = "node_database_host";

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
                "node_database_hosts.node_uuid",
                compact_str::format_compact!("{prefix}node_uuid"),
            ),
            (
                "node_database_hosts.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ]);

        columns.extend(super::database_host::DatabaseHost::base_columns(Some(
            "database_host_",
        )));

        columns
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            node: super::node::Node::get_fetchable(
                row.try_get(compact_str::format_compact!("{prefix}node_uuid").as_str())?,
            ),
            database_host: super::database_host::DatabaseHost::map(Some("database_host_"), row)?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

impl NodeDatabaseHost {
    pub async fn by_node_uuid_database_host_uuid(
        database: &crate::database::Database,
        node_uuid: uuid::Uuid,
        database_host_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM node_database_hosts
            JOIN database_hosts ON node_database_hosts.database_host_uuid = database_hosts.uuid
            WHERE node_database_hosts.node_uuid = $1 AND node_database_hosts.database_host_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(database_host_uuid)
        .fetch_optional(database.read())
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    pub async fn by_node_uuid_database_host_uuid_with_transaction(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        node_uuid: uuid::Uuid,
        database_host_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM node_database_hosts
            JOIN database_hosts ON node_database_hosts.database_host_uuid = database_hosts.uuid
            WHERE node_database_hosts.node_uuid = $1 AND node_database_hosts.database_host_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(database_host_uuid)
        .fetch_optional(&mut **transaction)
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    pub async fn by_node_uuid_with_pagination(
        database: &crate::database::Database,
        node_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!(
                r#"
                FROM node_database_hosts
                JOIN database_hosts ON node_database_hosts.database_host_uuid = database_hosts.uuid
                WHERE node_database_hosts.node_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["database_hosts.name"], &["database_hosts.uuid"]),
            ),
            "node_database_hosts.created, node_database_hosts.node_uuid, node_database_hosts.database_host_uuid",
        )
        .bind(node_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for NodeDatabaseHost {
    type AdminApiObject = AdminApiNodeDatabaseHost;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiNodeDatabaseHost::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiNodeDatabaseHost {
                database_host: self.database_host.into_admin_api_object(state, ()).await?,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreateNodeDatabaseHostOptions {
    #[garde(skip)]
    pub node_uuid: uuid::Uuid,
    #[garde(skip)]
    pub database_host_uuid: uuid::Uuid,
}

#[async_trait::async_trait]
impl CreatableModel for NodeDatabaseHost {
    type CreateOptions<'a> = CreateNodeDatabaseHostOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<NodeDatabaseHost>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        super::database_host::DatabaseHost::by_uuid_optional_cached(
            &state.database,
            options.database_host_uuid,
        )
        .await?
        .ok_or(crate::database::InvalidRelationError("database_host"))?;

        let mut query_builder = InsertQueryBuilder::new("node_database_hosts");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("node_uuid", options.node_uuid)
            .set("database_host_uuid", options.database_host_uuid);

        query_builder.execute(&mut **transaction).await?;

        let mut result = match Self::by_node_uuid_database_host_uuid_with_transaction(
            transaction,
            options.node_uuid,
            options.database_host_uuid,
        )
        .await?
        {
            Some(node_database_host) => node_database_host,
            None => return Err(sqlx::Error::RowNotFound.into()),
        };

        Self::run_after_create_handlers(&mut result, &options, state, transaction).await?;

        Ok(result)
    }
}

#[async_trait::async_trait]
impl DeletableModel for NodeDatabaseHost {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<NodeDatabaseHost>> =
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

        sqlx::query(
            r#"
            DELETE FROM node_database_hosts
            WHERE node_database_hosts.node_uuid = $1 AND node_database_hosts.database_host_uuid = $2
            "#,
        )
        .bind(self.node.uuid)
        .bind(self.database_host.uuid)
        .execute(&mut **transaction)
        .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[schema_extension_derive::extendible]
#[init_args(NodeDatabaseHost, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "NodeDatabaseHost")]
pub struct AdminApiNodeDatabaseHost {
    pub database_host: super::database_host::AdminApiDatabaseHost,

    pub created: chrono::DateTime<chrono::Utc>,
}
