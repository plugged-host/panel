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
pub struct Location {
    pub uuid: uuid::Uuid,
    pub backup_configuration: Option<Fetchable<super::backup_configuration::BackupConfiguration>>,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub flag: Option<compact_str::CompactString>,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for Location {
    const NAME: &'static str = "location";

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
                "locations.uuid",
                compact_str::format_compact!("{prefix}uuid"),
            ),
            (
                "locations.backup_configuration_uuid",
                compact_str::format_compact!("{prefix}location_backup_configuration_uuid"),
            ),
            (
                "locations.name",
                compact_str::format_compact!("{prefix}name"),
            ),
            (
                "locations.description",
                compact_str::format_compact!("{prefix}description"),
            ),
            (
                "locations.flag",
                compact_str::format_compact!("{prefix}flag"),
            ),
            (
                "locations.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ])
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            backup_configuration:
                super::backup_configuration::BackupConfiguration::get_fetchable_from_row(
                    row,
                    compact_str::format_compact!("{prefix}location_backup_configuration_uuid"),
                ),
            name: row.try_get(compact_str::format_compact!("{prefix}name").as_str())?,
            description: row
                .try_get(compact_str::format_compact!("{prefix}description").as_str())?,
            flag: row.try_get(compact_str::format_compact!("{prefix}flag").as_str())?,
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

impl Location {
    pub async fn by_backup_configuration_uuid_with_pagination(
        database: &crate::database::Database,
        backup_configuration_uuid: uuid::Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<super::Pagination<Self>, crate::database::DatabaseError> {
        super::PaginatedQueryBuilder::new(
            Self::columns_sql(None),
            format!(
                r#"
                FROM locations
                WHERE locations.backup_configuration_uuid = $1 AND {search}
                "#,
                search = super::search_sql(2, &["locations.name"], &["locations.uuid"]),
            ),
            "locations.created, locations.uuid",
        )
        .bind(backup_configuration_uuid)
        .bind(search)
        .fetch(database, page, per_page)
        .await
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
                FROM locations
                WHERE {search}
                "#,
                search = super::search_sql(1, &["locations.name"], &["locations.uuid"]),
            ),
            "locations.created, locations.uuid",
        )
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for Location {
    type AdminApiObject = AdminApiLocation;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiLocation::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiLocation {
                uuid: self.uuid,
                backup_configuration: if let Some(backup_configuration) = self.backup_configuration
                {
                    if let Ok(backup_configuration) =
                        backup_configuration.fetch_cached(&state.database).await
                    {
                        backup_configuration
                            .into_admin_api_object(state, ())
                            .await
                            .ok()
                    } else {
                        None
                    }
                } else {
                    None
                },
                name: self.name,
                description: self.description,
                flag: self.flag,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[async_trait::async_trait]
impl ByUuid for Location {
    async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Self, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM locations
            WHERE locations.uuid = $1
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
            FROM locations
            WHERE locations.uuid = $1
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
pub struct CreateLocationOptions {
    #[garde(skip)]
    pub backup_configuration_uuid: Option<uuid::Uuid>,
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub name: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 1024))]
    #[schema(min_length = 1, max_length = 1024)]
    pub description: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 2, max = 2))]
    #[schema(min_length = 2, max_length = 2)]
    pub flag: Option<compact_str::CompactString>,
}

#[async_trait::async_trait]
impl CreatableModel for Location {
    type CreateOptions<'a> = CreateLocationOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<Location>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        if let Some(backup_configuration_uuid) = &options.backup_configuration_uuid {
            super::backup_configuration::BackupConfiguration::by_uuid_optional_cached(
                &state.database,
                *backup_configuration_uuid,
            )
            .await?
            .ok_or(crate::database::InvalidRelationError(
                "backup_configuration",
            ))?;
        }

        let mut query_builder = InsertQueryBuilder::new("locations");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set(
                "backup_configuration_uuid",
                options.backup_configuration_uuid,
            )
            .set("name", &options.name)
            .set("description", &options.description)
            .set("flag", &options.flag);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut location = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut location, &options, state, transaction).await?;

        Ok(location)
    }
}

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Default)]
pub struct UpdateLocationOptions {
    #[garde(skip)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub backup_configuration_uuid: Option<Option<uuid::Uuid>>,
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
    #[garde(length(chars, min = 2, max = 2))]
    #[schema(min_length = 2, max_length = 2)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub flag: Option<Option<compact_str::CompactString>>,
}

#[async_trait::async_trait]
impl UpdatableModel for Location {
    type UpdateOptions = UpdateLocationOptions;

    fn get_update_handlers() -> &'static LazyLock<UpdateHandlerList<Self>> {
        static UPDATE_LISTENERS: LazyLock<UpdateHandlerList<Location>> =
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

        let backup_configuration =
            if let Some(backup_configuration_uuid) = &options.backup_configuration_uuid {
                match backup_configuration_uuid {
                    Some(uuid) => {
                        super::backup_configuration::BackupConfiguration::by_uuid_optional_cached(
                            &state.database,
                            *uuid,
                        )
                        .await?
                        .ok_or(crate::database::InvalidRelationError(
                            "backup_configuration",
                        ))?;

                        Some(Some(
                            super::backup_configuration::BackupConfiguration::get_fetchable(*uuid),
                        ))
                    }
                    None => Some(None),
                }
            } else {
                None
            };

        let mut query_builder = UpdateQueryBuilder::new("locations");

        self.run_update_handlers(&mut options, &mut query_builder, state, transaction)
            .await?;

        query_builder
            .set(
                "backup_configuration_uuid",
                options.backup_configuration_uuid.as_ref(),
            )
            .set("name", options.name.as_ref())
            .set("description", options.description.as_ref())
            .set("flag", options.flag.as_ref())
            .where_eq("uuid", self.uuid);

        query_builder.execute(&mut **transaction).await?;

        if let Some(backup_configuration) = backup_configuration {
            self.backup_configuration = backup_configuration;
        }
        if let Some(name) = options.name {
            self.name = name;
        }
        if let Some(description) = options.description {
            self.description = description;
        }
        if let Some(flag) = options.flag {
            self.flag = flag;
        }

        self.run_after_update_handlers(state, transaction).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeletableModel for Location {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<Location>> =
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
            DELETE FROM locations
            WHERE locations.uuid = $1
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
pub struct DuplicateLocationOptions {
    #[garde(length(chars, min = 1, max = 255))]
    pub name: compact_str::CompactString,
}

#[async_trait::async_trait]
impl DuplicableModel for Location {
    type DuplicateOptions<'a> = DuplicateLocationOptions;
    type DuplicateResult = Self;

    fn get_duplicate_handlers() -> &'static LazyLock<DuplicateHandlerList<Self>> {
        static DUPLICATE_LISTENERS: LazyLock<DuplicateHandlerList<Location>> =
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

        let mut query_builder = InsertQueryBuilder::new("locations");

        query_builder
            .set(
                "backup_configuration_uuid",
                self.backup_configuration.as_ref().map(|c| c.uuid),
            )
            .set("name", &options.name)
            .set("description", &self.description)
            .set("flag", &self.flag);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut location = Self::map(None, &row)?;

        sqlx::query!(
            "INSERT INTO location_database_hosts (location_uuid, database_host_uuid)
            SELECT $1, location_database_hosts.database_host_uuid
            FROM location_database_hosts
            WHERE location_database_hosts.location_uuid = $2",
            location.uuid,
            self.uuid,
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "INSERT INTO location_database_agent_hosts (location_uuid, database_agent_host_uuid)
            SELECT $1, location_database_agent_hosts.database_agent_host_uuid
            FROM location_database_agent_hosts
            WHERE location_database_agent_hosts.location_uuid = $2",
            location.uuid,
            self.uuid,
        )
        .execute(&mut **transaction)
        .await?;

        self.run_after_duplicate_handlers(&mut location, &options, state, transaction)
            .await?;

        Ok(location)
    }
}

#[schema_extension_derive::extendible]
#[init_args(Location, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "Location")]
pub struct AdminApiLocation {
    pub uuid: uuid::Uuid,
    pub backup_configuration: Option<super::backup_configuration::AdminApiBackupConfiguration>,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub flag: Option<compact_str::CompactString>,

    pub created: chrono::DateTime<chrono::Utc>,
}
