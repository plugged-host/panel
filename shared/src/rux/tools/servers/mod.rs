//! plugged.host: which of rux's servers a DM acts on (`switch_server`). A stub until it is
//! ported: no tools. The tool finds the server and answers `ToolResult::ok(..).with_guild(id)`;
//! the agent checks the move (a DM only, a server rux works on, a moderator only one they
//! moderate), moves the rest of the order there and remembers the pick for the DM for a day
//! (`access::remember_dm_server`).

use super::Tool;
use crate::rux::say::Strings;

/// The group's lines: `(key, English, German)`.
pub const STRINGS: Strings = &[];

pub fn tools() -> Vec<Box<dyn Tool>> {
    Vec::new()
}
