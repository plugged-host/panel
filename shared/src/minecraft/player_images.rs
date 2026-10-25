//! plugged.host: the pictures of the Players page (docs/features/minecraft.md#player-details):
//! a player's head, cut from their skin, and the icons of the items in their inventory. Both are
//! fetched by the panel and cached, so the browser only ever loads them from the panel.
//!
//! Skins come from Mojang (Java accounts) or GeyserMC's skin API (Bedrock players through
//! Floodgate) and are only taken from `textures.minecraft.net`. Item icons come from the
//! Minecraft Wiki's inventory pictures, then the game's own textures on mcasset.cloud.

use super::{player_data, players};
use base64::Engine;
use image::{GenericImageView, ImageFormat, RgbaImage};

const HEAD_SIZE: u32 = 64;
const MAX_SKIN: u64 = 512 * 1024;
const MAX_ICON: u64 = 256 * 1024;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
const HEAD_TTL: u64 = 24 * 60 * 60;
const ICON_TTL: u64 = 7 * 24 * 60 * 60;
const MISS_TTL: u64 = 24 * 60 * 60;
const ERROR_TTL: u64 = 10 * 60;
const SKIN_HOST: &str = "textures.minecraft.net";
/// The skin of a player without one (or whose skin cannot be fetched).
const DEFAULT_SKIN: &str =
    "https://assets.mcasset.cloud/latest/assets/minecraft/textures/entity/player/wide/steve.png";
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A player's head as a 64×64 PNG: the face with the hat layer over it.
pub async fn head(
    state: &crate::State,
    uuid: uuid::Uuid,
    name: Option<&str>,
) -> Result<Vec<u8>, anyhow::Error> {
    let name = name
        .map(str::trim)
        .filter(|name| players::valid_name(name))
        .map(str::to_ascii_lowercase);
    let key = format!(
        "minecraft::head::{}::{}",
        uuid.hyphenated(),
        name.as_deref().unwrap_or_default()
    );
    if let Ok(Some(cached)) = state.cache.get::<Vec<u8>>(&key).await {
        return Ok(cached);
    }

    let (skin, ttl) = match skin_url(state, uuid, name.as_deref()).await {
        Ok(Some(url)) => match fetch(state, &url, MAX_SKIN).await {
            Ok(Some(skin)) => (Some(skin), HEAD_TTL),
            Ok(None) => (None, HEAD_TTL),
            Err(err) => {
                tracing::debug!("fetching the skin of {uuid}: {err:#}");
                (None, ERROR_TTL)
            }
        },
        Ok(None) => (None, HEAD_TTL),
        Err(err) => {
            tracing::debug!("looking up the skin of {uuid}: {err:#}");
            (None, ERROR_TTL)
        }
    };

    let head = match skin.as_deref().map(render_head) {
        Some(Ok(head)) => head,
        // a skin the game would not load either: the default one
        _ => {
            let skin = fetch(state, DEFAULT_SKIN, MAX_SKIN)
                .await?
                .ok_or_else(|| anyhow::anyhow!("the default skin is missing"))?;
            render_head(&skin)?
        }
    };
    state.cache.set(&key, ttl, &head).await.ok();

    Ok(head)
}

/// Where the player's skin is, `None` for a player without one.
async fn skin_url(
    state: &crate::State,
    uuid: uuid::Uuid,
    name: Option<&str>,
) -> Result<Option<String>, anyhow::Error> {
    if player_data::is_bedrock(uuid) {
        let xuid = uuid.as_u64_pair().1;
        let response = state
            .client
            .get(format!("https://api.geysermc.org/v2/skin/{xuid}"))
            .timeout(TIMEOUT)
            .header("User-Agent", super::user_agent(state).await)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: serde_json::Value = response.error_for_status()?.json().await?;

        return Ok(body["texture_id"]
            .as_str()
            .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|id| format!("https://{SKIN_HOST}/texture/{id}")));
    }

    // a Mojang account's UUID is random (version 4); an offline-mode server makes its own
    // (version 3) from the name, which may still be an account's
    let account = if uuid.get_version_num() == 4 {
        Some(uuid)
    } else {
        match name {
            Some(name) => players::profile(state, name)
                .await?
                .and_then(|profile| uuid::Uuid::try_parse(&profile.uuid).ok()),
            None => None,
        }
    };
    let Some(account) = account else {
        return Ok(None);
    };

    let response = state
        .client
        .get(format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{}",
            account.simple()
        ))
        .timeout(TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .send()
        .await?;
    if matches!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NO_CONTENT
    ) {
        return Ok(None);
    }
    let body: serde_json::Value = response.error_for_status()?.json().await?;

    Ok(skin_from_profile(&body))
}

/// The skin URL in a session server profile: a base64 JSON `textures` property.
fn skin_from_profile(profile: &serde_json::Value) -> Option<String> {
    let value = profile["properties"]
        .as_array()?
        .iter()
        .find(|property| property["name"] == "textures")?["value"]
        .as_str()?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .ok()?;
    let textures: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    skin_host_url(textures["textures"]["SKIN"]["url"].as_str()?)
}

/// A skin URL on Mojang's texture host, over https; anything else is refused.
fn skin_host_url(raw: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(raw).ok()?;
    if url.host_str() != Some(SKIN_HOST) || !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_scheme("https").ok()?;

    Some(url.into())
}

/// A PNG from one of the fixed hosts, `None` when it is not there.
async fn fetch(
    state: &crate::State,
    url: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let response = state
        .client
        .get(url)
        .timeout(TIMEOUT)
        .header("User-Agent", super::user_agent(state).await)
        .header("Accept", "image/png")
        .send()
        .await?;
    if matches!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
    ) {
        return Ok(None);
    }
    let response = response.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        anyhow::bail!("the picture is too large");
    }

    let bytes = response.bytes().await?;
    if bytes.len() as u64 > limit {
        anyhow::bail!("the picture is too large");
    }
    if !bytes.starts_with(PNG_MAGIC) {
        anyhow::bail!("the answer is not a png");
    }

    Ok(Some(bytes.to_vec()))
}

/// Cuts the head out of a skin as the game draws it: the face (8,8) and the hat (40,8) over it,
/// scaled to 64×64 without smoothing. Skins of the old 64×32 layout whose hat area has no
/// transparent pixel have no hat, as in the game.
pub fn render_head(skin: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let skin = image::load_from_memory_with_format(skin, ImageFormat::Png)?;
    let (width, height) = skin.dimensions();
    if width < 64 || width % 64 != 0 || (height != width && height != width / 2) {
        anyhow::bail!("a skin is 64×64 or 64×32 (or a multiple)");
    }
    let skin = skin.to_rgba8();
    let scale = width / 64;
    let side = 8 * scale;

    let legacy = height == width / 2;
    let hat = !legacy
        || (32 * scale..64 * scale)
            .flat_map(|x| (0..32 * scale).map(move |y| (x, y)))
            .any(|(x, y)| skin.get_pixel(x, y)[3] < 128);

    let mut face = RgbaImage::new(side, side);
    for x in 0..side {
        for y in 0..side {
            let mut pixel = *skin.get_pixel(8 * scale + x, 8 * scale + y);
            // the face itself is always opaque
            pixel[3] = 255;

            if hat {
                let over = skin.get_pixel(40 * scale + x, 8 * scale + y);
                let alpha = over[3] as u32;
                for channel in 0..3 {
                    pixel[channel] = ((over[channel] as u32 * alpha
                        + pixel[channel] as u32 * (255 - alpha))
                        / 255) as u8;
                }
            }

            face.put_pixel(x, y, pixel);
        }
    }

    let face = image::imageops::resize(
        &face,
        HEAD_SIZE,
        HEAD_SIZE,
        image::imageops::FilterType::Nearest,
    );
    let mut png = Vec::new();
    face.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)?;

    Ok(png)
}

/// An item id the icons are looked up for: Minecraft's own, `minecraft:` optional.
pub fn valid_item(id: &str) -> bool {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);

    (1..=64).contains(&path.len())
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A Minecraft version as the texture site names it (`1.21.4`, `24w14a`, `1.20.5-rc1`), or
/// `latest`.
pub fn valid_version(version: &str) -> bool {
    (1..=32).contains(&version.len())
        && version
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && version
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// The Minecraft Wiki's name of an item's inventory picture (`Invicon_<title>.png`).
pub fn wiki_title(path: &str) -> String {
    match path {
        "tnt" => "TNT".into(),
        "tnt_minecart" => "Minecart_with_TNT".into(),
        "leather_helmet" => "Leather_Cap".into(),
        "leather_chestplate" => "Leather_Tunic".into(),
        "leather_leggings" => "Leather_Pants".into(),
        _ => player_data::label(path).replace(' ', "_"),
    }
}

/// Where an item's icon may be, best first: the wiki's rendered picture, then the game's item
/// texture, then its block texture. Nothing for another namespace's (a mod's) items.
pub fn icon_urls(id: &str, version: &str) -> Vec<String> {
    let path = match id.split_once(':') {
        Some(("minecraft", path)) => path,
        Some(_) => return Vec::new(),
        None => id,
    };
    if !valid_item(path) || !valid_version(version) {
        return Vec::new();
    }

    vec![
        format!(
            "https://minecraft.wiki/images/Invicon_{}.png",
            wiki_title(path)
        ),
        format!("https://assets.mcasset.cloud/{version}/assets/minecraft/textures/item/{path}.png"),
        format!(
            "https://assets.mcasset.cloud/{version}/assets/minecraft/textures/block/{path}.png"
        ),
    ]
}

/// An item's icon (PNG), `None` when no source has one. Kept a week; a miss a day, and a
/// failed lookup ten minutes.
pub async fn item_icon(
    state: &crate::State,
    id: &str,
    version: &str,
) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let urls = icon_urls(id, version);
    if urls.is_empty() {
        return Ok(None);
    }

    let key = format!(
        "minecraft::item_icon::{version}::{}",
        id.strip_prefix("minecraft:").unwrap_or(id)
    );
    if let Ok(Some(cached)) = state.cache.get::<Option<Vec<u8>>>(&key).await {
        return Ok(cached);
    }

    let mut failed = None;
    for url in &urls {
        match fetch(state, url, MAX_ICON).await {
            Ok(Some(icon)) => {
                state.cache.set(&key, ICON_TTL, &Some(&icon)).await.ok();
                return Ok(Some(icon));
            }
            Ok(None) => {}
            Err(err) => failed = Some(err),
        }
    }

    let ttl = if failed.is_some() {
        ERROR_TTL
    } else {
        MISS_TTL
    };
    state.cache.set(&key, ttl, &None::<Vec<u8>>).await.ok();
    if let Some(err) = failed {
        tracing::debug!("fetching the icon of {id}: {err:#}");
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(image: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn decode(head: &[u8]) -> RgbaImage {
        image::load_from_memory(head).unwrap().to_rgba8()
    }

    #[test]
    fn the_head_is_the_face_under_the_hat() {
        let mut skin = RgbaImage::new(64, 64);
        for x in 8..16 {
            for y in 8..16 {
                // a transparent face pixel still shows: the face is opaque
                skin.put_pixel(x, y, image::Rgba([200, 0, 0, if x == 8 { 0 } else { 255 }]));
            }
        }
        // one hat pixel over the face's top left corner
        skin.put_pixel(40, 8, image::Rgba([0, 0, 255, 255]));

        let head = decode(&render_head(&png(&skin)).unwrap());
        assert_eq!(head.dimensions(), (64, 64));
        // 8 pixels each
        assert_eq!(head.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(head.get_pixel(7, 7).0, [0, 0, 255, 255]);
        assert_eq!(head.get_pixel(8, 0).0, [200, 0, 0, 255]);
        assert_eq!(head.get_pixel(0, 8).0, [200, 0, 0, 255]);
        assert_eq!(head.get_pixel(63, 63).0, [200, 0, 0, 255]);
    }

    #[test]
    fn an_old_skin_with_an_opaque_hat_has_none() {
        let mut skin = RgbaImage::from_pixel(64, 32, image::Rgba([0, 0, 0, 255]));
        for x in 8..16 {
            for y in 8..16 {
                skin.put_pixel(x, y, image::Rgba([10, 120, 10, 255]));
            }
        }
        let head = decode(&render_head(&png(&skin)).unwrap());
        assert_eq!(head.get_pixel(0, 0).0, [10, 120, 10, 255]);

        // with one see-through pixel the hat counts
        skin.put_pixel(63, 31, image::Rgba([0, 0, 0, 0]));
        let head = decode(&render_head(&png(&skin)).unwrap());
        assert_eq!(head.get_pixel(0, 0).0, [0, 0, 0, 255]);

        // a high resolution skin is scaled down
        let big = RgbaImage::from_pixel(128, 128, image::Rgba([1, 2, 3, 255]));
        assert_eq!(
            decode(&render_head(&png(&big)).unwrap()).dimensions(),
            (64, 64)
        );
        assert!(render_head(&png(&RgbaImage::new(30, 30))).is_err());
        assert!(render_head(b"not a png").is_err());
    }

    #[test]
    fn skin_urls_come_from_mojang_only() {
        let textures = serde_json::json!({
            "textures": {"SKIN": {"url": "http://textures.minecraft.net/texture/abc123"}}
        });
        let profile = serde_json::json!({
            "id": "069a79f444e94726a5befca90e38aaf5",
            "properties": [{
                "name": "textures",
                "value": base64::engine::general_purpose::STANDARD.encode(textures.to_string()),
            }],
        });
        assert_eq!(
            skin_from_profile(&profile).as_deref(),
            Some("https://textures.minecraft.net/texture/abc123")
        );
        assert_eq!(skin_host_url("https://example.com/texture/abc123"), None);
        assert_eq!(
            skin_from_profile(&serde_json::json!({"properties": []})),
            None
        );
    }

    #[test]
    fn icons_are_looked_up_by_id() {
        assert_eq!(wiki_title("heart_of_the_sea"), "Heart_of_the_Sea");
        assert_eq!(wiki_title("carrot_on_a_stick"), "Carrot_on_a_Stick");
        assert_eq!(wiki_title("leather_chestplate"), "Leather_Tunic");
        assert_eq!(wiki_title("tnt"), "TNT");

        assert_eq!(
            icon_urls("minecraft:diamond_sword", "1.21.4"),
            vec![
                "https://minecraft.wiki/images/Invicon_Diamond_Sword.png",
                "https://assets.mcasset.cloud/1.21.4/assets/minecraft/textures/item/diamond_sword.png",
                "https://assets.mcasset.cloud/1.21.4/assets/minecraft/textures/block/diamond_sword.png",
            ]
        );
        assert!(icon_urls("create:wrench", "latest").is_empty());
        assert!(icon_urls("minecraft:../../etc", "latest").is_empty());
        assert!(icon_urls("minecraft:stone", "1.21/../x").is_empty());
        assert!(valid_version("24w14a") && valid_version("1.20.5-rc1") && valid_version("latest"));
        assert!(!valid_version("") && !valid_version("-1"));
    }
}
