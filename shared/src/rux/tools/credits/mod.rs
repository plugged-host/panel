//! plugged.host: rux's credit grant (`give_credits`, the settings' credit granters only). A stub
//! until it is ported: no tools.

use super::Tool;
use crate::rux::say::Strings;

/// The group's lines: `(key, English, German)`.
pub const STRINGS: Strings = &[];

pub fn tools() -> Vec<Box<dyn Tool>> {
    Vec::new()
}
