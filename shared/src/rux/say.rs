//! plugged.host: what rux posts on Discord, in its own language (the settings' `locale`, English
//! or German). Each part of rux keeps its lines in a table of `(key, English, German)` next to
//! its code ([`CORE`], [`super::tools`]' groups, [`super::watch`]); [`say`] looks a key up in
//! all of them. `:name` placeholders are filled in, longest name first; a line with a `|` is
//! "one|more" for [`say_count`].

/// A part of rux's lines: `(key, English, German)`.
pub type Strings = &'static [(&'static str, &'static str, &'static str)];

/// The agent's own replies.
pub const CORE: Strings = &[
    (
        "reply.not_admin",
        "I only take orders from :portal admins.",
        "Ich nehme nur Befehle von :portal-Admins an.",
    ),
    (
        "reply.noprofit_down",
        "I cannot reach :portal to check who you are. Try again in a minute.",
        "Ich erreiche :portal nicht, um zu prüfen, wer du bist. Versuch es in einer Minute noch einmal.",
    ),
    (
        "reply.unavailable",
        "I cannot think right now: the AI is unavailable. Try again in a few minutes.",
        "Ich kann gerade nicht denken: Die KI ist nicht erreichbar. Versuch es in ein paar Minuten noch einmal.",
    ),
    (
        "reply.too_fast",
        "Give me a moment, I am still on your last order.",
        "Einen Moment, ich bin noch bei deinem letzten Befehl.",
    ),
    (
        "reply.failed",
        "Something went wrong on my side. Nothing more was done.",
        "Bei mir ist etwas schiefgegangen. Es wurde nichts weiter getan.",
    ),
    (
        "reply.done_without_summary",
        "Done, but the AI is too busy right now to sum it up. What I did:",
        "Erledigt, aber die KI ist gerade zu ausgelastet für eine Zusammenfassung. Was ich getan habe:",
    ),
    ("reply.done", "Done.", "Erledigt."),
];

/// Every table, in lookup order.
fn tables() -> [Strings; 7] {
    [
        CORE,
        super::tools::moderation::STRINGS,
        super::tools::giveaways::STRINGS,
        super::tools::rules::STRINGS,
        super::tools::credits::STRINGS,
        super::tools::servers::STRINGS,
        super::watch::STRINGS,
    ]
}

fn line(locale: &str, key: &str) -> Option<&'static str> {
    tables()
        .into_iter()
        .flat_map(|table| table.iter())
        .find(|(k, _, _)| *k == key)
        .map(|(_, en, de)| match locale {
            "de" if !de.is_empty() => *de,
            _ => *en,
        })
}

/// `:name` placeholders filled in, longest name first (so `:portal` never eats `:port`).
pub fn fill(text: &str, replace: &[(&str, &str)]) -> String {
    let mut replace = replace.to_vec();
    replace.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));

    let mut out = text.to_string();
    for (name, value) in replace {
        out = out.replace(&format!(":{name}"), value);
    }

    out
}

/// The line `key` in `locale`, placeholders filled in; the key itself when no table has it.
pub fn say(locale: &str, key: &str, replace: &[(&str, &str)]) -> String {
    match line(locale, key) {
        Some(text) => fill(text, replace),
        None => {
            tracing::warn!("rux: no line for {key}");
            key.to_string()
        }
    }
}

/// A "one|more" line for `count`, with `:count` filled in too.
pub fn say_count(locale: &str, key: &str, count: i64, replace: &[(&str, &str)]) -> String {
    let Some(text) = line(locale, key) else {
        return key.to_string();
    };
    let text = match text.split_once('|') {
        Some((one, _)) if count == 1 => one,
        Some((_, more)) => more,
        None => text,
    };
    let count = count.to_string();
    let mut all = replace.to_vec();
    all.push(("count", &count));

    fill(text.trim(), &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_has_both_languages_once() {
        let mut seen = std::collections::HashSet::new();
        for table in tables() {
            for (key, en, de) in table.iter() {
                assert!(seen.insert(*key), "{key} is in two tables");
                assert!(!en.is_empty(), "{key} has no English");
                assert!(!de.is_empty(), "{key} has no German");
                assert_eq!(
                    en.contains('|'),
                    de.contains('|'),
                    "{key}: one language counts, the other does not"
                );
            }
        }
    }

    #[test]
    fn lines_are_filled_in() {
        assert_eq!(
            say("en", "reply.not_admin", &[("portal", "Example")]),
            "I only take orders from Example admins."
        );
        assert_eq!(
            say("de", "reply.not_admin", &[("portal", "Example")]),
            "Ich nehme nur Befehle von Example-Admins an."
        );
        assert_eq!(say("fr", "reply.done", &[]), "Done.");
        assert_eq!(say("en", "no.such.key", &[]), "no.such.key");
        assert_eq!(
            fill(":port :portal", &[("port", "1"), ("portal", "two")]),
            "1 two"
        );
    }
}
