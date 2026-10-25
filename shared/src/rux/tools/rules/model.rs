//! plugged.host: what a standing rule is and how it reads: its keywords (normalized, everyday
//! words dropped), whether a message hits them, where it applies, how the checks describe it to
//! the AI ([`brief`], [`judging`]) and how the tools show it to the model ([`summary`]). Also the
//! always-on guidelines of watched channels ([`guidelines`]), which the checks and `list_rules`
//! share.

use super::super::{ToolError, moderation::common::without_nulls};
use crate::rux::{models::Rule, text, time};
use serde_json::{Value, json};

/// What a rule can do: warn (counts towards the warning steps), time out, delete the message
/// only, kick or ban.
pub const ACTIONS: [&str; 5] = ["warn", "timeout", "delete", "kick", "ban"];

/// A rule's keywords, at most.
pub const MAX_KEYWORDS: usize = 80;
/// A rule's keywords, at least (after the everyday words are dropped).
pub const MIN_KEYWORDS: usize = 3;

/// Single words too common to be keywords: they would send almost every message to the AI.
const COMMON_WORDS: &[&str] = &[
    "a", "about", "add", "added", "adding", "all", "an", "and", "any", "anyone", "are", "as", "at",
    "be", "but", "by", "can", "coming", "do", "does", "for", "from", "get", "go", "got", "have",
    "help", "hey", "how", "i", "if", "in", "is", "it", "just", "like", "list", "make", "me",
    "more", "my", "need", "new", "no", "not", "now", "of", "ok", "on", "one", "or", "place",
    "places", "please", "pls", "plz", "r", "server", "servers", "so", "soon", "that", "the",
    "there", "they", "this", "to", "u", "up", "we", "what", "when", "where", "which", "who", "why",
    "will", "with", "would", "yes", "you", "your", "want", "wanna", "gonna", "lol", "yo", "ya",
    "ur",
];

/// How the AI judges messages, for the rule checks and the watched channels' checks. `:app` is
/// the server's host ([`judging`]).
pub const JUDGING: [&str; 5] = [
    "How to judge:",
    "- Members write casually: slang, typos, abbreviations, other languages. Judge what they mean, not the exact words.",
    "- Only a clear break counts. Not a break: mentioning the topic in passing, answering or quoting someone, asking about a rule, joking between friends, or complaining about :app.",
    "- What a rule says does NOT break it always wins, and so do the facts about :app above.",
    "- When in doubt, it is not a break.",
];

/// What every message in a watched channel is held to, besides the standing rules (while the
/// settings' `watch_guidelines` is on). `:app` is the server's host ([`guidelines`]).
pub const GUIDELINES: [(&str, &str); 5] = [
    (
        "G1",
        "Scam or phishing: trying to trick members with fake giveaways or free Nitro, fake login or download links, account or crypto scams. Not: saying one was scammed or hacked, warning others about a scam, or :app's own links",
    ),
    ("G2", "Slurs or hate against a group of people"),
    (
        "G3",
        "Harassing, threatening or bullying someone. Not: swearing in general, obvious jokes between friends, or being angry at :app",
    ),
    ("G4", "Sexual or gore content"),
    (
        "G6",
        "Advertising another Discord server, game host or paid service. Not: :app's own links",
    ),
];

/// [`JUDGING`] for a server whose host is `host`.
pub fn judging(host: &str) -> Vec<String> {
    JUDGING
        .iter()
        .map(|line| line.replace(":app", host))
        .collect()
}

/// [`GUIDELINES`] for a server whose host is `host`: `(key, text)`.
pub fn guidelines(host: &str) -> Vec<(&'static str, String)> {
    GUIDELINES
        .iter()
        .map(|(key, line)| (*key, line.replace(":app", host)))
        .collect()
}

/// Accents off the Latin letters ("é" → "e", "ß" → "ss"), so a keyword matches however it is
/// typed. Other scripts stay as they are.
fn fold(c: char, out: &mut String) {
    let plain = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        '’' | '‘' | '`' => "'",
        '“' | '”' => "\"",
        _ => {
            out.push(c);
            return;
        }
    };
    out.push_str(plain);
}

/// A message or keyword as rules compare them: lowercase, Latin accents off, whitespace as
/// single spaces.
pub fn normalize(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.to_lowercase().chars() {
        fold(c, &mut folded);
    }

    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether a normalized text contains a (normalized) keyword: a phrase anywhere, a single word
/// only as a whole word ("ram" is not in "program").
pub fn has_keyword(text: &str, keyword: &str) -> bool {
    if keyword.is_empty() {
        return false;
    }
    if keyword.contains(' ') {
        return text.contains(keyword);
    }

    text.match_indices(keyword).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + keyword.len()..].chars().next();

        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// A rule's keywords as rux keeps them: normalized and unique, without everyday single words
/// (`dropped`). At least [`MIN_KEYWORDS`] must be left; at most [`MAX_KEYWORDS`] are kept.
pub fn keywords(raw: &[String]) -> Result<(Vec<String>, Vec<String>), ToolError> {
    let mut all: Vec<String> = Vec::new();
    for keyword in raw {
        let keyword = text::cut(&normalize(keyword), 60, "").trim().to_string();
        if keyword.chars().count() >= 2 && !all.contains(&keyword) {
            all.push(keyword);
        }
    }

    let (dropped, mut kept): (Vec<String>, Vec<String>) = all
        .into_iter()
        .partition(|keyword| !keyword.contains(' ') && COMMON_WORDS.contains(&keyword.as_str()));
    if kept.len() < MIN_KEYWORDS {
        let examples: Vec<&str> = if dropped.is_empty() {
            vec!["new", "add", "when"]
        } else {
            dropped.iter().take(5).map(String::as_str).collect()
        };

        return Err(ToolError::Invalid(format!(
            "Give at least {MIN_KEYWORDS} specific keywords (topic words or phrases, not everyday words like {}).",
            examples.join(", ")
        )));
    }
    kept.truncate(MAX_KEYWORDS);

    Ok((kept, dropped))
}

/// The ids in a JSON list of ids (written as text or as numbers).
fn ids(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|id| match id {
            Value::String(id) => Some(id.clone()),
            Value::Number(id) => Some(id.to_string()),
            _ => None,
        })
        .collect()
}

impl Rule {
    pub fn keyword_list(&self) -> Vec<String> {
        self.keywords
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }

    /// The channels it applies in (none: the whole server).
    pub fn channel_list(&self) -> Vec<String> {
        ids(self.channel_ids.as_ref())
    }

    /// Whether a normalized message hits one of its keywords.
    pub fn matches(&self, normalized: &str) -> bool {
        self.keyword_list()
            .iter()
            .any(|keyword| has_keyword(normalized, &normalize(keyword)))
    }

    /// Whether it applies in a channel (or the thread of one: `parent`).
    pub fn applies_in(&self, channel_id: &str, parent_id: Option<&str>) -> bool {
        let channels = self.channel_list();

        channels.is_empty()
            || channels.iter().any(|id| id == channel_id)
            || parent_id.is_some_and(|parent| channels.iter().any(|id| id == parent))
    }

    /// What its member is told: its reason, else its description.
    pub fn told(&self) -> String {
        match self.reason.as_deref().map(str::trim) {
            Some(reason) if !reason.is_empty() => reason.to_string(),
            _ => {
                let mut description = self.description.trim().chars();
                description
                    .next()
                    .map(|first| first.to_uppercase().chain(description).collect())
                    .unwrap_or_default()
            }
        }
    }
}

/// A rule for the model: what breaks it, what does not, what it does, where, and how often it
/// was checked and acted.
pub fn summary(rule: &Rule) -> Value {
    let channels = rule.channel_list();

    without_nulls(json!({
        "id": rule.id,
        "when": rule.description,
        "not_when": rule.exceptions,
        "action": match rule.duration_seconds.filter(|seconds| *seconds > 0) {
            Some(seconds) => format!("{} for {}", rule.action, time::human_duration(i64::from(seconds))),
            None => rule.action.clone(),
        },
        "deletes_message": rule.delete_message.then_some(true),
        "reason": rule.reason,
        "only_in": (!channels.is_empty())
            .then(|| channels.iter().map(|id| format!("<#{id}>")).collect::<Vec<_>>()),
        "keywords": rule.keywords,
        "active": rule.active,
        "messages_checked": rule.hits,
        "times_acted": rule.actions,
    }))
}

fn trim_end(text: &str) -> &str {
    text.trim_end_matches([' ', '.'])
}

/// A rule as the checks give it to the AI: what breaks it, what does not, what the member is
/// told (the admin's intent, in their words) and where it applies.
pub fn brief(rule: &Rule) -> String {
    let mut parts = vec![format!(
        "R{} is broken by a member's message that {}.",
        rule.id,
        trim_end(&rule.description)
    )];
    if let Some(exceptions) = rule
        .exceptions
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(format!("It is NOT broken by: {}.", trim_end(exceptions)));
    }
    if let Some(reason) = rule
        .reason
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(format!("(Members who break it are told: \"{reason}\")"));
    }
    let channels = rule.channel_list();
    if !channels.is_empty() {
        parts.push(format!(
            "(Only in {}.)",
            channels
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    parts.join(" ")
}

/// "Name: text" of the message a message replies to (Discord sends it along), at most 400
/// characters.
pub fn replied_to(message: &Value) -> Option<String> {
    let replied = &message["referenced_message"];
    let content = replied["content"].as_str().unwrap_or_default().trim();
    if content.is_empty() {
        return None;
    }

    let author = replied["author"]["global_name"]
        .as_str()
        .or_else(|| replied["author"]["username"].as_str())
        .unwrap_or("someone");

    Some(text::cut(
        &format!("{author}: {}", content.replace('\n', " ")),
        400,
        "",
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn rule() -> Rule {
        let at = chrono::DateTime::from_timestamp(1_791_108_000, 0)
            .unwrap()
            .naive_utc();

        Rule {
            id: 4,
            guild_id: "100000000000000001".into(),
            description: "asks for a new server location. ".into(),
            exceptions: Some("asking about a location that exists.".into()),
            keywords: json!(["location", "new country", "singapore"]),
            action: "warn".into(),
            duration_seconds: None,
            delete_message: false,
            reason: None,
            channel_ids: None,
            active: true,
            created_by_discord_id: "100000000000000002".into(),
            created_by_account_id: None,
            created_by_user_uuid: None,
            hits: 12,
            actions: 2,
            last_action: None,
            created: at,
            updated: at,
        }
    }

    #[test]
    fn text_is_normalized() {
        assert_eq!(normalize("  Café   SÜD\n\tStraße "), "cafe sud strasse");
        assert_eq!(normalize("Привет Мир"), "привет мир");
        assert_eq!(normalize("Don’t"), "don't");
    }

    #[test]
    fn single_words_match_whole_words_only() {
        assert!(has_keyword("need more ram please", "ram"));
        assert!(has_keyword("ram?", "ram"));
        assert!(!has_keyword("my program crashed", "ram"));
        assert!(!has_keyword("ramen time", "ram"));
        assert!(has_keyword("ramen, then ram", "ram"));
        assert!(has_keyword("open a new country pls", "new country"));
        assert!(!has_keyword("anything", ""));
        assert!(has_keyword("über ram", "ram"));
        assert!(!has_keyword("überram", "ram"));
    }

    #[test]
    fn keywords_drop_everyday_words() {
        let raw: Vec<String> = [
            "Location",
            "location",
            "new",
            "Singapur",
            "add",
            "x",
            "new country",
        ]
        .iter()
        .map(|k| k.to_string())
        .collect();
        let (kept, dropped) = keywords(&raw).unwrap();
        assert_eq!(kept, vec!["location", "singapur", "new country"]);
        assert_eq!(dropped, vec!["new", "add"]);

        let err = keywords(&["new".into(), "add".into(), "location".into()]).unwrap_err();
        assert!(
            err.for_model().contains("at least 3"),
            "{}",
            err.for_model()
        );

        let many: Vec<String> = (0..100).map(|n| format!("word{n}")).collect();
        assert_eq!(keywords(&many).unwrap().0.len(), MAX_KEYWORDS);
    }

    #[test]
    fn rules_match_and_apply() {
        let mut rule = rule();
        assert!(rule.matches(&normalize("Any new  Country soon?")));
        assert!(rule.matches(&normalize("SINGAPORE when")));
        assert!(!rule.matches(&normalize("relocation")));
        assert!(rule.applies_in("100000000000000009", None));

        rule.channel_ids = Some(json!(["100000000000000007"]));
        assert!(rule.applies_in("100000000000000007", None));
        assert!(rule.applies_in("100000000000000009", Some("100000000000000007")));
        assert!(!rule.applies_in("100000000000000009", None));
    }

    #[test]
    fn rules_read_for_the_ai_and_the_model() {
        let mut rule = rule();
        assert_eq!(
            brief(&rule),
            "R4 is broken by a member's message that asks for a new server location. It is NOT broken by: asking about a location that exists."
        );
        assert_eq!(rule.told(), "Asks for a new server location.");

        rule.action = "timeout".into();
        rule.duration_seconds = Some(3600);
        rule.reason = Some("Please wait for the announcement.".into());
        rule.channel_ids = Some(json!(["100000000000000007"]));
        assert!(brief(&rule).ends_with(
            "(Members who break it are told: \"Please wait for the announcement.\") (Only in #100000000000000007.)"
        ));
        assert_eq!(rule.told(), "Please wait for the announcement.");

        let summary = summary(&rule);
        assert_eq!(summary["action"], "timeout for 1 hour");
        assert_eq!(summary["only_in"], json!(["<#100000000000000007>"]));
        assert!(summary.get("deletes_message").is_none());
        assert_eq!(summary["messages_checked"], 12);
    }

    #[test]
    fn hosts_are_filled_in() {
        assert!(judging("Example")[2].ends_with("complaining about Example."));
        assert!(guidelines("Example")[0].1.ends_with("Example's own links"));
        assert_eq!(guidelines("Example").len(), GUIDELINES.len());
    }

    #[test]
    fn replies_are_read() {
        let message = json!({
            "referenced_message": {
                "content": " line one\nline two ",
                "author": { "username": "ann" },
            },
        });
        assert_eq!(
            replied_to(&message).as_deref(),
            Some("ann: line one line two")
        );
        assert_eq!(replied_to(&json!({})), None);
    }
}
