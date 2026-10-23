// plugged.host: sponsored banners, sold by the panel's owner and shown in the ad slots (docs/features/ads.md#sponsored-banners).

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

/// Where a banner shows.
#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdBannerPlacement {
    /// In place of the Adsterra unit of its size.
    Slot,
    /// Full width above every page.
    Top,
}

impl AdBannerPlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slot => "slot",
            Self::Top => "top",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "top" => Self::Top,
            _ => Self::Slot,
        }
    }
}

/// The size a slot banner of `width`×`height` pixels is laid out at: an Adsterra size, or half of
/// twice one (a sharp image on dense screens).
pub fn slot_size(width: i32, height: i32) -> Option<(i32, i32)> {
    if crate::ads::SIZES.contains(&(width, height)) {
        Some((width, height))
    } else if width % 2 == 0
        && height % 2 == 0
        && crate::ads::SIZES.contains(&(width / 2, height / 2))
    {
        Some((width / 2, height / 2))
    } else {
        None
    }
}

/// An uploaded banner image: its file type and size in pixels.
pub struct BannerImage {
    pub extension: &'static str,
    pub content_type: &'static str,
    pub width: i32,
    pub height: i32,
}

/// Reads an upload: a PNG, JPEG, WebP or GIF; a slot banner of an Adsterra size or twice one.
pub fn read_image(bytes: &[u8], placement: AdBannerPlacement) -> Result<BannerImage, &'static str> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "the file is not an image")?;
    let (extension, content_type) = match reader.format() {
        Some(image::ImageFormat::Png) => ("png", "image/png"),
        Some(image::ImageFormat::Jpeg) => ("jpg", "image/jpeg"),
        Some(image::ImageFormat::WebP) => ("webp", "image/webp"),
        Some(image::ImageFormat::Gif) => ("gif", "image/gif"),
        _ => return Err("the image must be a PNG, JPEG, WebP or GIF"),
    };
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| "the image could not be read")?;
    let (width, height) = (
        width.min(i32::MAX as u32) as i32,
        height.min(i32::MAX as u32) as i32,
    );
    if placement == AdBannerPlacement::Slot && slot_size(width, height).is_none() {
        return Err(
            "a slot banner must have an adsterra size (728x90, 468x60, 320x50, 300x250, 160x600, 160x300) or twice one",
        );
    }

    Ok(BannerImage {
        extension,
        content_type,
        width,
        height,
    })
}

/// A link that may go on a banner: `https://` and a host.
pub fn valid_link(link: &str) -> bool {
    reqwest::Url::parse(link).is_ok_and(|url| {
        url.scheme() == "https" && url.host_str().is_some_and(|host| host.contains('.'))
    })
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AdBanner {
    pub uuid: uuid::Uuid,

    pub advertiser: compact_str::CompactString,
    pub link: compact_str::CompactString,
    pub placement: AdBannerPlacement,

    /// The image's path in the storage, its size in pixels.
    pub image: Option<compact_str::CompactString>,
    pub width: i32,
    pub height: i32,

    pub starts: Option<chrono::NaiveDateTime>,
    pub ends: Option<chrono::NaiveDateTime>,
    pub active: bool,
    pub clicks: i64,

    pub created: chrono::NaiveDateTime,

    extension_data: super::ModelExtensionData,
}

impl BaseModel for AdBanner {
    const NAME: &'static str = "ad_banner";

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
                "ad_banners.uuid",
                compact_str::format_compact!("{prefix}uuid"),
            ),
            (
                "ad_banners.advertiser",
                compact_str::format_compact!("{prefix}advertiser"),
            ),
            (
                "ad_banners.link",
                compact_str::format_compact!("{prefix}link"),
            ),
            (
                "ad_banners.placement",
                compact_str::format_compact!("{prefix}placement"),
            ),
            (
                "ad_banners.image",
                compact_str::format_compact!("{prefix}image"),
            ),
            (
                "ad_banners.width",
                compact_str::format_compact!("{prefix}width"),
            ),
            (
                "ad_banners.height",
                compact_str::format_compact!("{prefix}height"),
            ),
            (
                "ad_banners.starts",
                compact_str::format_compact!("{prefix}starts"),
            ),
            (
                "ad_banners.ends",
                compact_str::format_compact!("{prefix}ends"),
            ),
            (
                "ad_banners.active",
                compact_str::format_compact!("{prefix}active"),
            ),
            (
                "ad_banners.clicks",
                compact_str::format_compact!("{prefix}clicks"),
            ),
            (
                "ad_banners.created",
                compact_str::format_compact!("{prefix}created"),
            ),
        ])
    }

    #[inline]
    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, crate::database::DatabaseError> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            advertiser: row.try_get(compact_str::format_compact!("{prefix}advertiser").as_str())?,
            link: row.try_get(compact_str::format_compact!("{prefix}link").as_str())?,
            placement: AdBannerPlacement::parse(
                row.try_get(compact_str::format_compact!("{prefix}placement").as_str())?,
            ),
            image: row.try_get(compact_str::format_compact!("{prefix}image").as_str())?,
            width: row.try_get(compact_str::format_compact!("{prefix}width").as_str())?,
            height: row.try_get(compact_str::format_compact!("{prefix}height").as_str())?,
            starts: row.try_get(compact_str::format_compact!("{prefix}starts").as_str())?,
            ends: row.try_get(compact_str::format_compact!("{prefix}ends").as_str())?,
            active: row.try_get(compact_str::format_compact!("{prefix}active").as_str())?,
            clicks: row.try_get(compact_str::format_compact!("{prefix}clicks").as_str())?,
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

impl AdBanner {
    /// Shown now: active, with an image, inside its window.
    pub fn is_live(&self, now: chrono::NaiveDateTime) -> bool {
        self.active
            && self.image.is_some()
            && self.starts.is_none_or(|starts| starts <= now)
            && self.ends.is_none_or(|ends| ends > now)
            && (self.placement == AdBannerPlacement::Top
                || slot_size(self.width, self.height).is_some())
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
                FROM ad_banners
                WHERE {search}
                "#,
                search = super::search_sql(1, &["ad_banners.advertiser"], &["ad_banners.uuid"]),
            ),
            "ad_banners.created DESC, ad_banners.uuid",
        )
        .bind(search)
        .fetch(database, page, per_page)
        .await
    }

    /// The banners shown now.
    pub async fn live(
        database: &crate::database::Database,
    ) -> Result<Vec<Self>, crate::database::DatabaseError> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM ad_banners
            WHERE ad_banners.active AND ad_banners.image IS NOT NULL
                AND (ad_banners.starts IS NULL OR ad_banners.starts <= NOW())
                AND (ad_banners.ends IS NULL OR ad_banners.ends > NOW())
            "#,
            Self::columns_sql(None)
        )))
        .fetch_all(database.read())
        .await?;

        let now = chrono::Utc::now().naive_utc();

        Ok(rows
            .into_iter()
            .map(|row| Self::map(None, &row))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|banner| banner.is_live(now))
            .collect())
    }

    /// Counts a click; `false` when there is no such banner.
    pub async fn click(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Option<compact_str::CompactString>, sqlx::Error> {
        sqlx::query_scalar(
            "UPDATE ad_banners SET clicks = clicks + 1 WHERE uuid = $1 RETURNING link",
        )
        .bind(uuid)
        .fetch_optional(database.write())
        .await
    }

    /// Replaces the image (already stored at `path`) and its size.
    pub async fn set_image(
        &mut self,
        database: &crate::database::Database,
        path: &str,
        width: i32,
        height: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE ad_banners SET image = $2, width = $3, height = $4 WHERE uuid = $1")
            .bind(self.uuid)
            .bind(path)
            .bind(width)
            .bind(height)
            .execute(database.write())
            .await?;

        self.image = Some(path.into());
        self.width = width;
        self.height = height;

        Ok(())
    }

    /// What players' pages get.
    pub fn into_api_object(self, storage: &crate::storage::StorageUrlRetriever<'_>) -> ApiAdBanner {
        let (width, height) = match self.placement {
            AdBannerPlacement::Slot => {
                slot_size(self.width, self.height).unwrap_or((self.width, self.height))
            }
            AdBannerPlacement::Top => (self.width, self.height),
        };

        ApiAdBanner {
            uuid: self.uuid,
            advertiser: self.advertiser,
            placement: self.placement,
            image_url: self
                .image
                .map(|image| storage.get_url(image))
                .unwrap_or_default(),
            width,
            height,
        }
    }
}

#[async_trait::async_trait]
impl IntoAdminApiObject for AdBanner {
    type AdminApiObject = AdminApiAdBanner;
    type ExtraArgs<'a> = &'a crate::storage::StorageUrlRetriever<'a>;

    async fn into_admin_api_object<'a>(
        self,
        state: &crate::State,
        storage_url_retriever: Self::ExtraArgs<'a>,
    ) -> Result<Self::AdminApiObject, crate::database::DatabaseError> {
        let api_object = AdminApiAdBanner::init_hooks(&self, state).await?;
        let live = self.is_live(chrono::Utc::now().naive_utc());

        let api_object = finish_extendible!(
            AdminApiAdBanner {
                uuid: self.uuid,
                advertiser: self.advertiser,
                link: self.link,
                placement: self.placement,
                image_url: self.image.map(|image| storage_url_retriever.get_url(image)),
                width: self.width,
                height: self.height,
                starts: self.starts.map(|at| at.and_utc()),
                ends: self.ends.map(|at| at.and_utc()),
                active: self.active,
                live,
                clicks: self.clicks,
                created: self.created.and_utc(),
            },
            api_object,
            state
        )?;

        Ok(api_object)
    }
}

#[async_trait::async_trait]
impl ByUuid for AdBanner {
    async fn by_uuid(
        database: &crate::database::Database,
        uuid: uuid::Uuid,
    ) -> Result<Self, crate::database::DatabaseError> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM ad_banners
            WHERE ad_banners.uuid = $1
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
            FROM ad_banners
            WHERE ad_banners.uuid = $1
            "#,
            Self::columns_sql(None)
        )))
        .bind(uuid)
        .fetch_one(&mut **transaction)
        .await?;

        Self::map(None, &row)
    }
}

fn default_active() -> bool {
    true
}

fn validate_link(link: &compact_str::CompactString, _context: &()) -> garde::Result {
    if valid_link(link) {
        Ok(())
    } else {
        Err(garde::Error::new("the link must be an https:// URL"))
    }
}

fn validate_optional_link(
    link: &Option<compact_str::CompactString>,
    context: &(),
) -> garde::Result {
    match link {
        Some(link) => validate_link(link, context),
        None => Ok(()),
    }
}

#[derive(ToSchema, Deserialize, Validate)]
pub struct CreateAdBannerOptions {
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub advertiser: compact_str::CompactString,
    #[garde(length(chars, min = 1, max = 1024), custom(validate_link))]
    #[schema(min_length = 1, max_length = 1024)]
    pub link: compact_str::CompactString,
    #[garde(skip)]
    pub placement: AdBannerPlacement,
    #[garde(skip)]
    pub starts: Option<chrono::DateTime<chrono::Utc>>,
    #[garde(skip)]
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
    #[garde(skip)]
    #[serde(default = "default_active")]
    pub active: bool,
}

#[async_trait::async_trait]
impl CreatableModel for AdBanner {
    type CreateOptions<'a> = CreateAdBannerOptions;
    type CreateResult = Self;

    fn get_create_handlers() -> &'static LazyLock<CreateListenerList<Self>> {
        static CREATE_LISTENERS: LazyLock<CreateListenerList<AdBanner>> =
            LazyLock::new(|| Arc::new(ModelHandlerList::default()));

        &CREATE_LISTENERS
    }

    async fn create_with_transaction(
        state: &crate::State,
        mut options: Self::CreateOptions<'_>,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Self, crate::database::DatabaseError> {
        options.validate()?;

        let mut query_builder = InsertQueryBuilder::new("ad_banners");

        Self::run_create_handlers(&mut options, &mut query_builder, state, transaction).await?;

        query_builder
            .set("advertiser", &options.advertiser)
            .set("link", &options.link)
            .set("placement", options.placement.as_str())
            .set("starts", options.starts.map(|at| at.naive_utc()))
            .set("ends", options.ends.map(|at| at.naive_utc()))
            .set("active", options.active);

        let row = query_builder
            .returning(&Self::columns_sql(None))
            .fetch_one(&mut **transaction)
            .await?;
        let mut banner = Self::map(None, &row)?;

        Self::run_after_create_handlers(&mut banner, &options, state, transaction).await?;

        Ok(banner)
    }
}

#[derive(ToSchema, Serialize, Deserialize, Validate, Clone, Default)]
pub struct UpdateAdBannerOptions {
    #[garde(length(chars, min = 1, max = 255))]
    #[schema(min_length = 1, max_length = 255)]
    pub advertiser: Option<compact_str::CompactString>,
    #[garde(length(chars, min = 1, max = 1024), custom(validate_optional_link))]
    #[schema(min_length = 1, max_length = 1024)]
    pub link: Option<compact_str::CompactString>,
    #[garde(skip)]
    pub placement: Option<AdBannerPlacement>,
    #[garde(skip)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub starts: Option<Option<chrono::DateTime<chrono::Utc>>>,
    #[garde(skip)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub ends: Option<Option<chrono::DateTime<chrono::Utc>>>,
    #[garde(skip)]
    pub active: Option<bool>,
}

#[async_trait::async_trait]
impl UpdatableModel for AdBanner {
    type UpdateOptions = UpdateAdBannerOptions;

    fn get_update_handlers() -> &'static LazyLock<UpdateHandlerList<Self>> {
        static UPDATE_LISTENERS: LazyLock<UpdateHandlerList<AdBanner>> =
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

        let mut query_builder = UpdateQueryBuilder::new("ad_banners");

        self.run_update_handlers(&mut options, &mut query_builder, state, transaction)
            .await?;

        query_builder
            .set("advertiser", options.advertiser.as_ref())
            .set("link", options.link.as_ref())
            .set(
                "placement",
                options.placement.map(AdBannerPlacement::as_str),
            )
            .set(
                "starts",
                options.starts.map(|starts| starts.map(|at| at.naive_utc())),
            )
            .set(
                "ends",
                options.ends.map(|ends| ends.map(|at| at.naive_utc())),
            )
            .set("active", options.active)
            .where_eq("uuid", self.uuid);

        query_builder.execute(&mut **transaction).await?;

        if let Some(advertiser) = options.advertiser {
            self.advertiser = advertiser;
        }
        if let Some(link) = options.link {
            self.link = link;
        }
        if let Some(placement) = options.placement {
            self.placement = placement;
        }
        if let Some(starts) = options.starts {
            self.starts = starts.map(|at| at.naive_utc());
        }
        if let Some(ends) = options.ends {
            self.ends = ends.map(|at| at.naive_utc());
        }
        if let Some(active) = options.active {
            self.active = active;
        }

        self.run_after_update_handlers(state, transaction).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeletableModel for AdBanner {
    type DeleteOptions = ();

    fn get_delete_handlers() -> &'static LazyLock<DeleteHandlerList<Self>> {
        static DELETE_LISTENERS: LazyLock<DeleteHandlerList<AdBanner>> =
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

        sqlx::query("DELETE FROM ad_banners WHERE ad_banners.uuid = $1")
            .bind(self.uuid)
            .execute(&mut **transaction)
            .await?;

        self.run_after_delete_handlers(&options, state, transaction)
            .await?;

        Ok(())
    }
}

/// A banner as players' pages get it: the size it is laid out at.
#[derive(ToSchema, Serialize)]
pub struct ApiAdBanner {
    pub uuid: uuid::Uuid,
    pub advertiser: compact_str::CompactString,
    pub placement: AdBannerPlacement,
    pub image_url: String,
    pub width: i32,
    pub height: i32,
}

#[schema_extension_derive::extendible]
#[init_args(AdBanner, crate::State)]
#[hook_args(crate::State)]
#[derive(ToSchema, Serialize)]
#[schema(title = "AdBanner")]
pub struct AdminApiAdBanner {
    pub uuid: uuid::Uuid,

    pub advertiser: compact_str::CompactString,
    pub link: compact_str::CompactString,
    pub placement: AdBannerPlacement,

    pub image_url: Option<String>,
    pub width: i32,
    pub height: i32,

    pub starts: Option<chrono::DateTime<chrono::Utc>>,
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
    pub active: bool,
    /// Active, with an image, inside its window, and (a slot banner) of an Adsterra size.
    pub live: bool,
    pub clicks: i64,

    pub created: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_banners_take_adsterra_sizes_or_twice_them() {
        assert_eq!(slot_size(728, 90), Some((728, 90)));
        assert_eq!(slot_size(1456, 180), Some((728, 90)));
        assert_eq!(slot_size(600, 500), Some((300, 250)));
        assert_eq!(slot_size(700, 90), None);
    }

    #[test]
    fn links_are_https_only() {
        assert!(valid_link("https://example.com/shop?x=1"));
        assert!(!valid_link("http://example.com"));
        assert!(!valid_link("javascript:alert(1)"));
        assert!(!valid_link("https://localhost/"));
    }
}
