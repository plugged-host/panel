//! plugged.host: screenshots people show rux: the image attachments of an order (and of the
//! message it replies to), downloaded from Discord's CDN when the order runs and shrunk to a JPEG
//! at most 1568 px a side (what vision models read at full detail), transparent areas white.
//! History only keeps the note that they were there.

use base64::Engine;
use serde_json::Value;

pub const MAX_IMAGES: usize = crate::neo::chat::MAX_IMAGES;
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
const MAX_SIDE: u32 = 1568;
/// Decoding takes about 4 bytes a pixel.
const MAX_PIXELS: u64 = 12_000_000;
const TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];
const HOSTS: [&str; 2] = ["cdn.discordapp.com", "media.discordapp.net"];

/// Whether `url` is on Discord's CDN (https, no port, no user).
pub fn is_discord_cdn(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };

    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .is_some_and(|host| HOSTS.contains(&host.to_ascii_lowercase().as_str()))
}

/// The image attachments of a Discord message: their URLs, at most [`MAX_IMAGES`].
pub fn attachments(message: &Value) -> Vec<String> {
    message["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|file| {
            let kind = file["content_type"]
                .as_str()
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            TYPES.contains(&kind.as_str())
                && file["size"].as_u64().unwrap_or(0) <= MAX_BYTES
                && file["url"].as_str().is_some_and(is_discord_cdn)
        })
        .filter_map(|file| file["url"].as_str().map(str::to_string))
        .take(MAX_IMAGES)
        .collect()
}

/// Image bytes as the JPEG data URL a model takes; `None` when they are not a PNG, JPEG, WebP or
/// GIF (its first frame) of a sane size.
pub fn data_url(bytes: &[u8]) -> Option<String> {
    let jpeg = jpeg(bytes)?;

    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    ))
}

fn jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES {
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

/// One attachment's bytes, when it answers from Discord's CDN and is not too large.
async fn download(state: &crate::State, url: &str) -> Option<Vec<u8>> {
    use futures_util::StreamExt;

    let parsed = reqwest::Url::parse(url).ok()?;
    if !is_discord_cdn(url) {
        return None;
    }
    let response = crate::net::outbound_request(&state.env, reqwest::Method::GET, parsed)
        .ok()?
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success()
        || !is_discord_cdn(response.url().as_str())
        || response
            .content_length()
            .is_some_and(|length| length > MAX_BYTES)
    {
        return None;
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.ok()?);
        if bytes.len() as u64 > MAX_BYTES {
            return None;
        }
    }

    Some(bytes)
}

/// The images behind these URLs as data URLs, in order; the ones that cannot be loaded are left
/// out.
pub async fn load(state: &crate::State, urls: &[String]) -> Vec<String> {
    let mut images = Vec::new();
    for url in urls.iter().take(MAX_IMAGES) {
        let Some(bytes) = download(state, url).await else {
            continue;
        };
        if let Ok(Some(image)) = tokio::task::spawn_blocking(move || data_url(&bytes)).await {
            images.push(image);
        }
    }

    images
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_images_on_discords_cdn() {
        assert!(is_discord_cdn(
            "https://cdn.discordapp.com/attachments/1/2/a.png"
        ));
        assert!(!is_discord_cdn("http://cdn.discordapp.com/a.png"));
        assert!(!is_discord_cdn("https://cdn.discordapp.com:8443/a.png"));
        assert!(!is_discord_cdn("https://example.com/a.png"));

        let message = json!({"attachments": [
            {"url": "https://cdn.discordapp.com/a.png", "content_type": "image/png", "size": 10},
            {"url": "https://cdn.discordapp.com/a.txt", "content_type": "text/plain", "size": 10},
            {"url": "https://example.com/b.png", "content_type": "image/png", "size": 10},
            {"url": "https://media.discordapp.net/c.jpg", "content_type": "image/jpeg; charset=x", "size": 10},
            {"url": "https://cdn.discordapp.com/d.png", "content_type": "image/png", "size": 99_999_999u64}
        ]});
        assert_eq!(
            attachments(&message),
            vec![
                "https://cdn.discordapp.com/a.png",
                "https://media.discordapp.net/c.jpg"
            ]
        );
    }

    #[test]
    fn pictures_become_white_backed_jpegs() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 0]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let url = data_url(&png).unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(data_url(b"not an image").is_none());
    }
}
