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
pub struct NestEggDevice {
    pub device: Fetchable<super::device::Device>,
    pub nest_egg: Fetchable<super::nest_egg::NestEgg>,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for NestEggDevice {
    const NAME: &'static str = "nest_egg_device";

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
                "nest_egg_devices.device_uuid",
                compact_str::format_compact!("{prefix}device_uuid"),
            ),
            (
                "nest_egg_devices.egg_uuid",
                compact_str::format_compact!("{prefix}egg_uuid"),
            ),
            (
                "nest_egg_devices.created",
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
            nest_egg: super::nest_egg::NestEgg::get_fetchable(
                row.try_get(compact_str::format_compact!("{prefix}egg_uuid").as_str())?,
            ),
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }
}

impl NestEggDevice {
    pub async fn by_egg_uuid_device_uuid(
        database: &crate::database::Database,
        egg_uuid: uuid::Uuid,
        device_uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM nest_egg_devices
            WHERE nest_egg_devices.egg_uuid = $1 AND nest_egg_devices.device_uuid = $2
            "#,
            Self::columns_sql(None)
        )))
        .bind(egg_uuid)
        .bind(device_uuid)
        .fetch_optional(database.read())
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    pub async fn by_egg_uuid_with_pagination(
        database: &crate::database::Database,
        egg_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!(
                r#"
                FROM nest_egg_devices
                JOIN devices ON devices.uuid = nest_egg_devices.device_uuid
                WHERE nest_egg_devices.egg_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["devices.name"], &["devices.uuid"]),
            ),
            "nest_egg_devices.created, nest_egg_devices.egg_uuid, nest_egg_devices.device_uuid",
        )
        .bind(egg_uuid)
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
                FROM nest_egg_devices
                JOIN nest_eggs ON nest_eggs.uuid = nest_egg_devices.egg_uuid
                WHERE nest_egg_devices.device_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["nest_eggs.name"], &["nest_eggs.uuid"]),
            ),
            "nest_egg_devices.created, nest_egg_devices.egg_uuid, nest_egg_devices.device_uuid",
        )
        .bind(device_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }

    pub async fn into_admin_nest_egg_api_object(
        self,
        state: &crate::State,
        _args: (),
    ) -> Result<AdminApiNestEggNestEggDevice, crate::database::DatabaseError> {
        let nest_egg = self.nest_egg.fetch_cached(&state.database).await?;
        let nest = nest_egg.nest.clone();
        let (nest, nest_egg) = tokio::try_join!(nest.fetch_cached(&state.database), async {
            Ok(nest_egg.into_admin_api_object(state, ()).await?)
        })?;

        Ok(AdminApiNestEggNestEggDevice {
            nest: nest.into_admin_api_object(state, ()).await?,
            nest_egg,
            created: self.created.and_utc(),
        })
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for NestEggDevice {
    type AdminApiObject = AdminApiNestEggDevice;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiNestEggDevice::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiNestEggDevice {
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
pub struct CreateNestEggDeviceOptions {
    #[garde(skip)]
    pub egg_uuid: uuid::Uuid,
    #[garde(skip)]
    pub device_uuid: uuid::Uuid,
}

#[async_trait::async_trait]
impl CreatableModel for NestEggDevice {
    type CreateOptions<'a> = CreateNestEggDeviceOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<NestEggDevice>> =
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

        let mut query_builder = InsertQueryBuilder::new("nest_egg_devices");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("egg_uuid", options.egg_uuid)
            .set("device_uuid", options.device_uuid);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut nest_egg_device = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut nest_egg_device, &options, state, transaction).await?;

        Ok(nest_egg_device)
    }
}

#[async_trait::async_trait]
impl DeletableModel for NestEggDevice {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<NestEggDevice>> =
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
            DELETE FROM nest_egg_devices
            WHERE nest_egg_devices.egg_uuid = $1 AND nest_egg_devices.device_uuid = $2
            "#,
        )
        .bind(self.nest_egg.uuid)
        .bind(self.device.uuid)
        .execute(&mut **transaction)
        .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[derive(ToSchema, Serialize)]
#[schema(title = "AdminNestEggNestEggDevice")]
pub struct AdminApiNestEggNestEggDevice {
    pub nest: super::nest::AdminApiNest,
    pub nest_egg: super::nest_egg::AdminApiNestEgg,

    pub created: chrono::DateTime<chrono::Utc>,
}

#[schema_extension_derive::extendible]
#[init_args(NestEggDevice, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "AdminNestEggDevice")]
pub struct AdminApiNestEggDevice {
    pub device: super::device::AdminApiDevice,

    pub created: chrono::DateTime<chrono::Utc>,
}
