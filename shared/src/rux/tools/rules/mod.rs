//! plugged.host: rux's standing rules, watched channels and moderator roles as tools (add, list,
//! edit, remove rules; watch a channel; make a role a moderator role), and what the checks share
//! with them.
//!
//! A standing rule is an admin's order rux keeps following: a member message with one of its
//! keywords is checked by AI against what breaks it and what does not, and rux acts on a clear
//! break ([`enforce`]). What a rule is and how it reads is in [`model`] (with the always-on
//! guidelines of watched channels); the facts about a server's host the checks give the AI in
//! [`facts`]; the watched channels in [`watches`]. Deciding whether a message breaks a rule is the
//! watch's (`rux::watch`): it calls into these.

pub mod enforce;
pub mod facts;
pub mod model;
pub mod tools;
pub mod watches;

use super::Tool;
use crate::rux::say::Strings;

/// The group's lines (`rule.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[
    (
        "rule.warn",
        ":member, you were warned: :reason",
        ":member, du wurdest verwarnt: :reason",
    ),
    (
        "rule.timeout",
        ":member, you are timed out: :reason",
        ":member, du bist stummgeschaltet: :reason",
    ),
    ("rule.acted", ":member: :reason", ":member: :reason"),
];

pub fn tools() -> Vec<Box<dyn Tool>> {
    tools::tools()
}
