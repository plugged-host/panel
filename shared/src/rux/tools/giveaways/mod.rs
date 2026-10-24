//! plugged.host: rux's giveaways of credits (create, list, edit, end, cancel, reroll), their
//! Enter buttons and their tick (draws, payouts). A stub until the giveaways are ported: no
//! tools, no buttons, nothing on the tick.

use super::Tool;
use crate::rux::{config::Config, context::Me, discord::Rest, say::Strings};
use serde_json::Value;

/// The group's lines (`giveaway.*`, `entry.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[];

/// The `custom_id` prefix of the giveaways' buttons.
pub const BUTTON_PREFIX: &str = "rux:gw:";

pub fn tools() -> Vec<Box<dyn Tool>> {
    Vec::new()
}

/// An `INTERACTION_CREATE` whose `custom_id` starts with [`BUTTON_PREFIX`], on one of the servers
/// rux works on; `true` when it was answered. It runs in the gateway's event worker and Discord
/// wants an answer within 3 seconds: answer first, spawn anything slow.
pub async fn on_interaction(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
    _me: &Me,
    _interaction: &Value,
) -> Result<bool, anyhow::Error> {
    Ok(false)
}

/// Every ten seconds on the primary instance (`rux::tick`): draws the giveaways whose time is up,
/// so winners are not left waiting for the minute.
pub async fn due(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
) -> Result<(), anyhow::Error> {
    Ok(())
}

/// Every minute on the primary instance (`rux::tick`): payouts noprofit.space did not take yet,
/// entry counts, stalled draws.
pub async fn tick(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
) -> Result<(), anyhow::Error> {
    Ok(())
}
