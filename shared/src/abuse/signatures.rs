//! plugged.host: what abuse looks like: tool names, text signatures and the hiding tricks, shared
//! by every check (files, startup, console, processes).
//!
//! Severity follows the Terms. Mining, malware/backdoors/botnets, DDoS tools, port scanners and
//! brute forcers are critical, so is a VPN/proxy core. Tunnels, Tor and torrent clients are
//! review-only: the Terms do not ban them outright, and some people use them legitimately.
//!
//! Names are matched against one file/directory/process name, lowercased. An "any" name is
//! distinctive enough to count wherever it appears (`xmrig-6.26.0/` is a folder). An "exec" name
//! is an ordinary word as well (xray is also an anti-xray Minecraft plugin, hydra and medusa are
//! mods), so it only counts for a Linux executable or a running process named exactly that,
//! optionally with a `-linux-amd64` style suffix.
//!
//! The patterns run on bytes with Unicode off (`regex::bytes`, linear time on whatever a server
//! holds). The regex crate has no lookaround, so "not preceded by a letter or digit" is written
//! as `(?:^|[^a-z0-9])` and "not followed by a letter" as `(?:[^a-z]|$)`; where the matched text
//! is shown, the part that counts is the `m` group.

use super::Severity::{self, Critical, Review};
use regex::bytes::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use std::sync::LazyLock;

/// Categories, by key, with the label admins and owners read.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("miner", "Crypto miner"),
    ("proxy", "VPN/proxy server"),
    ("ddos", "DDoS tool"),
    ("scanner", "Port scanner or brute forcer"),
    ("malware", "Malware or backdoor"),
    ("spam", "Bulk mail sending"),
    ("obfuscation", "Obfuscated code"),
    ("tunnel", "Tunnel"),
    ("torrent", "Torrent client"),
    ("evasion", "Hidden process"),
    ("executable", "Unrecognised executable"),
    ("vps", "Hidden Linux system"),
    ("resources", "Sustained full CPU"),
];

pub fn category_label(category: &str) -> String {
    CATEGORIES
        .iter()
        .find(|(key, _)| *key == category)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| {
            let mut chars = category.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        })
}

type Signature = (&'static str, &'static str, Severity, &'static str);

const ANY_NAMES: &[Signature] = &[
    ("xmrig", "miner", Critical, "XMRig"),
    ("(?:^|[^a-z0-9])xmr-?stak", "miner", Critical, "XMR-Stak"),
    ("cpuminer", "miner", Critical, "cpuminer"),
    (
        "(?:^|[^a-z0-9])minerd(?:[^a-z]|$)",
        "miner",
        Critical,
        "minerd",
    ),
    (
        "(?:nano|lol|srb|nb|teamred|phoenix|eth|bz|hell|onezero|nheq)miner",
        "miner",
        Critical,
        "GPU/CPU miner",
    ),
    (
        "(?:^|[^a-z0-9])(?:bfg|cg|cc)miner",
        "miner",
        Critical,
        "GPU/CPU miner",
    ),
    (
        "cryptodredge|wildrig|xlarig|minergate|sugarmaker",
        "miner",
        Critical,
        "GPU/CPU miner",
    ),
    (
        "(?:^|[^a-z0-9])qli-?(?:client|runner)|rqiner",
        "miner",
        Critical,
        "Qubic miner",
    ),
    ("xmr-?node-?proxy", "miner", Critical, "mining proxy"),
    ("sing-?box", "proxy", Critical, "sing-box"),
    ("xray-?(?:core|linux)", "proxy", Critical, "Xray"),
    ("v2ray-?(?:core|linux)|v2fly", "proxy", Critical, "V2Ray"),
    (
        "(?:^|[^a-z0-9])hysteria2?(?:[^a-z]|$)",
        "proxy",
        Critical,
        "Hysteria",
    ),
    (
        "naiveproxy|trojan-go|shadowsocks|shadow-?tls|juicity|outline-ss-server|tuic-?server",
        "proxy",
        Critical,
        "VPN/proxy core",
    ),
    (
        "(?:^|[^a-z0-9])(?:3proxy|microsocks)",
        "proxy",
        Critical,
        "open proxy",
    ),
    (
        "nezha-?agent|komari-?agent",
        "proxy",
        Critical,
        "Nezha proxy kit",
    ),
    (
        "(?:^|[^a-z0-9])(?:3x|x|s|h)-ui(?:[^a-z]|$)",
        "proxy",
        Critical,
        "x-ui proxy panel",
    ),
    (
        "(?:^|[^a-z0-9])wsallin(?:[^a-z]|$)",
        "proxy",
        Critical,
        "wsallin proxy",
    ),
    ("mhddos", "ddos", Critical, "MHDDoS"),
    ("(?:^|[^a-z0-9])hping3", "ddos", Critical, "hping3"),
    ("slowloris|ufonet|torshammer", "ddos", Critical, "DDoS tool"),
    (
        "(?:^|[^a-z0-9])(?:udp|syn|tcp|ack|http)-?flood(?:er)?(?:[^a-z]|$)",
        "ddos",
        Critical,
        "flooder",
    ),
    (
        "(?:^|[^a-z0-9])(?:masscan|zgrab2?|naabu|rustscan)(?:[^a-z]|$)",
        "scanner",
        Critical,
        "port scanner",
    ),
    (
        "(?:^|[^a-z0-9])(?:ncrack|patator|brutespray|kerbrute|sqlmap)(?:[^a-z]|$)",
        "scanner",
        Critical,
        "brute forcer",
    ),
    (
        "(?:^|[^a-z0-9])ssh-?brute",
        "scanner",
        Critical,
        "SSH brute forcer",
    ),
    (
        "gs-?netcat|(?:^|[^a-z0-9])gsocket(?:[^a-z]|$)",
        "malware",
        Critical,
        "gsocket backdoor",
    ),
    (
        "kdevtmpfsi|(?:^|[^a-z0-9])kinsing(?:[^a-z]|$)",
        "malware",
        Critical,
        "Kinsing malware",
    ),
    (r"^\.configrc\d*$", "malware", Critical, "Outlaw botnet"),
    ("ngrok", "tunnel", Review, "ngrok"),
    ("cloudflared", "tunnel", Review, "cloudflared"),
    (
        "(?:^|[^a-z0-9])frp[cs](?:[^a-z]|$)",
        "tunnel",
        Review,
        "frp",
    ),
    ("wstunnel|localtunnel|pinggy", "tunnel", Review, "tunnel"),
    (
        "(?:^|[^a-z0-9])tmate(?:[^a-z]|$)",
        "tunnel",
        Review,
        "tmate",
    ),
    (
        "qbittorrent-?nox|transmission-?daemon|(?:^|[^a-z0-9])rtorrent(?:[^a-z]|$)",
        "torrent",
        Review,
        "torrent client",
    ),
];

const EXEC_NAMES: &[Signature] = &[
    (
        "t-?rex|gminer|rigel|claymore",
        "miner",
        Critical,
        "GPU/CPU miner",
    ),
    ("xray", "proxy", Critical, "Xray"),
    ("v2ray", "proxy", Critical, "V2Ray"),
    (
        "gost|brook|tuic|naive|trojan|mieru|ss-?server|ssserver|ss-?local|sslocal|sockd|danted|tinyproxy",
        "proxy",
        Critical,
        "VPN/proxy core",
    ),
    ("t50|xerxes", "ddos", Critical, "DDoS tool"),
    ("zmap|nmap|nuclei", "scanner", Critical, "port scanner"),
    ("hydra|medusa|crowbar", "scanner", Critical, "brute forcer"),
    (
        "chisel|bore|rathole|zrok|inlets",
        "tunnel",
        Review,
        "tunnel",
    ),
    ("tor", "tunnel", Review, "Tor"),
    ("deluged|aria2c", "torrent", Review, "torrent client"),
];

/// Mining pools, by domain. A miner config, a stats dashboard or a `-o` argument names one.
const POOLS: &str = r"moneroocean\.stream|supportxmr\.com|c3pool\.(?:com|org)|hashvault\.pro|nanopool\.org|2miners\.com|herominers\.com|f2pool\.com|unmineable\.com|nicehash\.com|minexmr\.com|xmrpool\.eu|monerohash\.com|xmrfast\.com|rplant\.xyz|zpool\.ca|zergpool\.com|mining-dutch\.nl|prohashing\.com|kryptex\.network|woolypooly\.com|p2pool\.io|qubic\.li|vipor\.net|luckpool\.net|aikapool\.com|ethermine\.org|flexpool\.io|miningpoolhub\.com|antpool\.com|viabtc\.(?:com|net)|emcd\.io|k1pool\.com|6block\.com|hashcity\.org|fairpool\.xyz|gntl\.uk|minerpool\.pub|baikalmine\.com|solopool\.org|kano\.is|braiins\.com|poolin\.com|moneroworld\.com|xmrig\.com";

/// One text signature: every pattern must match (most have one); the first one's text is shown.
struct ContentSignature {
    patterns: &'static [&'static str],
    category: &'static str,
    severity: Severity,
    label: &'static str,
}

const fn content(
    patterns: &'static [&'static str],
    category: &'static str,
    severity: Severity,
    label: &'static str,
) -> ContentSignature {
    ContentSignature {
        patterns,
        category,
        severity,
        label,
    }
}

const CONTENT: &[ContentSignature] = &[
    content(
        &[r#"(?i)stratum2?\+(?:tcp|ssl|tls|wss?)://[^\s"'<>]+"#],
        "miner",
        Critical,
        "mining pool address",
    ),
    // The pool list is spliced in by `content_patterns`.
    content(&["{POOLS}"], "miner", Critical, "mining pool"),
    content(
        &[r#"(?i)["']donate-level["']\s*:|--donate-level"#],
        "miner",
        Critical,
        "XMRig settings",
    ),
    content(
        &[
            r"(?i)\b(?:coinhive|coin-hive|cryptoloot|crypto-loot|webminepool|jsecoin|coinimp|minero\.cc|webmine\.cz|monerise)\b",
        ],
        "miner",
        Critical,
        "browser miner",
    ),
    content(
        &[
            r#"(?i)(?:^|\s)(?:-a|--algo)[= ]?["']?(?:rx/(?:0|wow|arq|graft|sfx|keva)|randomx|cn(?:-lite|-heavy|-pico)?/[a-z0-9]+|ghostrider|kawpow|argon2/(?:chukwa|wrkz)|astrobwt)\b"#,
        ],
        "miner",
        Critical,
        "mining algorithm",
    ),
    content(
        &[r#"(?i)"algo"\s*:\s*"(?:rx/|cn/|cn-|argon2/|ghostrider|kawpow)"#],
        "miner",
        Critical,
        "mining algorithm",
    ),
    content(
        &["(?:^|[^0-9A-Za-z])(?P<m>[48][0-9AB][1-9A-HJ-NP-Za-km-z]{93})(?:[^0-9A-Za-z]|$)"],
        "miner",
        Review,
        "Monero wallet address",
    ),
    content(&["(?i)sing-?box"], "proxy", Critical, "sing-box"),
    content(
        &[r#""inbounds"\s*:"#, r#""outbounds"\s*:"#],
        "proxy",
        Critical,
        "VPN/proxy core configuration",
    ),
    content(
        &[r#"(?i)\b(?:vless|vmess|trojan|hysteria2|hy2|tuic)://[^\s"'<>]{8,}"#],
        "proxy",
        Critical,
        "VPN share link",
    ),
    content(
        &[r"\bss://[A-Za-z0-9+/=_-]{16,}"],
        "proxy",
        Critical,
        "Shadowsocks share link",
    ),
    content(
        &[r"\b(?:NEZHA_SERVER|NEZHA_KEY|ARGO_DOMAIN|ARGO_AUTH|nezha-agent|komari-agent)\b"],
        "proxy",
        Critical,
        "Argo/Nezha proxy kit",
    ),
    content(
        &[r"(?i)\bwsallin\b"],
        "proxy",
        Critical,
        "wsallin proxy package",
    ),
    content(
        &[r"(?i)\b(?:xray|v2ray)\s+(?:run|-c\b|-config\b)"],
        "proxy",
        Critical,
        "starts Xray/V2Ray",
    ),
    content(
        &["ShadowInfected"],
        "ddos",
        Critical,
        "ShadowInfected botnet client",
    ),
    content(
        &[
            r#"C2_ADDRESS\s*=\s*["']\d+\.\d+\.\d+\.\d+["']"#,
            r"attack_|c2\.connect",
        ],
        "ddos",
        Critical,
        "DDoS botnet client",
    ),
    content(
        &[
            r"(?i)\b(?:attack_udp|attack_syn|attack_hex|attack_ovh|attack_vse|attack_mcpe|attack_fivem|ovh_tcp|ovh_pps|udp_kill)\b",
        ],
        "ddos",
        Critical,
        "DDoS attack methods",
    ),
    content(&[r"(?i)\bMHDDoS\b"], "ddos", Critical, "MHDDoS"),
    content(&[r"\bhping3\s+-"], "ddos", Critical, "hping3 flood"),
    content(&[r"(?i)\bslowloris\b"], "ddos", Critical, "Slowloris"),
    content(
        &[r"\b(?:masscan|zmap|nmap|naabu|rustscan)\s+(?:-|\d)"],
        "scanner",
        Critical,
        "runs a port scanner",
    ),
    content(
        &[r"\b(?:hydra|medusa|ncrack)\s+-[a-zA-Z]"],
        "scanner",
        Critical,
        "runs a password brute forcer",
    ),
    content(
        &[r"\bgs-netcat\b|\bGSOCKET_ARGS\b|gsocket\.io/[xy]"],
        "malware",
        Critical,
        "gsocket backdoor",
    ),
    content(
        &[r"mdrfckr|\.configrc\d*/|dota3\.tar"],
        "malware",
        Critical,
        "Outlaw botnet",
    ),
    content(
        &[
            r"/dev/tcp/\d{1,3}(?:\.\d{1,3}){3}/\d+|\bbash\s+-i\s+>&|\bnc(?:at)?\s+(?:-\w+\s+)*-e\s+/bin/(?:ba)?sh",
        ],
        "malware",
        Review,
        "reverse shell",
    ),
    content(
        &[
            r"(?i)(?:[a-z0-9-]+\.)?(?:ngrok(?:-free)?\.(?:io|app|dev)|trycloudflare\.com|loca\.lt|serveo\.net|pinggy\.(?:io|link)|bore\.pub|localhost\.run)\b",
        ],
        "tunnel",
        Review,
        "public tunnel",
    ),
    content(
        &[
            r"(?i)\bcloudflared\s+(?:tunnel|access)\b|\bngrok\s+(?:http|tcp|tls|start)\b|\bfrpc\s+-c\b",
        ],
        "tunnel",
        Review,
        "starts a tunnel",
    ),
];

/// Mentions XMRig: on its own a review, next to something that downloads or starts it, critical.
const XMRIG_MENTION: &str = r"(?i)\bxmrig\b";
const LAUNCHES: &str = r"(?i)\b(?:wget|curl|chmod\s+\+x|subprocess|child_process|os\.system|os\.popen|spawn|execFile|nohup|tar\s+-?x|unzip)\b";

/// Kernel thread names. A userland process (it has an executable) by one of these names is hiding.
const KERNEL_THREAD: &str = "^(?:kworker|kswapd|ksoftirqd|kthreadd|kdevtmpfs|rcu_|edac|khugepaged|kcompactd|kblockd|kauditd|jbd2|cpuhp|oom_reaper|writeback|kintegrityd|ata_sff|netns|kstrp|ipv6_addrconf|zswap|scsi_|irq/)";

const PACKED_JS: &str =
    r"eval\s*\(\s*function\s*\(\s*p\s*,\s*a\s*,\s*c\s*,\s*k\s*,\s*e\s*,\s*[dr]\s*\)";
const OBFUSCATOR_IO_NAME: &str = r"(?i)\b_0x[a-f0-9]{4,6}\b";
const HIDDEN_PYTHON: &str = r#"\bexec\s*\(\s*(?:base64\.b64decode|zlib\.decompress|marshal\.loads|bytes\.fromhex|__import__\(\s*["'](?:base64|zlib|marshal)["']\s*\))"#;

fn build(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .unicode(false)
        .size_limit(32 << 20)
        .build()
        .unwrap_or_else(|err| panic!("abuse signature {pattern:?} does not compile: {err}"))
}

fn build_set(patterns: impl IntoIterator<Item = String>) -> RegexSet {
    RegexSetBuilder::new(patterns)
        .unicode(false)
        .size_limit(64 << 20)
        .build()
        .expect("abuse signature set compiles")
}

fn content_pattern(pattern: &str) -> String {
    pattern.replace(
        "{POOLS}",
        &format!("(?i)(?P<m>(?:[a-z0-9-]+\\.)*(?:{POOLS}))(?:[^a-z0-9-]|$)"),
    )
}

static ANY_SET: LazyLock<RegexSet> =
    LazyLock::new(|| build_set(ANY_NAMES.iter().map(|(regex, ..)| regex.to_string())));
static EXEC_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    build_set(
        EXEC_NAMES
            .iter()
            .map(|(regex, ..)| format!("^(?:{regex})(?:[-_.][a-z0-9._-]*)?$")),
    )
});
/// The first pattern of every content signature, to find the candidates in one pass.
static CONTENT_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    build_set(
        CONTENT
            .iter()
            .map(|signature| content_pattern(signature.patterns[0])),
    )
});
static CONTENT_REGEXES: LazyLock<Vec<Vec<Regex>>> = LazyLock::new(|| {
    CONTENT
        .iter()
        .map(|signature| {
            signature
                .patterns
                .iter()
                .map(|pattern| build(&content_pattern(pattern)))
                .collect()
        })
        .collect()
});
static XMRIG_MENTION_REGEX: LazyLock<Regex> = LazyLock::new(|| build(XMRIG_MENTION));
static LAUNCHES_REGEX: LazyLock<Regex> = LazyLock::new(|| build(LAUNCHES));
static KERNEL_THREAD_REGEX: LazyLock<Regex> = LazyLock::new(|| build(KERNEL_THREAD));
static PACKED_JS_REGEX: LazyLock<Regex> = LazyLock::new(|| build(PACKED_JS));
static OBFUSCATOR_IO_REGEX: LazyLock<Regex> = LazyLock::new(|| build(OBFUSCATOR_IO_NAME));
static HIDDEN_PYTHON_REGEX: LazyLock<Regex> = LazyLock::new(|| build(HIDDEN_PYTHON));

/// A signature that matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub category: &'static str,
    pub severity: Severity,
    pub label: &'static str,
    /// The text that matched (content signatures only).
    pub text: Option<String>,
}

/// Tools named by one file, directory or process name.
pub fn match_name(name: &str, is_executable: bool) -> Vec<Match> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<Match> = ANY_SET
        .matches(name.as_bytes())
        .iter()
        .map(|index| {
            let (_, category, severity, label) = ANY_NAMES[index];
            Match {
                category,
                severity,
                label,
                text: None,
            }
        })
        .collect();

    if is_executable {
        // `xray-linux-amd64` is both a distinctive name and the plain word: report it once.
        for index in EXEC_SET.matches(name.as_bytes()).iter() {
            let (_, category, severity, label) = EXEC_NAMES[index];
            if !matches
                .iter()
                .any(|found| found.category == category && found.label == label)
            {
                matches.push(Match {
                    category,
                    severity,
                    label,
                    text: None,
                });
            }
        }
    }

    matches
}

fn matched_text(regex: &Regex, haystack: &[u8]) -> Option<String> {
    let captures = regex.captures(haystack)?;
    let found = captures.name("m").or_else(|| captures.get(0))?;
    Some(String::from_utf8_lossy(found.as_bytes()).into_owned())
}

/// Text signatures in a file, a command line or startup values.
pub fn match_content(content: &[u8]) -> Vec<Match> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for index in CONTENT_SET.matches(content).iter() {
        let signature = &CONTENT[index];
        let regexes = &CONTENT_REGEXES[index];

        if !regexes[1..].iter().all(|regex| regex.is_match(content)) {
            continue;
        }
        let Some(text) = matched_text(&regexes[0], content) else {
            continue;
        };

        matches.push(Match {
            category: signature.category,
            severity: signature.severity,
            label: signature.label,
            text: Some(text),
        });
    }

    if let Some(text) = matched_text(&XMRIG_MENTION_REGEX, content) {
        let launches = LAUNCHES_REGEX.is_match(content);
        matches.push(Match {
            category: "miner",
            severity: if launches { Critical } else { Review },
            label: if launches {
                "downloads or starts XMRig"
            } else {
                "mentions XMRig"
            },
            text: Some(text),
        });
    }

    matches
}

pub fn is_kernel_thread_name(name: &str) -> bool {
    KERNEL_THREAD_REGEX.is_match(name.as_bytes())
}

/// Obfuscated JavaScript: the p,a,c,k,e,d packer, or obfuscator.io's `_0x` identifiers. Minified
/// libraries have the odd `_0x` name too, so it takes fifteen distinct ones, which obfuscator.io
/// output passes by hundreds.
pub fn obfuscated_javascript(content: &[u8]) -> Option<&'static str> {
    if PACKED_JS_REGEX.is_match(content) {
        return Some("packed JavaScript");
    }

    let mut names = std::collections::HashSet::new();
    for found in OBFUSCATOR_IO_REGEX.find_iter(content) {
        names.insert(found.as_bytes().to_ascii_lowercase());
        if names.len() >= 15 {
            return Some("obfuscated JavaScript");
        }
    }

    None
}

/// Python that decodes and runs a hidden payload. Review: some paid bots ship this way.
pub fn obfuscated_python(content: &[u8]) -> Option<&'static str> {
    HIDDEN_PYTHON_REGEX
        .is_match(content)
        .then_some("Python that runs a hidden payload")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(matches: &[Match]) -> Vec<&'static str> {
        matches.iter().map(|m| m.label).collect()
    }

    #[test]
    fn every_signature_compiles() {
        LazyLock::force(&ANY_SET);
        LazyLock::force(&EXEC_SET);
        LazyLock::force(&CONTENT_SET);
        LazyLock::force(&CONTENT_REGEXES);
        LazyLock::force(&KERNEL_THREAD_REGEX);
        assert_eq!(CONTENT_SET.len(), CONTENT.len());
    }

    #[test]
    fn names_anywhere() {
        assert_eq!(labels(&match_name("xmrig-6.26.0", false)), ["XMRig"]);
        assert_eq!(labels(&match_name("Sing-Box", false)), ["sing-box"]);
        assert_eq!(labels(&match_name("minerd", false)), ["minerd"]);
        assert!(match_name("minerdashboard", false).is_empty());
        assert_eq!(labels(&match_name("udp-flooder", false)), ["flooder"]);
        assert!(match_name("udpfloodx", false).is_empty());
        assert_eq!(labels(&match_name(".configrc5", false)), ["Outlaw botnet"]);
        assert!(match_name("x.configrc5", false).is_empty());
        assert_eq!(labels(&match_name("3x-ui", false)), ["x-ui proxy panel"]);
        assert!(match_name("box-ui", false).is_empty());
    }

    #[test]
    fn exec_names_only_for_executables() {
        assert!(match_name("xray", false).is_empty());
        assert_eq!(labels(&match_name("xray", true)), ["Xray"]);
        assert_eq!(labels(&match_name("xray-linux-amd64", true)), ["Xray"]);
        // `java -jar xray-detector.jar`: an argument is matched as a file name, not a program.
        assert!(match_name("xray-detector.jar", false).is_empty());
        assert!(match_name("antixray", true).is_empty());
        assert_eq!(labels(&match_name("tor", true)), ["Tor"]);
        assert!(match_name("torch", true).is_empty());
    }

    #[test]
    fn content_signatures() {
        let found = match_content(b"./xmrig -o stratum+tcp://pool.supportxmr.com:3333 -u wallet");
        let found = labels(&found);
        assert!(found.contains(&"mining pool address"));
        assert!(found.contains(&"mining pool"));
        assert!(found.contains(&"mentions XMRig"));

        let found = match_content(b"wget https://example.com/xmrig.tar.gz && tar -xf xmrig.tar.gz");
        assert!(labels(&found).contains(&"downloads or starts XMRig"));
        assert!(
            found
                .iter()
                .any(|m| m.label == "downloads or starts XMRig" && m.severity == Critical)
        );

        assert!(
            labels(&match_content(br#"{"inbounds": [], "outbounds": []}"#))
                .contains(&"VPN/proxy core configuration")
        );
        assert!(match_content(br#"{"inbounds": []}"#).is_empty());

        assert!(match_content(b"just a minecraft server").is_empty());
    }

    #[test]
    fn the_shown_text_is_the_group_that_counts() {
        let found = match_content(b"pool: gulf.moneroocean.stream:10128");
        let pool = found.iter().find(|m| m.label == "mining pool").unwrap();
        assert_eq!(pool.text.as_deref(), Some("gulf.moneroocean.stream"));

        let wallet = format!("4{}", "A".repeat(94));
        let found = match_content(format!("wallet={wallet} ").as_bytes());
        let address = found
            .iter()
            .find(|m| m.label == "Monero wallet address")
            .unwrap();
        assert_eq!(address.text.as_deref(), Some(wallet.as_str()));
    }

    #[test]
    fn obfuscation() {
        assert_eq!(
            obfuscated_javascript(b"eval(function(p,a,c,k,e,d){return p})"),
            Some("packed JavaScript")
        );
        let names: String = (0..20).map(|i| format!("var _0x{:04x}=1;", i)).collect();
        assert_eq!(
            obfuscated_javascript(names.as_bytes()),
            Some("obfuscated JavaScript")
        );
        assert_eq!(obfuscated_javascript(b"var _0x1234=1;"), None);
        assert!(obfuscated_python(b"exec(base64.b64decode('aGk='))").is_some());
    }

    #[test]
    fn kernel_threads() {
        assert!(is_kernel_thread_name("kworker/0:1"));
        assert!(!is_kernel_thread_name("java"));
    }
}
