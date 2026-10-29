//! plugged.host: rux's giveaways (create, list, edit, end, cancel, reroll), their Enter buttons
//! and their tick (draws, payouts).
//!
//! The prize is credits, tokens or days of a membership, paid to accounts linked to Discord
//! ([`service`]); the post's buttons are answered in [`entries`]; the tools are in [`tools`].

pub mod entries;
pub mod service;
pub mod tools;

use super::Tool;
use crate::rux::{config::Config, context::Me, discord::Rest, say::Strings};
use serde_json::Value;

/// The group's lines (`giveaway.*`, `entry.*`): `(key, English, German)`.
pub const STRINGS: Strings = &[
    // the post
    ("giveaway.enter", "Enter", "Teilnehmen"),
    ("giveaway.ended", "Ended", "Beendet"),
    ("giveaway.cancelled", "Cancelled", "Abgesagt"),
    (
        "giveaway.prize_each",
        ":credits credits for each winner",
        ":credits Credits für jeden Gewinner",
    ),
    ("giveaway.prize_one", ":credits credits", ":credits Credits"),
    // plugged.host: tokens and membership days as prizes
    (
        "giveaway.prize_tokens_one",
        "1 token|:count tokens",
        "1 Token|:count Tokens",
    ),
    (
        "giveaway.prize_tokens_each",
        "1 token for each winner|:count tokens for each winner",
        "1 Token für jeden Gewinner|:count Tokens für jeden Gewinner",
    ),
    (
        "giveaway.prize_days_one",
        "1 day of :membership|:count days of :membership",
        "1 Tag :membership|:count Tage :membership",
    ),
    (
        "giveaway.prize_days_each",
        "1 day of :membership for each winner|:count days of :membership for each winner",
        "1 Tag :membership für jeden Gewinner|:count Tage :membership für jeden Gewinner",
    ),
    (
        "giveaway.open_body",
        "Press **Enter** to join. Ends :relative (:absolute).",
        "Drücke **Teilnehmen**, um mitzumachen. Endet :relative (:absolute).",
    ),
    ("giveaway.winners", "Winners", "Gewinner"),
    ("giveaway.entries", "Entries", "Teilnehmer"),
    ("giveaway.rules", "Who can enter", "Wer teilnehmen kann"),
    (
        "giveaway.rule_link",
        "Your Discord account is linked to :portal.",
        "Dein Discord-Konto ist mit :portal verknüpft.",
    ),
    (
        "giveaway.rule_exclude",
        "You did not win the last giveaway.|You did not win any of the last :count giveaways.",
        "Du hast das letzte Gewinnspiel nicht gewonnen.|Du hast keines der letzten :count Gewinnspiele gewonnen.",
    ),
    ("giveaway.footer", "Giveaway #:id", "Gewinnspiel #:id"),
    (
        "giveaway.ended_body",
        "Ended :relative.",
        "Beendet :relative.",
    ),
    (
        "giveaway.cancelled_body",
        "This giveaway was cancelled.",
        "Dieses Gewinnspiel wurde abgesagt.",
    ),
    (
        "giveaway.no_winner",
        "Nobody eligible entered.",
        "Niemand Berechtigtes hat teilgenommen.",
    ),
    // the announcement
    (
        "giveaway.announce",
        "Congratulations :winners! You won **:prize**.",
        "Glückwunsch :winners! Du hast **:prize** gewonnen.",
    ),
    (
        "giveaway.announce_paid",
        "The credits are already in your :portal account.",
        "Die Credits sind schon auf deinem :portal-Konto.",
    ),
    (
        "giveaway.announce_pending",
        "Credits are on their way to your :portal account.",
        "Die Credits sind auf dem Weg zu deinem :portal-Konto.",
    ),
    (
        "giveaway.announce_paid_tokens",
        "The tokens are already in your :app account.",
        "Die Tokens sind schon auf deinem :app-Konto.",
    ),
    (
        "giveaway.announce_pending_tokens",
        "The tokens are on their way to your :app account.",
        "Die Tokens sind auf dem Weg zu deinem :app-Konto.",
    ),
    (
        "giveaway.announce_paid_days",
        ":membership is already running on your :app account.",
        ":membership läuft schon auf deinem :app-Konto.",
    ),
    (
        "giveaway.announce_pending_days",
        ":membership is on its way to your :app account.",
        ":membership ist auf dem Weg zu deinem :app-Konto.",
    ),
    (
        "giveaway.announce_none",
        "The **:prize** giveaway ended without a winner: nobody eligible entered.",
        "Das Gewinnspiel um **:prize** ist ohne Gewinner zu Ende: Niemand Berechtigtes hat teilgenommen.",
    ),
    (
        "giveaway.announce_reroll",
        "New draw: congratulations :winners! You won **:prize**.",
        "Neue Ziehung: Glückwunsch :winners! Du hast **:prize** gewonnen.",
    ),
    // the answers to the buttons (only the presser sees them)
    (
        "entry.joined",
        "You are in. Winners are drawn :relative.",
        "Du bist dabei. Die Gewinner werden :relative gezogen.",
    ),
    (
        "entry.already",
        "You are already in. Winners are drawn :relative.",
        "Du bist schon dabei. Die Gewinner werden :relative gezogen.",
    ),
    ("entry.leave", "Leave the giveaway", "Gewinnspiel verlassen"),
    (
        "entry.left",
        "You left the giveaway.",
        "Du hast das Gewinnspiel verlassen.",
    ),
    (
        "entry.closed",
        "This giveaway has ended.",
        "Dieses Gewinnspiel ist vorbei.",
    ),
    (
        "entry.excluded",
        "You won the last giveaway, so you cannot enter this one.|You won one of the last :count giveaways, so you cannot enter this one.",
        "Du hast das letzte Gewinnspiel gewonnen und kannst bei diesem nicht mitmachen.|Du hast eines der letzten :count Gewinnspiele gewonnen und kannst bei diesem nicht mitmachen.",
    ),
    (
        "entry.unlinked",
        "To enter, link your Discord account to your :portal account, then press Enter again.",
        "Verknüpfe zum Teilnehmen dein Discord-Konto mit deinem :portal-Konto und drücke dann noch einmal auf Teilnehmen.",
    ),
    ("entry.link", "Link on :portal", "Auf :portal verknüpfen"),
    (
        "entry.unavailable",
        "Entering does not work right now. Try again in a minute.",
        "Teilnehmen klappt gerade nicht. Versuch es in einer Minute noch einmal.",
    ),
];

/// The `custom_id` prefix of the giveaways' buttons.
pub const BUTTON_PREFIX: &str = "rux:gw:";

pub fn tools() -> Vec<Box<dyn Tool>> {
    tools::tools()
}

/// An `INTERACTION_CREATE` whose `custom_id` starts with [`BUTTON_PREFIX`], on one of the servers
/// rux works on; `true` when it was answered. It runs in the gateway's event worker and Discord
/// wants an answer within 3 seconds: answer first, spawn anything slow.
pub async fn on_interaction(
    state: &crate::State,
    config: &Config,
    rest: &Rest,
    me: &Me,
    interaction: &Value,
) -> Result<bool, anyhow::Error> {
    entries::on_interaction(state, config, rest, me, interaction).await
}

/// Every ten seconds on the primary instance (`rux::tick`): draws the giveaways whose time is up,
/// so winners are not left waiting for the minute.
pub async fn due(state: &crate::State, config: &Config, rest: &Rest) -> Result<(), anyhow::Error> {
    service::due(state, config, rest).await
}

/// Every minute on the primary instance (`rux::tick`): payouts noprofit.space did not take yet,
/// entry counts, stalled draws.
pub async fn tick(state: &crate::State, config: &Config, rest: &Rest) -> Result<(), anyhow::Error> {
    service::recover_stalled(state).await?;
    if let Err(err) = service::retry_payouts(state).await {
        tracing::warn!("rux: retrying giveaway payouts failed: {err:#}");
    }
    service::refresh_counts(state, config, rest).await?;

    Ok(())
}
