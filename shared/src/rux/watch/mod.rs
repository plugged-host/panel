//! plugged.host: what rux checks by itself: spam in every channel (without AI), every message in
//! a watched channel and messages that hit a standing rule's keywords (checked by the AI, never
//! on premium models). A stub until the checks are ported: nothing is checked.

use crate::rux::{config::Config, context::Me, discord::Rest, say::Strings};
use serde_json::Value;

/// The checks' lines: `(key, English, German)`.
pub const STRINGS: Strings = &[];

/// A `MESSAGE_CREATE` on a server rux works on (orders included: staff are left alone by the
/// checks themselves). It runs in the gateway's event worker, in order: anything slow (an AI
/// check, a Discord call) is spawned or queued, never awaited here.
pub async fn on_message(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
    _me: &Me,
    _message: &Value,
) -> Result<(), anyhow::Error> {
    Ok(())
}

/// Every minute on the primary instance (`rux::tick`).
pub async fn tick(
    _state: &crate::State,
    _config: &Config,
    _rest: &Rest,
) -> Result<(), anyhow::Error> {
    Ok(())
}
