// plugged.host: resource presets offered when a player creates a server (docs/features/create-server.md).

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

/// A plan: CPU in percent of one thread, memory and disk in MiB. Players pick one when they create
/// a server; it has to fit in what is left of their allowance ([`crate::allowance`]).
#[derive(Serialize, Deserialize, Clone)]
pub struct Plan {
    pub uuid: uuid::Uuid,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,

    pub sort: i32,
    pub public: bool,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for Plan {
    const NAME: &'static str = "plan";

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
            ("plans.uuid", compact_str::format_compact!("{prefix}uuid")),
            ("plans.name", compact_str::format_compact!("{prefix}name")),
            (
                "plans.description",
                compact_str::format_compact!("{prefix}description"),
            ),
            ("plans.cpu", compact_str::format_compact!("{prefix}cpu")),
            (
                "plans.memory",
                compact_str::format_compact!("{prefix}memory"),
            ),
            ("plans.disk", compact_str::format_compact!("{prefix}disk")),
            ("plans.sort", compact_str::format_compact!("{prefix}sort")),
            (
                "plans.public",
                compact_str::format_compact!("{prefix}public"),
            ),
            (
                "plans.created",
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
            cpu: row.try_get(compact_str::format_compact!("{prefix}cpu").as_str())?,
            memory: row.try_get(compact_str::format_compact!("{prefix}memory").as_str())?,
            disk: row.try_get(compact_str::format_compact!("{prefix}disk").as_str())?,
            sort: row.try_get(compact_str::format_compact!("{prefix}sort").as_str())?,
            public: row.try_get(compact_str::format_compact!("{prefix}public").as_str())?,
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

impl Plan {
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
                FROM plans
                WHERE {search}
                "#,
                search = super::search_sql(1, &["plans.name"], &["plans.uuid"]),
            ),
            "plans.sort, plans.memory, plans.uuid",
        )
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }

    /// The plans players can pick, in their order.
    pub async fn all_public(
        database: &crate::database::Database,
    ) -> Result<Vec<Self>, crate::database::DatabaseError> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM plans
            WHERE plans.public
            ORDER BY plans.sort, plans.memory, plans.uuid
            "#,
            Self::columns_sql(None)
        )))
        .fetch_all(database.read())
        .await?;

        rows.into_iter().map(|row| Self::map(None, &row)).collect()
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for Plan {
    type AdminApiObject = AdminApiPlan;
    type ExtraArgs<'a> = ();

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        _args: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiPlan::init_hooks(&self, state).await?;

        let api_object = finish_extendible!(
            AdminApiPlan {
                uuid: self.uuid,
                name: self.name,
                description: self.description,
                cpu: self.cpu,
                memory: self.memory,
                disk: self.disk,
                sort: self.sort,
                public: self.public,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[async_trait::async_trait]
impl ByUuid for Plan {
    async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Self, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM plans
            WHERE plans.uuid = $1
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
            FROM plans
            WHERE plans.uuid = $1
            "#,
            Self::columns_sql(None)
        )))
        .bind(uuid)
        .fetch_one(&mut **transaction)
        .await?;

        Self::map(None, &row)
    }
}

fn default_public() -> bool {
    true
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreatePlanOptions {
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub name: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 1024))]
    #[schema(min_length = 1, max_length = 1024)]
    pub description: Option<compact_str::CompactString>,
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    pub cpu: i32,
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    pub memory: i64,
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    pub disk: i64,
    #[garde(skip)]
    #[serde(default)]
    pub sort: i32,
    #[garde(skip)]
    #[serde(default = "default_public")]
    pub public: bool,
}

#[async_trait::async_trait]
impl CreatableModel for Plan {
    type CreateOptions<'a> = CreatePlanOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<Plan>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        let mut query_builder = InsertQueryBuilder::new("plans");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("name", &options.name)
            .set("description", &options.description)
            .set("cpu", options.cpu)
            .set("memory", options.memory)
            .set("disk", options.disk)
            .set("sort", options.sort)
            .set("public", options.public);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut plan = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut plan, &options, state, transaction).await?;

        Ok(plan)
    }
}

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Default)]
pub struct UpdatePlanOptions {
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
    #[garde(inner(range(min = 1)))]
    #[schema(minimum = 1)]
    pub cpu: Option<i32>,
    #[garde(inner(range(min = 1)))]
    #[schema(minimum = 1)]
    pub memory: Option<i64>,
    #[garde(inner(range(min = 1)))]
    #[schema(minimum = 1)]
    pub disk: Option<i64>,
    #[garde(skip)]
    pub sort: Option<i32>,
    #[garde(skip)]
    pub public: Option<bool>,
}

#[async_trait::async_trait]
impl UpdatableModel for Plan {
    type UpdateOptions = UpdatePlanOptions;

    fn get_update_handlers() -> &'static LazyLock<UpdateHandlerList<Self>> {
        static UPDATE_LISTENERS: LazyLock<UpdateHandlerList<Plan>> =
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

        let mut query_builder = UpdateQueryBuilder::new("plans");

        self.run_update_handlers(&mut options, &mut query_builder, state, transaction)
            .await?;

        query_builder
            .set("name", options.name.as_ref())
            .set("description", options.description.as_ref())
            .set("cpu", options.cpu)
            .set("memory", options.memory)
            .set("disk", options.disk)
            .set("sort", options.sort)
            .set("public", options.public)
            .where_eq("uuid", self.uuid);

        query_builder.execute(&mut **transaction).await?;

        if let Some(name) = options.name {
            self.name = name;
        }
        if let Some(description) = options.description {
            self.description = description;
        }
        if let Some(cpu) = options.cpu {
            self.cpu = cpu;
        }
        if let Some(memory) = options.memory {
            self.memory = memory;
        }
        if let Some(disk) = options.disk {
            self.disk = disk;
        }
        if let Some(sort) = options.sort {
            self.sort = sort;
        }
        if let Some(public) = options.public {
            self.public = public;
        }

        self.run_after_update_handlers(state, transaction).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeletableModel for Plan {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<Plan>> =
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

        sqlx::query("DELETE FROM plans WHERE plans.uuid = $1")
            .bind(self.uuid)
            .execute(&mut **transaction)
            .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

#[schema_extension_derive::extendible]
#[init_args(Plan, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "Plan")]
pub struct AdminApiPlan {
    pub uuid: uuid::Uuid,

    pub name: compact_str::CompactString,
    pub description: Option<compact_str::CompactString>,

    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,

    pub sort: i32,
    pub public: bool,

    pub created: chrono::DateTime<chrono::Utc>,
}
