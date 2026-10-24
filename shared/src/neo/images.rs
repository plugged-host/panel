//! plugged.host: images attached to a neo chat message. Shrunk once on upload to a JPEG at most
//! 1568 px a side (what vision models read at full detail) and kept in `neo_images`, so the
//! model and the chat both read the small copy; they go with their message.

use base64::Engine;

/// The most images one message takes ([`super::chat::MAX_IMAGES`] reach the model at once).
pub const MAX_IMAGES: usize = super::chat::MAX_IMAGES;
/// The largest upload read.
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_SIDE: u32 = 1568;
const MAX_PIXELS: u64 = 12_000_000;

/// Image bytes as a JPEG at most [`MAX_SIDE`] px a side, transparent areas white; `None` when
/// they are not a PNG, JPEG, WebP or GIF (its first frame) of a sane size.
pub fn jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return None;
    }

    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    if !matches!(
        reader.format(),
        Some(
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::WebP
                | image::ImageFormat::Gif
        )
    ) {
        return None;
    }

    let (width, height) = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }

    let image = reader.decode().ok()?;
    let image = if width.max(height) > MAX_SIDE {
        image.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        image
    };

    let rgba = image.into_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = u16::from(a);
        let over_white = |c: u8| ((u16::from(c) * alpha + 255 * (255 - alpha)) / 255) as u8;
        rgb.put_pixel(
            x,
            y,
            image::Rgb([over_white(r), over_white(g), over_white(b)]),
        );
    }

    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&rgb)
        .ok()?;

    Some(out)
}

/// JPEG bytes as the data URL a model takes.
pub fn data_url(jpeg: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    )
}

/// Keeps shrunk images with their message; returns their uuids in order.
pub async fn store(
    executor: &mut sqlx::PgConnection,
    message_uuid: uuid::Uuid,
    images: &[Vec<u8>],
) -> Result<Vec<uuid::Uuid>, sqlx::Error> {
    let mut uuids = Vec::with_capacity(images.len());
    for image in images.iter().take(MAX_IMAGES) {
        uuids.push(
            sqlx::query_scalar(
                "INSERT INTO neo_images (message_uuid, data) VALUES ($1, $2) RETURNING uuid",
            )
            .bind(message_uuid)
            .bind(image)
            .fetch_one(&mut *executor)
            .await?,
        );
    }

    Ok(uuids)
}

/// The images of these uuids that still exist, as data URLs.
pub async fn data_urls(
    database: &crate::database::Database,
    uuids: &[uuid::Uuid],
) -> Result<std::collections::HashMap<uuid::Uuid, String>, sqlx::Error> {
    if uuids.is_empty() {
        return Ok(Default::default());
    }

    let rows: Vec<(uuid::Uuid, Vec<u8>)> =
        sqlx::query_as("SELECT uuid, data FROM neo_images WHERE uuid = ANY($1)")
            .bind(uuids)
            .fetch_all(database.read())
            .await?;

    Ok(rows
        .into_iter()
        .map(|(uuid, data)| (uuid, data_url(&data)))
        .collect())
}

/// One image of the user's own conversation, for the chat to show.
pub async fn own(
    database: &crate::database::Database,
    uuid: uuid::Uuid,
    user_uuid: uuid::Uuid,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT neo_images.data FROM neo_images
        JOIN neo_messages ON neo_messages.uuid = neo_images.message_uuid
        JOIN neo_conversations ON neo_conversations.uuid = neo_messages.conversation_uuid
        WHERE neo_images.uuid = $1 AND neo_conversations.user_uuid = $2",
    )
    .bind(uuid)
    .bind(user_uuid)
    .fetch_optional(database.read())
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, alpha]));
        let mut out = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("a png is written");

        out.into_inner()
    }

    #[test]
    fn shrinks_to_a_jpeg() {
        let jpeg = jpeg(&png(3000, 1000, 255)).expect("a jpeg");
        let decoded = image::load_from_memory(&jpeg).expect("it decodes");
        assert_eq!(
            image::guess_format(&jpeg).ok(),
            Some(image::ImageFormat::Jpeg)
        );
        assert_eq!(decoded.width(), MAX_SIDE);
        assert!(decoded.height() <= 523 && decoded.height() >= 522);
        assert!(data_url(&jpeg).starts_with("data:image/jpeg;base64,/9j/"));
    }

    #[test]
    fn keeps_small_images_and_whitens_transparency() {
        let jpeg = jpeg(&png(10, 10, 0)).expect("a jpeg");
        let decoded = image::load_from_memory(&jpeg)
            .expect("it decodes")
            .into_rgb8();
        assert_eq!(decoded.dimensions(), (10, 10));
        assert!(decoded.get_pixel(5, 5).0.iter().all(|c| *c > 240));
    }

    #[test]
    fn refuses_what_is_not_an_image() {
        assert!(jpeg(b"not an image").is_none());
        assert!(jpeg(&[]).is_none());
    }
}
