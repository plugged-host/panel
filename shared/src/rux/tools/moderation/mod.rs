//! plugged.host: rux's moderation tools (looking members up, warnings, timeouts, kicks, bans,
//! messages, channels, roles, the case log) and their tick (bans that ran out are lifted).
//! A stub until the moderation tools are ported: no tools, nothing on the tick.

use super::Tool;
use crate::rux::{config::Config, discord::Rest, say::Strings};

/// The group's lines (`notice.*`, `log.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[];

pub fn tools() -> Vec<Box<dyn Tool>> {
    Vec::new()
}

/// Every minute on the primary instance (`rux::tick`).
pub async fn tick(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
) -> Result<(), anyhow::Error> {
    Ok(())
}
