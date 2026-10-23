use crate::{models::ByUuid, response::DisplayError};
use futures_util::TryStreamExt;
use image::{ImageReader, codecs::webp::WebPEncoder, imageops::FilterType};
use rand::distr::SampleString;
use sha2::Digest;

const STORED_SIZE: u32 = 512;
const MAX_IMPORT_BYTES: usize = 8 * 1024 * 1024;
const MIN_UPLOAD_DIMENSION: u32 = 64;
const MAX_UPLOAD_DIMENSION: u32 = 4096;

pub fn upload_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);

    limits
}

fn import_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(16 * 1024 * 1024);
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);

    limits
}

pub async fn decode(
    data: impl AsRef<[u8]> + Send + 'static,
    limits: image::Limits,
) -> Result<image::DynamicImage, anyhow::Error> {
    if data.as_ref().is_empty() {
        return Err(DisplayError::new("image: payload cannot be empty").into());
    }

    let mut reader = ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| DisplayError::new("image: unable to decode"))?;

    let Some(format) = reader.format() else {
        return Err(DisplayError::new("image: unrecognized image format").into());
    };

    if !matches!(
        format,
        image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::WebP
            | image::ImageFormat::Gif
    ) {
        return Err(
            DisplayError::new("image: only PNG, JPEG, WebP, and GIF formats are allowed").into(),
        );
    }

    reader.limits(limits);

    Ok(tokio::task::spawn_blocking(move || reader.decode())
        .await?
        .map_err(|_| DisplayError::new("image: unable to decode"))?)
}

pub async fn transcode(
    data: impl AsRef<[u8]> + Send + 'static,
    limits: image::Limits,
    enforce_minimum: bool,
) -> Result<Vec<u8>, anyhow::Error> {
    let image = decode(data, limits).await?;

    if enforce_minimum
        && (!(MIN_UPLOAD_DIMENSION..=MAX_UPLOAD_DIMENSION).contains(&image.width())
            || !(MIN_UPLOAD_DIMENSION..=MAX_UPLOAD_DIMENSION).contains(&image.height()))
    {
        return Err(DisplayError::new(
            "image: invalid resolution, dimensions must not be smaller than 64px and must not exceed 4096px",
        )
        .into());
    }

    Ok(
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>, image::ImageError> {
            let image = image.resize_exact(STORED_SIZE, STORED_SIZE, FilterType::Triangle);
            let mut data: Vec<u8> = Vec::new();
            let encoder = WebPEncoder::new_lossless(&mut data);
            let color = image.color();
            encoder.encode(image.as_bytes(), STORED_SIZE, STORED_SIZE, color.into())?;

            Ok(data)
        })
        .await??,
    )
}

fn random_path(user_uuid: uuid::Uuid) -> String {
    format!(
        "avatars/{}/{}.webp",
        user_uuid,
        rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 8)
    )
}

fn imported_path_prefix(user_uuid: uuid::Uuid, source_url: &str) -> String {
    let digest = sha2::Sha256::digest(source_url.as_bytes());

    format!("avatars/{}/{}-", user_uuid, hex::encode(&digest[..16]))
}

async fn store(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    previous: Option<&str>,
    path: &str,
    data: Vec<u8>,
) -> Result<(), anyhow::Error> {
    state
        .storage
        .store_with_cache_control(
            path,
            data.as_slice(),
            "image/webp",
            Some("public, max-age=31536000, immutable"),
        )
        .await?;

    if previous != Some(path) {
        state.storage.remove(previous).await?;
    }

    sqlx::query!(
        "UPDATE users
        SET avatar = $2
        WHERE users.uuid = $1",
        user_uuid,
        path
    )
    .execute(state.database.write())
    .await?;

    super::User::invalidate_cached(&state.database, user_uuid).await;

    Ok(())
}

impl super::User {
    /// Stores `data` as this user's avatar under a freshly generated path, removing the
    /// previously stored one, and returns the path it was stored at.
    pub async fn store_avatar(
        &mut self,
        state: &crate::State,
        data: Vec<u8>,
    ) -> Result<String, anyhow::Error> {
        let path = random_path(self.uuid);
        store(state, self.uuid, self.avatar.as_deref(), &path, data).await?;
        self.avatar = Some(path.clone());

        Ok(path)
    }

    /// Downloads, transcodes and stores `source_url` as the avatar of `user_uuid`. The path is
    /// prefixed by a digest of `source_url`, so re-importing an unchanged url is a no-op, and
    /// suffixed by a digest of the stored bytes, so a path never changes content (avatars are
    /// served as immutable).
    pub async fn import_avatar_by_uuid(
        state: &crate::State,
        user_uuid: uuid::Uuid,
        previous: Option<&str>,
        source_url: &str,
    ) -> Result<(), anyhow::Error> {
        let path_prefix = imported_path_prefix(user_uuid, source_url);
        if previous.is_some_and(|previous| previous.starts_with(&path_prefix)) {
            return Ok(());
        }

        let response = crate::net::outbound_request(
            &state.env,
            reqwest::Method::GET,
            reqwest::Url::parse(source_url)?,
        )?
        .send()
        .await?;

        if !response.status().is_success() {
            return Err(DisplayError::new(format!(
                "avatar source responded with status {}",
                response.status()
            ))
            .into());
        }

        let mut data = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.try_next().await? {
            if data.len() + chunk.len() > MAX_IMPORT_BYTES {
                return Err(DisplayError::new("avatar source is too large").into());
            }

            data.extend_from_slice(&chunk);
        }

        let data = transcode(data, import_limits(), false).await?;
        let path = format!(
            "{path_prefix}{}.webp",
            hex::encode(&sha2::Sha256::digest(&data)[..8])
        );

        store(state, user_uuid, previous, &path, data).await
    }
}
