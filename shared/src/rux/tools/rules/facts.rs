//! plugged.host: facts about a server's host the checks give the AI, so it does not take the
//! host's own links for scams or adverts, nor a question about a location that exists for a
//! request for a new one: its own sites (the server's trusted domains, the panel and status
//! page, the account portal, the Discord invite) and, for the panel's own community, its
//! locations.

use crate::rux::config::{Config, Guild};

/// How long the locations line is kept.
const LOCATIONS_SECONDS: u64 = 600;
const LOCATIONS_KEY: &str = "rux::facts::locations";

/// The host name of a URL or bare domain, lowercase: "https://Panel.example.com/x" →
/// "panel.example.com"; "*.example.com" → "example.com".
pub fn host(value: &str) -> String {
    let value = value.trim().to_lowercase();
    let value = match value.split_once("://") {
        Some((scheme, rest))
            if !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) =>
        {
            rest.to_string()
        }
        _ => value,
    };
    let value = value.strip_prefix("*.").unwrap_or(&value);
    let end = value.find(['/', ':', '?', '#']).unwrap_or(value.len());

    value[..end].trim_end_matches('.').to_string()
}

/// "a", "a and b", "a, b, and c".
pub fn listing(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The host's own sites: each trusted domain "and its subdomains" (naming the panel's and status
/// page's hosts under it), other hosts as they are, the account portal and the Discord invite.
pub fn own_sites(
    app: &str,
    trusted_domains: &[String],
    hosts: &[String],
    portal_host: Option<&str>,
    invite: &str,
) -> Vec<String> {
    let mut domains: Vec<String> = Vec::new();
    for domain in trusted_domains {
        let domain = host(domain);
        if !domain.is_empty() && !domains.contains(&domain) {
            domains.push(domain);
        }
    }

    let mut others: Vec<String> = Vec::new();
    for value in hosts {
        let value = host(value);
        if !value.is_empty() && !others.contains(&value) && !domains.contains(&value) {
            others.push(value);
        }
    }

    let mut sites = Vec::new();
    for domain in &domains {
        let suffix = format!(".{domain}");
        let (under, rest): (Vec<String>, Vec<String>) = others
            .into_iter()
            .partition(|other| other.ends_with(&suffix));
        others = rest;
        sites.push(if under.is_empty() {
            format!("{domain} and its subdomains")
        } else {
            format!("{domain} and its subdomains ({}, …)", under.join(", "))
        });
    }
    sites.extend(others);

    if let Some(portal) = portal_host.map(host).filter(|portal| !portal.is_empty()) {
        // "accounts.example.org" → "example.org and accounts.example.org"
        let parent = portal
            .split_once('.')
            .map(|(_, parent)| parent.to_string())
            .filter(|parent| parent.contains('.'));
        let covered = |name: &str| {
            domains
                .iter()
                .any(|domain| name == domain || name.ends_with(&format!(".{domain}")))
        };
        if !covered(&portal) {
            sites.push(match parent {
                Some(parent) if !covered(&parent) => {
                    format!("{parent} and {portal} ({app} accounts, credits and sign-in)")
                }
                _ => format!("{portal} ({app} accounts, credits and sign-in)"),
            });
        }
    }

    let invite = invite.trim().trim_end_matches('/');
    let invite = invite
        .strip_prefix("https://")
        .or_else(|| invite.strip_prefix("http://"))
        .unwrap_or(invite);
    if !invite.is_empty() {
        sites.push(format!("its Discord invite {invite}"));
    }

    sites
}

/// The line about locations: "France (FR, Plus; nodes fr1, fr3)".
pub fn location_line(
    name: &str,
    flag: Option<&str>,
    tier: Option<&str>,
    nodes: &[String],
) -> String {
    let label = [flag.map(str::to_uppercase), tier.map(str::to_string)]
        .into_iter()
        .flatten()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let nodes = format!(
        "{} {}",
        if nodes.len() > 1 { "nodes" } else { "node" },
        nodes.join(", ")
    );

    if label.is_empty() {
        format!("{name} ({nodes})")
    } else {
        format!("{name} ({label}; {nodes})")
    }
}

/// The panel's public locations that have game nodes, as [`location_line`]s (kept 10 minutes).
async fn locations(state: &crate::State) -> Vec<String> {
    if let Ok(Some(lines)) = state.cache.get::<Vec<String>>(LOCATIONS_KEY).await {
        return lines;
    }

    let rows: Vec<(String, Option<String>, Option<String>, Vec<String>)> = match sqlx::query_as(
        "SELECT l.name::text, l.flag::text, l.tier::text, array_agg(n.name::text ORDER BY n.name)
        FROM locations l
        JOIN nodes n ON n.location_uuid = l.uuid AND n.role = 'game'
        WHERE l.public
        GROUP BY l.uuid, l.name, l.flag, l.tier, l.sort
        ORDER BY l.sort, l.name",
    )
    .fetch_all(state.database.read())
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!("rux: could not read the locations: {err}");
            return Vec::new();
        }
    };

    let lines: Vec<String> = rows
        .iter()
        .map(|(name, flag, tier, nodes)| {
            location_line(name, flag.as_deref(), tier.as_deref(), nodes)
        })
        .collect();
    let _ = state
        .cache
        .set(LOCATIONS_KEY, LOCATIONS_SECONDS, &lines)
        .await;

    lines
}

/// The facts about a server's host (the main server's when none is given), as lines for a
/// check's prompt: "Facts about Example:", then a line per kind of fact.
pub async fn lines(state: &crate::State, config: &Config, guild: Option<&Guild>) -> Vec<String> {
    let guild = guild.or_else(|| config.primary());
    let app = guild.map_or_else(|| config.app_name.clone(), |guild| guild.host_name.clone());
    let mut lines = vec![format!("Facts about {app}:")];

    let (hosts, portal, invite) = match state.settings.get().await {
        Ok(settings) => (
            [
                Some(settings.app.url.to_string()),
                settings.status.page_url.as_ref().map(|url| url.to_string()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
            settings
                .noprofit
                .config()
                .is_some()
                .then(|| settings.noprofit.accounts_url.to_string()),
            settings.discord.invite_url.to_string(),
        ),
        Err(err) => {
            tracing::warn!("rux: could not read the settings for the facts: {err:#}");
            (Vec::new(), None, String::new())
        }
    };
    let trusted: Vec<String> = match guild {
        Some(guild) => guild.trusted_domains.clone(),
        None => config
            .settings
            .trusted_domains
            .iter()
            .map(|domain| domain.to_string())
            .collect(),
    };

    let sites = own_sites(&app, &trusted, &hosts, portal.as_deref(), &invite);
    if !sites.is_empty() {
        lines.push(format!(
            "- Its own sites and links, never a scam or an advert: {}.",
            listing(&sites)
        ));
    }

    if guild.is_none_or(|guild| guild.locations) {
        let locations = locations(state).await;
        if !locations.is_empty() {
            lines.push(format!(
                "- The server locations it already has: {}. Asking about one of these, which to pick, how to get or move to one, or for one to be opened, enabled or brought back is NOT asking for a new location.",
                locations.join("; ")
            ));
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_read() {
        assert_eq!(host("https://Panel.example.com/x?y"), "panel.example.com");
        assert_eq!(host("*.example.com"), "example.com");
        assert_eq!(host("example.com:8080"), "example.com");
        assert_eq!(host("example.com."), "example.com");
        assert_eq!(host("  "), "");
    }

    #[test]
    fn lists_read_as_english() {
        let items = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(listing(&items(&[])), "");
        assert_eq!(listing(&items(&["a"])), "a");
        assert_eq!(listing(&items(&["a", "b"])), "a and b");
        assert_eq!(listing(&items(&["a", "b", "c"])), "a, b, and c");
    }

    #[test]
    fn own_sites_group_subdomains() {
        let sites = own_sites(
            "Example",
            &["example.com".into(), "https://Example.com/".into()],
            &[
                "https://panel.example.com".into(),
                "https://status.example.com/".into(),
                "https://other.example.net".into(),
            ],
            Some("https://accounts.example.org"),
            "https://discord.gg/example/",
        );
        assert_eq!(
            sites,
            vec![
                "example.com and its subdomains (panel.example.com, status.example.com, …)",
                "other.example.net",
                "example.org and accounts.example.org (Example accounts, credits and sign-in)",
                "its Discord invite discord.gg/example",
            ]
        );

        // a portal under a trusted domain is already named
        let sites = own_sites(
            "Example",
            &["example.com".into()],
            &[],
            Some("https://accounts.example.com"),
            "",
        );
        assert_eq!(sites, vec!["example.com and its subdomains"]);
    }

    #[test]
    fn locations_are_described() {
        assert_eq!(
            location_line(
                "France",
                Some("fr"),
                Some("Plus"),
                &["fr1".into(), "fr3".into()]
            ),
            "France (FR, Plus; nodes fr1, fr3)"
        );
        assert_eq!(
            location_line("Lab", None, None, &["lab1".into()]),
            "Lab (node lab1)"
        );
    }
}
