//! plugged.host: the FiveM pages of a FiveM (Cfx) server: server.cfg, players, the FXServer build
//! and edition, frameworks (docs/features/fivem.md).

use crate::models::{nest_egg::NestEgg, server::Server};

pub mod artifacts;
pub mod config;
pub mod frameworks;
pub mod players;
pub mod recipe;
pub mod runtime;
pub mod server_cfg;
pub mod txadmin;

/// The egg feature a FiveM (and RedM) egg carries.
pub const FEATURE: &str = "cfx";

/// Whether the egg runs FiveM or RedM: decided by its `cfx` feature, like the Minecraft pages by
/// `eula`.
pub fn is_fivem(egg: &NestEgg) -> bool {
    egg.features.iter().any(|feature| feature == FEATURE)
}

/// Refuses the FiveM pages for a server whose egg is not a Cfx one.
pub fn require(server: &Server) -> Result<(), crate::response::ApiResponse> {
    if is_fivem(&server.egg) {
        Ok(())
    } else {
        Err(
            crate::response::ApiResponse::error("this server does not run fivem")
                .with_status(axum::http::StatusCode::NOT_FOUND),
        )
    }
}

/// How long the last good answer from Cfx or GitHub is kept on top of its TTL.
const STALE_TTL: u64 = 24 * 60 * 60;

/// A value from elsewhere, cached for `ttl` seconds, with the last good copy kept for a day so a
/// bad minute at the source shows the last answer instead of nothing.
async fn remember<T, F, Fut>(state: &crate::State, key: &str, ttl: u64, fetch: F) -> Option<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Send + Sync,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let last = format!("{key}::last");
    if let Ok(Some(value)) = state.cache.get::<T>(key).await {
        return Some(value);
    }

    match fetch().await {
        Some(value) => {
            state.cache.set(key, ttl, &value).await.ok();
            state.cache.set(&last, STALE_TTL, &value).await.ok();
            Some(value)
        }
        None => state.cache.get::<T>(&last).await.ok().flatten(),
    }
}
