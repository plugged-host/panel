use crate::{
    models::{InsertQueryBuilder, UpdateQueryBuilder},
    prelude::*,
};
use garde::Validate;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone)]
pub struct Device {
    pub uuid: uuid::Uuid,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub source: compact_str::CompactString,
    pub target: compact_str::CompactString,

    pub permissions: compact_str::CompactString,
    pub user_attachable: bool,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for Device {
    const NAME: &'static str = "device";

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
            ("devices.uuid", compact_str::format_compact!("{prefix}uuid")),
            ("devices.name", compact_str::format_compact!("{prefix}name")),
            (
                "devices.description",
                compact_str::format_compact!("{prefix}description"),
            ),
            (
                "devices.source",
                compact_str::format_compact!("{prefix}source"),
            ),
            (
                "devices.target",
                compact_str::format_compact!("{prefix}target"),
            ),
            (
                "devices.permissions",
                compact_str::format_compact!("{prefix}permissions"),
            ),
            (
                "devices.user_attachable",
                compact_str::format_compact!("{prefix}user_attachable"),
            ),
            (
                "devices.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ])
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            name: row.try_get(compact_str::format_compact!("{prefix}name").as_str())?,
            description: row
                .try_get(compact_str::format_compact!("{prefix}description").as_str())?,
            source: row.try_get(compact_str::format_compact!("{prefix}source").as_str())?,
            target: row.try_get(compact_str::format_compact!("{prefix}target").as_str())?,
            permissions: row
                .try_get(compact_str::format_compact!("{prefix}permissions").as_str())?,
            user_attachable: row
                .try_get(compact_str::format_compact!("{prefix}user_attachable").as_str())?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
            extension_data: Self::map_extensions(prefix, row)?,
        })
    }

    fn cache_invalidation_keys(&self) -> Vec<compact_str::CompactString> {
        vec![compact_str::format_compact!(
            "{}::{}",
            Self::NAME,
            self.uuid
        )]
    }
}

impl Device {
    pub async fn by_node_uuid_egg_uuid_uuid(
        database: &crate::database::Database,
        node_uuid: uuid::Uuid,
        egg_uuid: uuid::Uuid,
        uuid: uuid::Uuid,
    ) -> Result<Option<Self>, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM devices
            JOIN node_devices ON devices.uuid = node_devices.device_uuid
            JOIN nest_egg_devices ON devices.uuid = nest_egg_devices.device_uuid
            WHERE node_devices.node_uuid = $1 AND nest_egg_devices.egg_uuid = $2 AND devices.uuid = $3
            "#,
            Self::columns_sql(None)
        )))
        .bind(node_uuid)
        .bind(egg_uuid)
        .bind(uuid)
        .fetch_optional(database.read())
        .await?;

        row.try_map(|row| Self::map(None, &row))
    }

    pub async fn all_with_pagination(
        database: &crate::database::Database,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!(
                r#"
                FROM devices
                WHERE {search}
                "#,
                search = super::search_sql(1, &["devices.name"], &["devices.uuid"]),
            ),
            "devices.created, devices.uuid",
        )
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for Device {
    type AdminApiObject = AdminApiDevice;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiDevice::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiDevice {
                uuid: self.uuid,
                name: self.name,
                description: self.description,
                source: self.source,
                target: self.target,
                permissions: self.permissions,
                user_attachable: self.user_attachable,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[async_trait::async_trait]
impl ByUuid for Device {
    async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Self, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM devices
            WHERE devices.uuid = $1
            "#,
            Self::columns_sql(None)
        )))
        .bind(uuid)
        .fetch_one(database.read())
        .await?;

        Self::map(None, &row)
    }

    async fn by_uuid_with_transaction(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        uuid: uuid::Uuid,
    ) -> Result<Self, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM devices
            WHERE devices.uuid = $1
            "#,
            Self::columns_sql(None)
        )))
        .bind(uuid)
        .fetch_one(&mut **transaction)
        .await?;

        Self::map(None, &row)
    }
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreateDeviceOptions {
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub name: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 1024))]
    #[schema(min_length = 1, max_length = 1024)]
    pub description: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub source: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub target: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 255), pattern("^[rwm]+$"))]
    #[schema(min_length = 1, max_length = 255, pattern = "^[rwm]+$")]
    pub permissions: compact_str::CompactString,
    #[garde(skip)]
    pub user_attachable: bool,
}

#[async_trait::async_trait]
impl CreatableModel for Device {
    type CreateOptions<'a> = CreateDeviceOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<Device>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        let mut query_builder = InsertQueryBuilder::new("devices");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("name", &options.name)
            .set("description", &options.description)
            .set("source", &options.source)
            .set("target", &options.target)
            .set("permissions", &options.permissions)
            .set("user_attachable", options.user_attachable);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut device = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut device, &options, state, transaction).await?;

        Ok(device)
    }
}

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Default)]
pub struct UpdateDeviceOptions {
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub name: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 1, max = 1024))]
    #[schema(min_length = 1, max_length = 1024)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub description: Option<Option<compact_str::CompactString>>,
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub source: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub target: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 1, max = 255), pattern("^[rwm]+$"))]
    #[schema(min_length = 1, max_length = 255, pattern = "^[rwm]+$")]
    pub permissions: Option<compact_str::CompactString>,
    #[garde(skip)]
    pub user_attachable: Option<bool>,
}

#[async_trait::async_trait]
impl UpdatableModel for Device {
    type UpdateOptions = UpdateDeviceOptions;

    fn get_update_handlers() -> &'static LazyLock<UpdateHandlerList<Self>> {
        static UPDATE_LISTENERS: LazyLock<UpdateHandlerList<Device>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &UPDATE_LISTENERS
    }

    async fn update_with_transaction(
        &mut self,
        state: &crate::State,
        mut options: Self::UpdateOptions,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), crate::database::DatabaseError> {
        options.validate()?;

        let mut query_builder = UpdateQueryBuilder::new("devices");

        self.run_update_handlers(&mut options, &mut query_builder, state, transaction)
            .await?;

        query_builder
            .set("name", options.name.as_ref())
            .set(
                "description",
                options.description.as_ref().map(|d| d.as_ref()),
            )
            .set("source", options.source.as_ref())
            .set("target", options.target.as_ref())
            .set("permissions", options.permissions.as_ref())
            .set("user_attachable", options.user_attachable)
            .where_eq("uuid", self.uuid);

        query_builder.execute(&mut **transaction).await?;

        if let Some(name) = options.name {
            self.name = name;
        }
        if let Some(description) = options.description {
            self.description = description;
        }
        if let Some(source) = options.source {
            self.source = source;
        }
        if let Some(target) = options.target {
            self.target = target;
        }
        if let Some(permissions) = options.permissions {
            self.permissions = permissions;
        }
        if let Some(user_attachable) = options.user_attachable {
            self.user_attachable = user_attachable;
        }

        self.run_after_update_handlers(state, transaction).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeletableModel for Device {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<Device>> =
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
            DELETE FROM devices
            WHERE devices.uuid = $1
            "#,
        )
        .bind(self.uuid)
        .execute(&mut **transaction)
        .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[derive(Validate)]
pub struct DuplicateDeviceOptions {
    #[garde(length(chars, min = 1, max = 255))]
    pub name: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 255))]
    pub source: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 255))]
    pub target: compact_str::CompactString,
}

#[async_trait::async_trait]
impl DuplicableModel for Device {
    type DuplicateOptions<'a> = DuplicateDeviceOptions;
    type DuplicateResult = Self;

    fn get_duplicate_handlers() -> &'static LazyLock<DuplicateHandlerList<Self>> {
        static DUPLICATE_LISTENERS: LazyLock<DuplicateHandlerList<Device>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &DUPLICATE_LISTENERS
    }

    async fn duplicate_with_transaction(
        &self,
        state: &crate::State,
        options: Self::DuplicateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        self.run_duplicate_handlers(&options, state, transaction)
            .await?;

        let mut query_builder = InsertQueryBuilder::new("devices");

        query_builder
            .set("name", &options.name)
            .set("description", &self.description)
            .set("source", &options.source)
            .set("target", &options.target)
            .set("permissions", &self.permissions)
            .set("user_attachable", self.user_attachable);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut device = Self::map(None, &row)?;

        self.run_after_duplicate_handlers(&mut device, &options, state, transaction)
            .await?;

        Ok(device)
    }
}

#[schema_extension_derive::extendible]
#[init_args(Device, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "Device")]
pub struct AdminApiDevice {
    pub uuid: uuid::Uuid,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub source: compact_str::CompactString,
    pub target: compact_str::CompactString,

    pub permissions: compact_str::CompactString,
    pub user_attachable: bool,

    pub created: chrono::DateTime<chrono::Utc>,
}
