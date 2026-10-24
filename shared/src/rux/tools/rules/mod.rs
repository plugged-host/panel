//! plugged.host: rux's standing rules, watched channels and moderator roles as tools (add, list,
//! edit, remove rules; watch a channel; make a role a moderator role). A stub until they are
//! ported: no tools.

use super::Tool;
use crate::rux::say::Strings;

/// The group's lines (`rule.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[];

pub fn tools() -> Vec<Box<dyn Tool>> {
    Vec::new()
}
