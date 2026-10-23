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
pub struct NodeDatabaseAgentHost {
    pub node: Fetchable<super::node::Node>,
    pub database_agent_host: super::database_agent_host::DatabaseAgentHost,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for NodeDatabaseAgentHost {
    const NAME: &'static str = "node_database_agent_host";

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
                "node_database_agent_hosts.node_uuid",
                compact_str::format_compact!("{prefix}node_uuid"),
            ),
            (
                "node_database_agent_hosts.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ]);

        columns.extend(super::database_agent_host::DatabaseAgentHost::base_columns(
            Some("database_agent_host_"),
        ));

        columns
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            node: super::node::Node::get_fetchable(
                row.try_get(compact_str::format_compact!("{prefix}node_uuid").as_str())?,
            ),
            database_agent_host: super::database_agent_host::DatabaseAgentHost::map(
                Some("database_agent_host_"),
                row,
            )?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

impl NodeDatabaseAgentHost {
    pub async fn by_node_uuid_database_agent_host_uuid(
        database: &crate::database::Database,
        node_uuid: uuid::Uuid,
        database_agent_host_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM node_database_agent_hosts
            JOIN database_agent_hosts ON node_database_agent_hosts.database_agent_host_uuid = database_agent_hosts.uuid
            WHERE node_database_agent_hosts.node_uuid = $1 AND node_database_agent_hosts.database_agent_host_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(database_agent_host_uuid)
        .fetch_optional(database.read())
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    pub async fn by_node_uuid_database_agent_host_uuid_with_transaction(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        node_uuid: uuid::Uuid,
        database_agent_host_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM node_database_agent_hosts
            JOIN database_agent_hosts ON node_database_agent_hosts.database_agent_host_uuid = database_agent_hosts.uuid
            WHERE node_database_agent_hosts.node_uuid = $1 AND node_database_agent_hosts.database_agent_host_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(database_agent_host_uuid)
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
                FROM node_database_agent_hosts
                JOIN database_agent_hosts ON node_database_agent_hosts.database_agent_host_uuid = database_agent_hosts.uuid
                WHERE node_database_agent_hosts.node_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["database_agent_hosts.name"], &["database_agent_hosts.uuid"]),
            ),
            "node_database_agent_hosts.created, node_database_agent_hosts.node_uuid, node_database_agent_hosts.database_agent_host_uuid",
        )
        .bind(node_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for NodeDatabaseAgentHost {
    type AdminApiObject = AdminApiNodeDatabaseAgentHost;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiNodeDatabaseAgentHost::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiNodeDatabaseAgentHost {
                database_agent_host: self
                    .database_agent_host
                    .into_admin_api_object(state, ())
                    .await?,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreateNodeDatabaseAgentHostOptions {
    #[garde(skip)]
    pub node_uuid: uuid::Uuid,
    #[garde(skip)]
    pub database_agent_host_uuid: uuid::Uuid,
}

#[async_trait::async_trait]
impl CreatableModel for NodeDatabaseAgentHost {
    type CreateOptions<'a> = CreateNodeDatabaseAgentHostOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<NodeDatabaseAgentHost>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        super::database_agent_host::DatabaseAgentHost::by_uuid_optional_cached(
            &state.database,
            options.database_agent_host_uuid,
        )
        .await?
        .ok_or(crate::database::InvalidRelationError("database_agent_host"))?;

        let mut query_builder = InsertQueryBuilder::new("node_database_agent_hosts");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("node_uuid", options.node_uuid)
            .set("database_agent_host_uuid", options.database_agent_host_uuid);

        query_builder.execute(&mut **transaction).await?;

        let mut result = match Self::by_node_uuid_database_agent_host_uuid_with_transaction(
            transaction,
            options.node_uuid,
            options.database_agent_host_uuid,
        )
        .await?
        {
            Some(node_database_agent_host) => node_database_agent_host,
            None => return Err(sqlx::Error::RowNotFound.into()),
        };

        Self::run_after_create_handlers(&mut result, &options, state, transaction).await?;

        Ok(result)
    }
}

#[async_trait::async_trait]
impl DeletableModel for NodeDatabaseAgentHost {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<NodeDatabaseAgentHost>> =
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
            DELETE FROM node_database_agent_hosts
            WHERE node_database_agent_hosts.node_uuid = $1 AND node_database_agent_hosts.database_agent_host_uuid = $2
            "#,
        )
        .bind(self.node.uuid)
        .bind(self.database_agent_host.uuid)
        .execute(&mut **transaction)
        .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[schema_extension_derive::extendible]
#[init_args(NodeDatabaseAgentHost, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "NodeDatabaseAgentHost")]
pub struct AdminApiNodeDatabaseAgentHost {
    pub database_agent_host: super::database_agent_host::AdminApiDatabaseAgentHost,

    pub created: chrono::DateTime<chrono::Utc>,
}
