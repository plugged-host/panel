use crate::{models::InsertQueryBuilder, prelude::*};
use garde::Validate;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize)]
pub struct NodeDevice {
    pub device: Fetchable<super::device::Device>,
    pub node: Fetchable<super::node::Node>,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for NodeDevice {
    const NAME: &'static str = "node_device";

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
                "node_devices.device_uuid",
                compact_str::format_compact!("{prefix}device_uuid"),
            ),
            (
                "node_devices.node_uuid",
                compact_str::format_compact!("{prefix}node_uuid"),
            ),
            (
                "node_devices.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ])
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            device: super::device::Device::get_fetchable(
                row.try_get(compact_str::format_compact!("{prefix}device_uuid").as_str())?,
            ),
            node: super::node::Node::get_fetchable(
                row.try_get(compact_str::format_compact!("{prefix}node_uuid").as_str())?,
            ),
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

impl NodeDevice {
    pub async fn by_node_uuid_device_uuid(
        database: &crate::database::Database,
        node_uuid: uuid::Uuid,
        device_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM node_devices
            WHERE node_devices.node_uuid = $1 AND node_devices.device_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(device_uuid)
        .fetch_optional(database.read())
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
                FROM node_devices
                JOIN devices ON devices.uuid = node_devices.device_uuid
                WHERE node_devices.node_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["devices.name"], &["devices.uuid"]),
            ),
            "node_devices.created, node_devices.node_uuid, node_devices.device_uuid",
        )
        .bind(node_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }

    pub async fn by_device_uuid_with_pagination(
        database: &crate::database::Database,
        device_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!(
                r#"
                FROM node_devices
                JOIN nodes ON nodes.uuid = node_devices.node_uuid
                WHERE node_devices.device_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["nodes.name"], &["nodes.uuid"]),
            ),
            "node_devices.created, node_devices.node_uuid, node_devices.device_uuid",
        )
        .bind(device_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }

    #[inline]
    pub async fn into_admin_node_api_object(
        self,
        state: &crate::State,
        _args: (),
    ) -> Result<AdminApiNodeNodeDevice, crate::database::DatabaseError> {
        Ok(AdminApiNodeNodeDevice {
            node: self
                .node
                .fetch_cached(&state.database)
                .await?
                .into_admin_api_object(state, ())
                .await?,
            created: self.created.and_utc(),
        })
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for NodeDevice {
    type AdminApiObject = AdminApiNodeDevice;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiNodeDevice::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiNodeDevice {
                device: self
                    .device
                    .fetch_cached(&state.database)
                    .await?
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
pub struct CreateNodeDeviceOptions {
    #[garde(skip)]
    pub node_uuid: uuid::Uuid,
    #[garde(skip)]
    pub device_uuid: uuid::Uuid,
}

#[async_trait::async_trait]
impl CreatableModel for NodeDevice {
    type CreateOptions<'a> = CreateNodeDeviceOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<NodeDevice>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        super::device::Device::by_uuid_optional_cached(&state.database, options.device_uuid)
            .await?
            .ok_or(crate::database::InvalidRelationError("device"))?;

        let mut query_builder = InsertQueryBuilder::new("node_devices");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("node_uuid", options.node_uuid)
            .set("device_uuid", options.device_uuid);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut node_device = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut node_device, &options, state, transaction).await?;

        Ok(node_device)
    }
}

#[async_trait::async_trait]
impl DeletableModel for NodeDevice {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<NodeDevice>> =
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
            DELETE FROM node_devices
            WHERE node_devices.node_uuid = $1 AND node_devices.device_uuid = $2
            "#,
        )
        .bind(self.node.uuid)
        .bind(self.device.uuid)
        .execute(&mut **transaction)
        .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[derive(ToSchema, Serialize)]
#[schema(title = "AdminNodeNodeDevice")]
pub struct AdminApiNodeNodeDevice {
    pub node: super::node::AdminApiNode,

    pub created: chrono::DateTime<chrono::Utc>,
}

#[schema_extension_derive::extendible]
#[init_args(NodeDevice, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "AdminNodeDevice")]
pub struct AdminApiNodeDevice {
    pub device: super::device::AdminApiDevice,

    pub created: chrono::DateTime<chrono::Utc>,
}
