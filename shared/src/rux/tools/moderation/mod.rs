//! plugged.host: rux's moderation tools (looking members up, warnings, timeouts, kicks, bans,
//! messages, channels, roles, the case log) and their tick (bans that ran out are lifted).
//!
//! Everything rux does to someone is a case ([`cases`]): who ordered it, on whom, why, on which
//! server, posted to that server's mod log. Moderators (a rux moderator role) get the moderation
//! tools but never act on staff, admins, themselves or the bot ([`common::guard`]), never in the
//! mod log, and only read or delete in channels they can see on Discord. The actions themselves
//! are in [`service`], shared with the standing rules and the checks.

pub mod cases;
pub mod common;
pub mod members;
pub mod messages;
pub mod server;
pub mod service;

use super::Tool;
use crate::rux::{config::Config, discord::Rest, say::Strings};

/// The group's lines (`notice.*`, `log.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[
    // the member's DM
    (
        "notice.warn",
        "You were warned in **:server**. :reason",
        "Du wurdest auf **:server** verwarnt. :reason",
    ),
    (
        "notice.timeout",
        "You were timed out in **:server** until :until. :reason",
        "Du bist auf **:server** bis :until stummgeschaltet. :reason",
    ),
    (
        "notice.kick",
        "You were kicked from **:server**. :reason",
        "Du wurdest von **:server** gekickt. :reason",
    ),
    (
        "notice.ban",
        "You were banned from **:server**. :reason",
        "Du wurdest von **:server** gebannt. :reason",
    ),
    (
        "notice.ban_for",
        "You were banned from **:server** until :until. :reason",
        "Du wurdest von **:server** bis :until gebannt. :reason",
    ),
    (
        "notice.warnings_reached",
        "Automatic: you have 1 warning.|Automatic: you have :count warnings.",
        "Automatisch: Du hast 1 Verwarnung.|Automatisch: Du hast :count Verwarnungen.",
    ),
    ("notice.reason", "Reason: :reason", "Grund: :reason"),
    // the mod log: what happened
    ("log.warn", "Warned", "Verwarnt"),
    ("log.timeout", "Timed out", "Stummgeschaltet"),
    (
        "log.untimeout",
        "Timeout removed",
        "Stummschaltung aufgehoben",
    ),
    ("log.kick", "Kicked", "Gekickt"),
    ("log.ban", "Banned", "Gebannt"),
    ("log.unban", "Unbanned", "Entbannt"),
    ("log.delete", "Message deleted", "Nachricht gelöscht"),
    ("log.purge", "Messages deleted", "Nachrichten gelöscht"),
    ("log.slowmode", "Slowmode changed", "Slowmode geändert"),
    ("log.lock", "Channel locked", "Kanal gesperrt"),
    ("log.unlock", "Channel unlocked", "Kanal entsperrt"),
    ("log.role_add", "Role added", "Rolle vergeben"),
    ("log.role_remove", "Role removed", "Rolle entfernt"),
    ("log.rule_add", "Rule added", "Regel hinzugefügt"),
    ("log.rule_remove", "Rule removed", "Regel entfernt"),
    ("log.rule_edit", "Rule changed", "Regel geändert"),
    ("log.watch_add", "Channel watched", "Kanal wird überwacht"),
    (
        "log.watch_remove",
        "Channel no longer watched",
        "Kanal wird nicht mehr überwacht",
    ),
    (
        "log.mod_role_add",
        "Moderator role added",
        "Moderatorrolle hinzugefügt",
    ),
    (
        "log.mod_role_remove",
        "Moderator role removed",
        "Moderatorrolle entfernt",
    ),
    (
        "log.clear_warnings",
        "Warnings cleared",
        "Verwarnungen gelöscht",
    ),
    ("log.nickname", "Nickname changed", "Spitzname geändert"),
    ("log.credits", "Credits given", "Credits vergeben"),
    // the mod log: its fields
    ("log.member", "Member", "Mitglied"),
    ("log.channel", "Channel", "Kanal"),
    ("log.by", "Ordered by", "Angeordnet von"),
    ("log.reason", "Reason", "Grund"),
    ("log.duration", "Duration", "Dauer"),
    ("log.details", "Details", "Details"),
    ("log.said", "Message", "Nachricht"),
    ("log.by_rux", ":name, automatically", ":name, automatisch"),
    ("log.by_rule", ":name, rule #:rule", ":name, Regel #:rule"),
    (
        "log.warnings",
        "1 warning in total|:count warnings in total",
        "1 Verwarnung insgesamt|:count Verwarnungen insgesamt",
    ),
    ("log.case", "Case #:id", "Fall #:id"),
    ("log.off", "off", "aus"),
    ("log.detail_deleted", ":count deleted", ":count gelöscht"),
    (
        "log.detail_cleared",
        ":cleared cleared, :left left",
        ":cleared gelöscht, :left übrig",
    ),
    ("log.no_nickname", "no nickname", "kein Spitzname"),
    (
        "log.detail_changed",
        "changed: :fields",
        "geändert: :fields",
    ),
    (
        "log.detail_credits",
        "+:credits credits",
        "+:credits Credits",
    ),
    (
        "log.ban_ran_out",
        ":name: the ban from case #:id ran out",
        ":name: Der Bann aus Fall #:id ist abgelaufen",
    ),
    (
        "log.ban_ended",
        "The ban from case #:id ran out.",
        "Der Bann aus Fall #:id ist abgelaufen.",
    ),
];

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(members::LookupMember),
        Box::new(members::WarnMember),
        Box::new(members::TimeoutMember),
        Box::new(members::KickMember),
        Box::new(members::BanMember),
        Box::new(members::UnbanUser),
        Box::new(members::ListCases),
        Box::new(members::ListBans),
        Box::new(members::SetNickname),
        Box::new(messages::ReadMessages),
        Box::new(messages::DeleteMessages),
        Box::new(messages::PurgeMessages),
        Box::new(server::ChannelSettings),
        Box::new(server::ManageRole),
        Box::new(server::RoleInfo),
        Box::new(messages::SendMessage),
        Box::new(messages::EditMessage),
        Box::new(messages::SendDm),
        Box::new(server::ServerOverview),
        Box::new(members::ClearWarnings),
    ]
}

/// Every minute on the primary instance (`rux::tick`): lifts the bans for a while whose time is
/// up.
pub async fn tick(state: &crate::State, config: &Config, rest: &Rest) -> Result<(), anyhow::Error> {
    let lifted = cases::lift_expired_bans(state, config, rest).await?;
    if lifted > 0 {
        tracing::info!(lifted, "rux: bans that ran out were lifted");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rux::access::Level;

    #[test]
    fn moderators_get_moderation_only() {
        let moderator: Vec<&str> = tools()
            .iter()
            .filter(|tool| tool.level() == Level::Moderator)
            .map(|tool| tool.name())
            .collect();

        assert_eq!(
            moderator,
            vec![
                "lookup_member",
                "warn_member",
                "timeout_member",
                "kick_member",
                "ban_member",
                "unban_user",
                "list_cases",
                "list_bans",
                "set_nickname",
                "read_messages",
                "delete_messages",
                "purge_messages",
                "channel_settings",
                "role_info",
                "server_overview",
                "clear_warnings",
            ]
        );
        // posting, DMs and roles are for admins
        for name in ["manage_role", "send_message", "edit_message", "send_dm"] {
            assert!(
                tools()
                    .iter()
                    .any(|tool| tool.name() == name && tool.level() == Level::Admin)
            );
        }
    }
}
