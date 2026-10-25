//! plugged.host: what a subdomain's DNS records are: the name rules, the game a server's egg runs
//! and the SRV records it gets, and the comment that marks a record as the panel's.

/// Names nobody may take, whatever the settings say; `*` matches anything.
pub const BUILT_IN_BLOCKED: &[&str] = &[
    "www",
    "panel",
    "admin",
    "api",
    "auth",
    "login",
    "billing",
    "client",
    "node*",
    "dash*",
    "status",
    "cp",
    "webmail",
    "mail",
    "email",
    "smtp",
    "pop",
    "imap",
    "ns*",
    "dns*",
    "vpn",
    "ssl",
    "secure",
    "autodiscover",
    "ftp",
    "ssh",
    "sftp",
    "pma",
    "mysql",
    "database",
    "proxy",
    "gateway",
    "cloud*",
    "dev",
    "stage",
    "staging",
    "test",
    "prod",
    "internal",
    "support",
    "help",
    "shop",
    "store",
    "docs",
    "legal",
    "terms",
    "privacy",
    "cdn",
    "assets",
    "static",
    "app",
    "bot",
    "discord",
    "fivem",
    "minecraft",
    "mc",
];

/// A DNS label players may pick: 2 to 63 lowercase letters, digits and hyphens, not starting or
/// ending with a hyphen.
pub fn valid_name(name: &str) -> bool {
    (2..=63).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Whether the name is one of the built-in or the configured (`extra`, comma separated) blocked
/// names.
pub fn blocked(name: &str, extra: &str) -> bool {
    BUILT_IN_BLOCKED
        .iter()
        .copied()
        .chain(extra.split(',').map(str::trim).filter(|p| !p.is_empty()))
        .any(|pattern| wildcard(&pattern.to_lowercase(), name))
}

/// `*` matches any run of characters, everything else itself.
fn wildcard(pattern: &str, value: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = value.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };

    for part in middle {
        match rest.find(part) {
            Some(index) => rest = &rest[index + part.len()..],
            None => return false,
        }
    }

    rest.len() >= last.len() && rest.ends_with(last)
}

/// The game a server's egg runs, for its records: an SRV service, or `Plain` for an address
/// record alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Minecraft,
    Fivem,
    Mumble,
    Factorio,
    Rust,
    Scpsl,
    Teamspeak,
    Plain,
}

/// One SRV record of a service: the key its id is stored under, the `_service._proto` prefix and
/// the weight.
pub struct SrvSpec {
    pub key: &'static str,
    pub prefix: &'static str,
    pub weight: u16,
}

impl Service {
    /// The egg's game: [`of_features`](Self::of_features), with Minecraft being a Java Edition
    /// game server (`crate::minecraft::is_minecraft`), not a proxy such as Velocity.
    pub fn of_egg(egg: &crate::models::nest_egg::NestEgg) -> Option<Self> {
        Self::of_features(&egg.features, crate::minecraft::is_minecraft(egg))
    }

    /// The egg's game from its features: `srv-<service>` names one (`srv-minecraft`, `srv-fivem`,
    /// `srv-teamspeak`, ...), `subdomain` asks for an address record alone, and otherwise
    /// FiveM / RedM (`cfx`) and a Minecraft server (`minecraft`) are recognised. `None`: no
    /// subdomains.
    pub fn of_features(features: &[compact_str::CompactString], minecraft: bool) -> Option<Self> {
        let has = |name: &str| features.iter().any(|feature| feature == name);

        for (feature, service) in [
            ("srv-minecraft", Self::Minecraft),
            ("srv-fivem", Self::Fivem),
            ("srv-cfx", Self::Fivem),
            ("srv-mumble", Self::Mumble),
            ("srv-factorio", Self::Factorio),
            ("srv-rust", Self::Rust),
            ("srv-scpsl", Self::Scpsl),
            ("srv-teamspeak", Self::Teamspeak),
            ("subdomain", Self::Plain),
            (crate::fivem::FEATURE, Self::Fivem),
        ] {
            if has(feature) {
                return Some(service);
            }
        }

        minecraft.then_some(Self::Minecraft)
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Minecraft => "minecraft",
            Self::Fivem => "fivem",
            Self::Mumble => "mumble",
            Self::Factorio => "factorio",
            Self::Rust => "rust",
            Self::Scpsl => "scpsl",
            Self::Teamspeak => "teamspeak",
            Self::Plain => "plain",
        }
    }

    pub fn srv(self) -> &'static [SrvSpec] {
        const fn one(prefix: &'static str) -> [SrvSpec; 1] {
            [SrvSpec {
                key: "srv",
                prefix,
                weight: 0,
            }]
        }
        const MINECRAFT: [SrvSpec; 1] = one("_minecraft._tcp");
        const FIVEM: [SrvSpec; 2] = [
            SrvSpec {
                key: "srv_udp",
                prefix: "_cfx._udp",
                weight: 5,
            },
            SrvSpec {
                key: "srv_tcp",
                prefix: "_cfx._tcp",
                weight: 5,
            },
        ];
        const MUMBLE: [SrvSpec; 1] = one("_mumble._tcp");
        const FACTORIO: [SrvSpec; 1] = one("_factorio._udp");
        const RUST: [SrvSpec; 1] = one("_rust._udp");
        const SCPSL: [SrvSpec; 1] = one("_scpsl._udp");
        const TEAMSPEAK: [SrvSpec; 1] = one("_ts3._udp");

        match self {
            Self::Minecraft => &MINECRAFT,
            Self::Fivem => &FIVEM,
            Self::Mumble => &MUMBLE,
            Self::Factorio => &FACTORIO,
            Self::Rust => &RUST,
            Self::Scpsl => &SCPSL,
            Self::Teamspeak => &TEAMSPEAK,
            Self::Plain => &[],
        }
    }

    /// What players type to join: Minecraft follows the SRV record, FiveM's console takes
    /// `connect`, everything else needs the port.
    pub fn connect(self, fqdn: &str, port: Option<i32>) -> String {
        match (self, port) {
            (Self::Minecraft, _) => fqdn.to_string(),
            (Self::Fivem, _) => format!("connect {fqdn}"),
            (_, Some(port)) if port != 80 && port != 443 => format!("{fqdn}:{port}"),
            _ => fqdn.to_string(),
        }
    }
}

/// The comment on every record the panel makes, "Created by <panel name> Subdomains (<detail>)",
/// within Cloudflare's 100 characters.
pub fn comment(app_name: &str, detail: &str) -> String {
    let name: String = app_name
        .chars()
        .map(|c| {
            if c.is_control() || c == '(' || c == ')' {
                ' '
            } else {
                c
            }
        })
        .take(32)
        .collect();
    let name = name.trim();

    let comment = if name.is_empty() {
        format!("Created by Subdomains ({detail})")
    } else {
        format!("Created by {name} Subdomains ({detail})")
    };

    comment.chars().take(100).collect()
}

/// The detail of a server's record comments.
pub fn server_detail(server_uuid: uuid::Uuid) -> String {
    format!("Server {}", &server_uuid.simple().to_string()[..8])
}

/// Whether a record comment marks the record as the panel's for this server: a record someone
/// else made in a shared zone is never touched.
pub fn own_comment(comment: Option<&str>, app_name: &str, server_uuid: uuid::Uuid) -> bool {
    let Some(comment) = comment else {
        return false;
    };
    let prefix = comment_prefix(app_name);

    comment.starts_with(&prefix) && comment.ends_with(&format!("{})", server_detail(server_uuid)))
}

fn comment_prefix(app_name: &str) -> String {
    let empty = comment(app_name, "");

    empty.trim_end_matches(')').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_dns_label_rules() {
        assert!(valid_name("play"));
        assert!(valid_name("my-server-2"));
        assert!(!valid_name("a"));
        assert!(!valid_name("-play"));
        assert!(!valid_name("play-"));
        assert!(!valid_name("Play"));
        assert!(!valid_name("pl_ay"));
        assert!(!valid_name(&"a".repeat(64)));
    }

    #[test]
    fn blocked_names_match_with_wildcards() {
        assert!(blocked("www", ""));
        assert!(blocked("node12", ""));
        assert!(blocked("dashboard", ""));
        assert!(!blocked("play", ""));
        assert!(blocked("test-1", "test*"));
        assert!(blocked("my-shop", "*shop"));
        assert!(blocked("a-b-c", "a*c"));
        assert!(!blocked("ab", "a*b*c"));
        assert!(blocked("best", "Best, other"));
    }

    #[test]
    fn games_come_from_the_egg_features() {
        let features = |names: &[&str]| -> Vec<compact_str::CompactString> {
            names.iter().map(|name| (*name).into()).collect()
        };

        assert_eq!(
            Service::of_features(&features(&["eula", "java_version"]), true),
            Some(Service::Minecraft)
        );
        // Velocity: `eula`, but a proxy
        assert_eq!(
            Service::of_features(&features(&["eula", "java_version"]), false),
            None
        );
        assert_eq!(
            Service::of_features(&features(&["cfx"]), false),
            Some(Service::Fivem)
        );
        assert_eq!(
            Service::of_features(&features(&["eula", "srv-teamspeak"]), true),
            Some(Service::Teamspeak)
        );
        assert_eq!(
            Service::of_features(&features(&["eula", "srv-minecraft"]), false),
            Some(Service::Minecraft)
        );
        assert_eq!(
            Service::of_features(&features(&["subdomain"]), false),
            Some(Service::Plain)
        );
        assert_eq!(Service::of_features(&features(&["pid_limit"]), false), None);
        assert_eq!(Service::Fivem.srv().len(), 2);
        assert!(Service::Plain.srv().is_empty());
    }

    #[test]
    fn connect_lines() {
        assert_eq!(
            Service::Minecraft.connect("a.example.com", Some(25565)),
            "a.example.com"
        );
        assert_eq!(
            Service::Fivem.connect("a.example.com", Some(30120)),
            "connect a.example.com"
        );
        assert_eq!(
            Service::Plain.connect("a.example.com", Some(7777)),
            "a.example.com:7777"
        );
        assert_eq!(
            Service::Plain.connect("a.example.com", Some(443)),
            "a.example.com"
        );
    }

    #[test]
    fn comments_are_recognised_only_for_the_same_panel_and_server() {
        let server = uuid::Uuid::from_u128(0xabcdef12_0000_0000_0000_000000000001);
        let other = uuid::Uuid::from_u128(0x12345678_0000_0000_0000_000000000001);
        let made = comment("My (Panel)", &server_detail(server));

        assert_eq!(made, "Created by My  Panel Subdomains (Server abcdef12)");
        assert!(own_comment(Some(&made), "My (Panel)", server));
        assert!(!own_comment(Some(&made), "My (Panel)", other));
        assert!(!own_comment(Some(&made), "Another", server));
        assert!(!own_comment(None, "My (Panel)", server));
        assert!(comment(&"x".repeat(200), &"y".repeat(200)).chars().count() <= 100);
    }
}
