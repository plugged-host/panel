//! plugged.host: the FiveM pages of a FiveM (Cfx) server: server.cfg, players, the FXServer build
//! and edition (docs/features/fivem.md).

use crate::models::{nest_egg::NestEgg, server::Server};

pub mod artifacts;
pub mod config;
pub mod players;
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
