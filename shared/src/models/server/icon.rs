// plugged.host: a server's icon, shaped like upstream's user avatar (`models/user/avatar.rs`): an
// upload is decoded with the avatar's limits, cut to a square and stored as WebP under a fresh
// path; without one a server shows its game's icon (docs/features/server-settings.md#server-icon).

use crate::{
    models::{ByUuid, user::avatar},
    response::DisplayError,
};
use rand::distr::SampleString;

/// The stored icon's width and height: twice the 120 px it is shown at on Settings.
const STORED_SIZE: u32 = 256;
const MIN_UPLOAD_DIMENSION: u32 = 64;
const MAX_UPLOAD_DIMENSION: u32 = 4096;

/// Where an icon of `server_uuid` is stored. Under `avatars/` (a server's icon is its avatar in
/// Filament's words), which the panel serves as immutable, so every upload gets a new name.
pub fn stored_path(server_uuid: uuid::Uuid, name: &str) -> String {
    format!("avatars/servers/{server_uuid}/{name}.webp")
}

/// The square that `width` x `height` is cut to: its shorter side, centred on the longer one.
fn centred_square(width: u32, height: u32) -> (u32, u32, u32) {
    let side = width.min(height);

    (side, (width - side) / 2, (height - side) / 2)
}

/// The games catalog's icon of the server's egg, what the Pelican panel calls the egg's icon.
pub fn egg_icon(
    catalog: &[crate::settings::games::Game],
    egg_uuid: uuid::Uuid,
) -> Option<compact_str::CompactString> {
    catalog
        .iter()
        .find(|game| game.egg_uuid == egg_uuid)
        .and_then(|game| game.icon.as_deref())
        .map(str::trim)
        .filter(|icon| !icon.is_empty())
        .map(Into::into)
}

/// An uploaded image as a server icon: PNG, JPEG, WebP or GIF, 64 to 4096 px a side, cut to its
/// centred square (an upload that is not square is never stretched) and stored as lossless WebP.
pub async fn transcode(data: impl AsRef<[u8]> + Send + 'static) -> Result<Vec<u8>, anyhow::Error> {
    let image = avatar::decode(data, avatar::upload_limits()).await?;

    if !(MIN_UPLOAD_DIMENSION..=MAX_UPLOAD_DIMENSION).contains(&image.width())
        || !(MIN_UPLOAD_DIMENSION..=MAX_UPLOAD_DIMENSION).contains(&image.height())
    {
        return Err(DisplayError::new(
            "image: invalid resolution, dimensions must not be smaller than 64px and must not exceed 4096px",
        )
        .into());
    }

    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, anyhow::Error> {
        let (side, x, y) = centred_square(image.width(), image.height());
        let image = image
            .crop_imm(x, y, side, side)
            .resize_exact(
                STORED_SIZE,
                STORED_SIZE,
                image::imageops::FilterType::Triangle,
            )
            .into_rgba8();

        let mut data = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut data).encode(
            image.as_raw(),
            STORED_SIZE,
            STORED_SIZE,
            image::ExtendedColorType::Rgba8,
        )?;

        Ok(data)
    })
    .await?
}

impl super::Server {
    /// Stores `data` (from [`transcode`]) as this server's icon under a fresh path, removes the
    /// previous one, and returns the new path.
    pub async fn store_icon(
        &mut self,
        state: &crate::State,
        data: Vec<u8>,
    ) -> Result<String, anyhow::Error> {
        let path = stored_path(
            self.uuid,
            &rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 8),
        );

        state
            .storage
            .store_with_cache_control(
                &path,
                data.as_slice(),
                "image/webp",
                Some("public, max-age=31536000, immutable"),
            )
            .await?;

        sqlx::query("UPDATE servers SET icon = $2 WHERE servers.uuid = $1")
            .bind(self.uuid)
            .bind(&path)
            .execute(state.database.write())
            .await?;

        Self::invalidate_cached(&state.database, self.uuid).await;

        let previous = self.icon.replace(path.clone());
        if previous.as_deref() != Some(path.as_str()) {
            state.storage.remove(previous.as_deref()).await?;
        }

        Ok(path)
    }

    /// Removes this server's icon, so it shows its game's again.
    pub async fn remove_icon(&mut self, state: &crate::State) -> Result<(), anyhow::Error> {
        let Some(previous) = self.icon.take() else {
            return Ok(());
        };

        sqlx::query("UPDATE servers SET icon = NULL WHERE servers.uuid = $1")
            .bind(self.uuid)
            .execute(state.database.write())
            .await?;

        Self::invalidate_cached(&state.database, self.uuid).await;

        state.storage.remove(Some(previous)).await
    }
}

/// Removes a deleted server's stored icon, in the background: the server is already gone.
pub fn forget(state: &crate::State, icon: Option<String>) {
    let Some(icon) = icon else {
        return;
    };
    let state = state.clone();

    tokio::spawn(async move {
        if let Err(err) = state.storage.remove(Some(&icon)).await {
            tracing::warn!(%icon, "failed to remove a deleted server's icon: {:#?}", err);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{centred_square, egg_icon, stored_path, transcode};
    use crate::settings::games::Game;

    fn game(egg_uuid: uuid::Uuid, icon: Option<&str>) -> Game {
        Game {
            egg_uuid,
            name: None,
            description: None,
            icon: icon.map(Into::into),
            variable: None,
            variants: Vec::new(),
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::new();
        image::DynamicImage::new_rgba8(width, height)
            .write_to(
                &mut std::io::Cursor::new(&mut data),
                image::ImageFormat::Png,
            )
            .unwrap();

        data
    }

    #[test]
    fn icons_are_stored_under_the_servers_own_folder() {
        let uuid = uuid::Uuid::nil();

        assert_eq!(
            stored_path(uuid, "abcdefgh"),
            "avatars/servers/00000000-0000-0000-0000-000000000000/abcdefgh.webp"
        );
    }

    #[test]
    fn a_picture_is_cut_to_its_centred_square() {
        assert_eq!(centred_square(300, 100), (100, 100, 0));
        assert_eq!(centred_square(100, 301), (100, 0, 100));
        assert_eq!(centred_square(64, 64), (64, 0, 0));
    }

    #[test]
    fn the_egg_icon_is_its_games_icon() {
        let minecraft = uuid::Uuid::from_u128(1);
        let rust = uuid::Uuid::from_u128(2);
        let blank = uuid::Uuid::from_u128(3);
        let catalog = [
            game(minecraft, Some(" https://cdn.example.com/minecraft.png ")),
            game(rust, None),
            game(blank, Some("  ")),
        ];

        assert_eq!(
            egg_icon(&catalog, minecraft).as_deref(),
            Some("https://cdn.example.com/minecraft.png")
        );
        assert_eq!(egg_icon(&catalog, rust), None);
        assert_eq!(egg_icon(&catalog, blank), None);
        assert_eq!(egg_icon(&catalog, uuid::Uuid::from_u128(4)), None);
    }

    #[tokio::test]
    async fn an_upload_becomes_a_square_webp() {
        let data = transcode(png(300, 100)).await.unwrap();
        let image = image::load_from_memory(&data).unwrap();

        assert_eq!(
            image::guess_format(&data).unwrap(),
            image::ImageFormat::WebP
        );
        assert_eq!((image.width(), image.height()), (256, 256));
    }

    #[tokio::test]
    async fn too_small_and_unknown_uploads_are_refused() {
        assert!(transcode(png(32, 300)).await.is_err());
        assert!(transcode(b"not an image".to_vec()).await.is_err());
        assert!(transcode(Vec::new()).await.is_err());
    }
}
