//! plugged.host: the server's roles as rux reads them out to the model. An order's prompt lists
//! them highest first ([`listing`]); the checks name the roles of a message's author and the roles
//! it pings ([`Names`]), from the roles kept a minute in the cache ([`Names::load`]), and fall back
//! to the ids when they cannot be read. Role names are the server's own (its staff set them) and
//! still go in as one plain line each ([`text::name`]).

use super::{access, discord::Rest, text};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

/// A role's name as the prompts show it, at most this long.
const NAME_CHARS: usize = 50;

static MENTION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"<@&(\d{17,20})>").unwrap());

/// A role's name on one plain line (its id when it has none).
pub fn name(role: &Value) -> String {
    let name = text::name(role["name"].as_str().unwrap_or_default(), NAME_CHARS);
    if name.is_empty() {
        role["id"].as_str().unwrap_or("unknown").to_string()
    } else {
        name
    }
}

fn position(role: &Value) -> i64 {
    role["position"].as_i64().unwrap_or(0)
}

fn id_number(role: &Value) -> u64 {
    role["id"]
        .as_str()
        .and_then(|id| id.parse().ok())
        .unwrap_or(u64::MAX)
}

/// The roles, highest first (by position; the older role first where two share one).
pub fn highest_first(roles: &[Value]) -> Vec<&Value> {
    let mut sorted: Vec<&Value> = roles.iter().collect();
    sorted.sort_by(|a, b| {
        position(b)
            .cmp(&position(a))
            .then_with(|| id_number(a).cmp(&id_number(b)))
    });

    sorted
}

/// The role ids a message's text mentions (`<@&id>`), each once, in order. A message pings
/// exactly these (Discord's `mention_roles`), and a queued message keeps only its text.
pub fn mentioned(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();

    MENTION
        .captures_iter(content)
        .map(|captures| captures[1].to_string())
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

/// The server's roles by id, for naming them.
#[derive(Debug, Clone, Default)]
pub struct Names {
    /// id => (name, position)
    by_id: HashMap<String, (String, i64)>,
}

impl Names {
    pub fn new(roles: &[Value]) -> Self {
        Self {
            by_id: roles
                .iter()
                .filter_map(|role| {
                    let id = role["id"].as_str()?;
                    Some((id.to_string(), (name(role), position(role))))
                })
                .collect(),
        }
    }

    /// The server's roles from the cache (kept a minute; one Discord call when it is empty).
    /// None when they cannot be read: the roles are then named by their ids.
    pub async fn load(rest: &Rest, guild_id: &str) -> Self {
        match rest.roles(guild_id, false).await {
            Ok(roles) => Self::new(&roles),
            Err(err) => {
                tracing::debug!(
                    server = %guild_id,
                    "rux: the server's roles could not be read: {err}"
                );
                Self::default()
            }
        }
    }

    /// A role's name, when it is known.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.by_id.get(id).map(|(name, _)| name.as_str())
    }

    /// A role's name, else its id (the roles could not be read, or it is new).
    pub fn label(&self, id: &str) -> String {
        self.get(id).unwrap_or(id).to_string()
    }

    /// How a ping of the role reads: `@Name`, else `@&id` (as [`text::untrusted`] writes a
    /// mention it cannot name).
    pub fn ping(&self, id: &str) -> String {
        match self.get(id) {
            Some(name) => format!("@{name}"),
            None => format!("@&{id}"),
        }
    }

    /// These role ids, each once, highest first; the ones not known after the others, in their
    /// order.
    pub fn sorted<'a>(&self, ids: &'a [String]) -> Vec<&'a str> {
        let mut seen = HashSet::new();
        let mut known: Vec<(&str, i64)> = Vec::new();
        let mut unknown: Vec<&str> = Vec::new();
        for id in ids.iter().map(String::as_str) {
            if !seen.insert(id) {
                continue;
            }
            match self.by_id.get(id) {
                Some((_, position)) => known.push((id, *position)),
                None => unknown.push(id),
            }
        }
        // stable: roles of the same position keep the member's order
        known.sort_by_key(|(_, position)| std::cmp::Reverse(*position));

        known.into_iter().map(|(id, _)| id).chain(unknown).collect()
    }

    /// `<@&id>` in `text` made `@Name` for the roles that are known; the others stay as they are.
    pub fn name_mentions(&self, text: &str) -> String {
        MENTION
            .replace_all(text, |captures: &regex::Captures| {
                match self.get(&captures[1]) {
                    Some(name) => format!("@{name}"),
                    None => captures[0].to_string(),
                }
            })
            .into_owned()
    }
}

/// `items` joined with ", ", at most `max` of them, then "+N more".
pub fn joined(items: Vec<String>, max: usize) -> String {
    let more = items.len().saturating_sub(max);
    let mut items: Vec<String> = items.into_iter().take(max).collect();
    if more > 0 {
        items.push(format!("+{more} more"));
    }

    items.join(", ")
}

/// The server's roles for an order's prompt, highest first and @everyone left out: "- Name
/// <@&id>" with what each is (`managed`: Discord or a bot gives it; `rewards`: it comes with
/// rewards; `moderator`: a rux moderator role), at most `max`, then "… and N more: use
/// server_overview.".
pub fn listing(
    roles: &[Value],
    guild_id: &str,
    moderator_role_ids: &[String],
    boost_two_role_id: &str,
    max: usize,
) -> Vec<String> {
    let roles: Vec<&Value> = highest_first(roles)
        .into_iter()
        .filter(|role| role["id"].as_str().is_some_and(|id| id != guild_id))
        .collect();

    let mut lines: Vec<String> = roles
        .iter()
        .take(max)
        .map(|role| {
            let id = role["id"].as_str().unwrap_or_default();
            let managed = role["managed"].as_bool() == Some(true);
            let mut marks: Vec<&str> = Vec::new();
            if managed {
                marks.push("managed");
            }
            // every managed role counts as one with rewards: said once
            if !managed && access::is_reward_role(role, boost_two_role_id) {
                marks.push("rewards");
            }
            if moderator_role_ids.iter().any(|moderator| moderator == id) {
                marks.push("moderator");
            }

            if marks.is_empty() {
                format!("- {} <@&{id}>", name(role))
            } else {
                format!("- {} <@&{id}> ({})", name(role), marks.join(", "))
            }
        })
        .collect();
    if roles.len() > max {
        lines.push(format!(
            "… and {} more: use server_overview.",
            roles.len() - max
        ));
    }

    lines
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub const GUILD: &str = "100000000000000001";
    pub const VERIFIED: &str = "100000000000000008";
    pub const STAFF: &str = "100000000000000009";
    pub const BOOSTER: &str = "100000000000000010";
    pub const MEMBER: &str = "100000000000000011";

    /// @everyone, Staff (highest), the booster role, Verified, Member (lowest).
    pub fn roles() -> Vec<Value> {
        vec![
            json!({"id": GUILD, "name": "@everyone", "position": 0}),
            json!({"id": MEMBER, "name": "Member", "position": 1}),
            json!({"id": VERIFIED, "name": "Verified", "position": 2}),
            json!({"id": BOOSTER, "name": "Server Booster", "position": 3, "managed": true, "tags": {"premium_subscriber": null}}),
            json!({"id": STAFF, "name": "Staff\n<b>", "position": 4}),
        ]
    }

    #[test]
    fn roles_are_named_highest_first() {
        let names = Names::new(&roles());
        let ids: Vec<String> = [
            MEMBER,
            "100000000000000099",
            STAFF,
            VERIFIED,
            MEMBER,
            "100000000000000098",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();

        assert_eq!(
            names.sorted(&ids),
            vec![
                STAFF,
                VERIFIED,
                MEMBER,
                "100000000000000099",
                "100000000000000098"
            ]
        );
        // names are one plain line
        assert_eq!(names.label(STAFF), "Staff ‹b›");
        assert_eq!(names.label("100000000000000099"), "100000000000000099");
        assert_eq!(names.ping(VERIFIED), "@Verified");
        assert_eq!(names.ping("100000000000000099"), "@&100000000000000099");
    }

    #[test]
    fn without_the_roles_ids_stand_in() {
        let names = Names::default();
        let ids = vec![MEMBER.to_string(), VERIFIED.to_string()];

        assert_eq!(names.sorted(&ids), vec![MEMBER, VERIFIED]);
        assert_eq!(names.label(VERIFIED), VERIFIED);
        assert_eq!(
            names.name_mentions(&format!("hi <@&{VERIFIED}>")),
            format!("hi <@&{VERIFIED}>")
        );
    }

    #[test]
    fn mentions_are_found_and_named() {
        let text = format!("<@&{STAFF}> help <@{MEMBER}> <@&{STAFF}> <@&{VERIFIED}> <@&12>");
        assert_eq!(mentioned(&text), vec![STAFF, VERIFIED]);
        assert!(mentioned("no pings @Staff").is_empty());

        let names = Names::new(&roles());
        assert_eq!(
            names.name_mentions(&format!("<@&{VERIFIED}> or <@&100000000000000099>")),
            "@Verified or <@&100000000000000099>"
        );
    }

    #[test]
    fn lists_are_capped() {
        let items: Vec<String> = ["a", "b", "c"].into_iter().map(str::to_string).collect();
        assert_eq!(joined(items.clone(), 5), "a, b, c");
        assert_eq!(joined(items, 2), "a, b, +1 more");
        assert_eq!(joined(Vec::new(), 2), "");
    }

    #[test]
    fn the_listing_says_what_each_role_is() {
        let moderators = vec![STAFF.to_string()];
        let lines = listing(&roles(), GUILD, &moderators, VERIFIED, 75);

        assert_eq!(
            lines,
            vec![
                format!("- Staff ‹b› <@&{STAFF}> (moderator)"),
                format!("- Server Booster <@&{BOOSTER}> (managed)"),
                // the Discord section's two-boost role comes with rewards
                format!("- Verified <@&{VERIFIED}> (rewards)"),
                format!("- Member <@&{MEMBER}>"),
            ]
        );

        let capped = listing(&roles(), GUILD, &[], "", 2);
        assert_eq!(capped.len(), 3);
        assert_eq!(
            capped[1],
            format!("- Server Booster <@&{BOOSTER}> (managed)")
        );
        assert_eq!(capped[2], "… and 2 more: use server_overview.");

        // equal positions: the older role first
        let tied = vec![
            json!({"id": "100000000000000021", "name": "B", "position": 1}),
            json!({"id": "100000000000000020", "name": "A", "position": 1}),
        ];
        assert_eq!(
            listing(&tied, GUILD, &[], "", 75),
            vec![
                "- A <@&100000000000000020>".to_string(),
                "- B <@&100000000000000021>".to_string()
            ]
        );
        assert!(listing(&[], GUILD, &[], "", 75).is_empty());
    }
}
