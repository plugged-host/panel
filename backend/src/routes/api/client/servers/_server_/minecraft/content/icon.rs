// plugged.host: the catalog's icons through the panel, so a hotlink block, a mixed-content rule or
// a privacy setting on the browser cannot leave a plugin or mod without its picture
// (docs/features/minecraft.md). Only the three catalog CDNs are fetched, and each answer is cached
// for a week.
use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Bigger than any catalog icon; a smaller cap keeps a hostile URL from filling the cache.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

mod get {
    use axum::{body::Body, extract::Query, http::StatusCode};
    use serde::Deserialize;
    use shared::{
        ApiError, GetState,
        models::{server::GetServer, user::GetPermissionManager},
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    const HOSTS: &[&str] = &[
        "cdn.modrinth.com",
        "media.forgecdn.net",
        "edge.forgecdn.net",
        "www.spigotmc.org",
        "spigotmc.org",
        "static.spigotmc.org",
    ];

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        url: String,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, description = "The icon", content_type = "image/*"),
        (status = BAD_REQUEST, body = ApiError),
        (status = UNAUTHORIZED, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = BAD_GATEWAY, body = ApiError),
    ), params(
        (
            "server" = uuid::Uuid,
            description = "The server ID",
            example = "123e4567-e89b-12d3-a456-426614174000",
        ),
        ("url" = String, Query, description = "The icon URL from the catalog"),
    ))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        _server: GetServer,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        permissions.has_server_permission("minecraft.content")?;

        if params.url.len() > 2048 {
            return ApiResponse::error("the icon URL is too long")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let Ok(url) = reqwest::Url::parse(&params.url) else {
            return ApiResponse::error("invalid icon URL")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        };
        if url.scheme() != "https" || !url.host_str().is_some_and(|host| HOSTS.contains(&host)) {
            return ApiResponse::error("that icon host is not allowed")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let key = compact_str::format_compact!(
            "minecraft_icon::{}",
            shared::crypt::token_digest(url.as_str())
        );
        let client = state.client.clone();
        let icon = state
            .cache
            .cached(&key, 7 * 24 * 60 * 60, || {
                super::fetch_icon(client.clone(), url.clone())
            })
            .await
            .map_err(|err| {
                tracing::warn!("failed to fetch a catalog icon: {err:#}");
                anyhow::anyhow!("the icon could not be fetched")
            });

        let (content_type, bytes) = match icon {
            Ok(icon) => icon,
            Err(err) => {
                return ApiResponse::error(format!("{err:#}"))
                    .with_status(StatusCode::BAD_GATEWAY)
                    .ok();
            }
        };

        ApiResponse::new(Body::from(bytes))
            .with_header("Content-Type", content_type)
            .with_header("Cache-Control", "public, max-age=604800, immutable")
            .with_header("X-Content-Type-Options", "nosniff")
            .ok()
    }
}

async fn fetch_icon(
    client: reqwest::Client,
    url: reqwest::Url,
) -> Result<(String, Vec<u8>), anyhow::Error> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .header("Accept", "image/avif,image/webp,image/png,image/*")
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("the icon host answered {}", response.status());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BYTES)
    {
        anyhow::bail!("the icon is too large");
    }

    let header_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    let bytes = response.bytes().await?;
    if bytes.len() as u64 > MAX_BYTES {
        anyhow::bail!("the icon is too large");
    }

    // Modrinth's CDN sometimes labels a WebP as image/png; trust the bytes, not the header, so
    // the browser gets a type it will actually decode.
    let sniffed = infer::get(&bytes)
        .map(|kind| kind.mime_type())
        .filter(|mime| mime.starts_with("image/"));
    let content_type = sniffed.unwrap_or(&header_type).to_string();
    if !content_type.starts_with("image/") {
        anyhow::bail!("the answer is not an image");
    }

    Ok((content_type, bytes.to_vec()))
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
