//! plugged.host: the abuse checks themselves, without I/O: what a server's startup values, files,
//! console and processes say. `super` fetches the inputs from the database and Wings.

use super::{
    Finding, Severity,
    Severity::{Critical, Review},
    signatures,
};
use regex::bytes::Regex;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::LazyLock,
};

/// One signature that matched, and where.
#[derive(Debug, Clone)]
pub struct Hit {
    pub category: &'static str,
    pub severity: Severity,
    pub label: String,
    pub location: String,
    pub text: Option<String>,
}

impl Hit {
    fn new(
        category: &'static str,
        severity: Severity,
        label: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self {
            category,
            severity,
            label: label.into(),
            location: location.into(),
            text: None,
        }
    }

    fn from_match(found: signatures::Match, location: impl Into<String>) -> Self {
        Self {
            category: found.category,
            severity: found.severity,
            label: found.label.to_string(),
            location: location.into(),
            text: found.text,
        }
    }
}

/// `s` cut to `width` characters, the last one an ellipsis when it was longer.
pub fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }

    let mut clipped: String = s.chars().take(width.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

/// Collapses hits into one finding per source and category: the strongest severity wins, and its
/// matches lead the summary. The summary is what an owner reads on their suspended server, so it
/// says what was found and where, without jargon.
pub fn findings_from_hits(source: &str, hits: Vec<Hit>) -> Vec<Finding> {
    let mut order: Vec<&'static str> = Vec::new();
    let mut groups: HashMap<&'static str, Vec<Hit>> = HashMap::new();
    for hit in hits {
        if !groups.contains_key(hit.category) {
            order.push(hit.category);
        }
        groups.entry(hit.category).or_default().push(hit);
    }

    order
        .into_iter()
        .map(|category| {
            let mut group = groups.remove(category).unwrap_or_default();
            // Stable: critical first, in the order they were found.
            group.sort_by_key(|hit| hit.severity != Critical);

            let severity = group[0].severity;
            let mut parts: Vec<String> = Vec::new();
            for hit in group.iter().filter(|hit| hit.severity == severity) {
                let part = format!("{}: {}", hit.label, hit.location);
                if !parts.contains(&part) {
                    parts.push(part);
                }
            }

            let mut summary = format!("{} ({})", signatures::category_label(category), parts[0]);
            if parts.len() > 1 {
                summary.push_str(&format!(" and {} more", parts.len() - 1));
            }

            let matches: Vec<serde_json::Value> = group
                .iter()
                .take(15)
                .map(|hit| {
                    let mut entry = json!({
                        "severity": hit.severity.as_str(),
                        "label": hit.label,
                        "where": clip(&hit.location, 300),
                    });
                    if let Some(text) = &hit.text {
                        entry["match"] = json!(clip(text, 200));
                    }
                    entry
                })
                .collect();

            Finding {
                rule: format!("{source}.{category}"),
                category: category.to_string(),
                severity,
                source: source.to_string(),
                summary: clip(&summary, 480),
                evidence: json!({ "matches": matches }),
            }
        })
        .collect()
}

// --- Startup ---------------------------------------------------------------------------------

/// Variables whose value is something the server runs or installs, so a bare tool name counts.
const RUN_VARIABLES: &[&str] = &[
    "PY_FILE",
    "JS_FILE",
    "MAIN_FILE",
    "BOT_JS_FILE",
    "BOT_PY_FILE",
    "STARTUP_FILE",
    "STARTUP_CMD",
    "CMD_RUN",
    "PY_PACKAGES",
    "NODE_PACKAGES",
    "NODE_ARGS",
    "GIT_ADDRESS",
];

static STARTUP_TOKENS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[\s"'=;,&|()<>]+"#).expect("compiles"));

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The startup command and the variable values (env name, value): a GIT_ADDRESS that clones
/// xmrig, PY_PACKAGES that installs a proxy, a startup line that runs ./xray. No node call.
pub fn inspect_startup(startup: &str, variables: &[(String, String)]) -> Vec<Finding> {
    let mut values: Vec<(&str, &str)> = vec![("startup command", startup)];
    for (env, value) in variables {
        let value = value.trim();
        if !value.is_empty() {
            values.push((env.as_str(), value));
        }
    }

    let mut hits = Vec::new();
    for (location, value) in values {
        for found in signatures::match_content(value.as_bytes()) {
            hits.push(Hit::from_match(found, location));
        }

        let runs = location == "startup command" || RUN_VARIABLES.contains(&location);
        for token in STARTUP_TOKENS.split(value.as_bytes()) {
            let token = String::from_utf8_lossy(token);
            let name = basename(token.trim_end_matches('/'));
            for found in signatures::match_name(name, runs) {
                let mut hit = Hit::from_match(found, location);
                hit.text = Some(token.to_string());
                hits.push(hit);
            }
        }
    }

    findings_from_hits("startup", hits)
}

// --- Files -----------------------------------------------------------------------------------

/// Files whose contents are read per scan, at most.
pub const MAX_READS: usize = 30;
pub const MAX_READ_BYTES: u64 = 524_288;

/// Never read inside these. Names inside them are still matched.
const LIBRARY_DIRS: &[&str] = &[
    ".local",
    "node_modules",
    ".cache",
    ".npm",
    ".yarn",
    ".pnpm-store",
    "venv",
    ".venv",
    "env",
    "__pycache__",
    ".git",
    "site-packages",
    "dist-packages",
    "libraries",
    "versions",
    "logs",
    "crash-reports",
    "cache",
    ".fabric",
    ".paper",
    ".quilt",
    "bundler",
    "world",
    "world_nether",
    "world_the_end",
    "alpine",
    "server-data",
    "resources",
    "mods",
    "plugins",
    "config",
    "defaultconfigs",
    "kubejs",
    "journeymap",
    ".mixin.out",
    "dynmap",
    "squaremap",
    "bluemap",
];

/// Bulk data the fallback walk does not open: world chunks, player data, packages, caches.
pub const WALK_SKIP: &[&str] = &[
    "region",
    "entities",
    "poi",
    "dim-1",
    "dim1",
    "playerdata",
    "stats",
    "advancements",
    "node_modules",
    ".git",
    "__pycache__",
    ".cache",
    ".npm",
    "logs",
    "crash-reports",
    "libraries",
    "versions",
    "cache",
    "site-packages",
    "dist-packages",
    "backups",
    "bluemap",
    "dynmap",
    "squaremap",
];

const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "js", "mjs", "cjs", "ts", "sh", "bash", "pl", "rb", "php",
];
const CONFIG_EXTENSIONS: &[&str] = &[
    "json", "jsonc", "yml", "yaml", "toml", "ini", "conf", "cfg", "env", "txt",
];
const EXECUTABLE_MIMES: &[&str] = &[
    "application/x-executable",
    "application/x-pie-executable",
    "application/x-elf",
];
/// Data files whose names are not worth matching (a texture called xmrig.png is not a miner).
const DATA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "ogg", "mp3", "wav", "mca", "mcr", "nbt", "dat",
    "dat_old", "lang", "class", "pyc", "map",
];
/// Variables that name the file the egg runs.
pub const MAIN_FILE_VARIABLES: &[&str] = &[
    "PY_FILE",
    "JS_FILE",
    "MAIN_FILE",
    "BOT_JS_FILE",
    "BOT_PY_FILE",
    "STARTUP_FILE",
];

/// A whole Linux system unpacked in a server folder: the proot "free VPS" most proxy kits run in.
static ROOTFS_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|/)(?:etc/os-release|etc/apt/sources\.list|usr/bin/apt-get|usr/bin/dpkg)$")
        .expect("compiles")
});
/// Eggs that run the owner's own code, where the console and the files are theirs end to end.
static GENERIC_EGG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)python|node|bun|deno|generic").expect("compiles"));

pub fn is_generic_egg(egg_name: &str) -> bool {
    GENERIC_EGG.is_match(egg_name.as_bytes())
}

/// One entry of a server's file tree, its path relative to the server's root.
#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: String,
    pub directory: bool,
    pub size: u64,
    pub mime: String,
}

fn in_library(path: &str) -> bool {
    path.split('/')
        .any(|segment| LIBRARY_DIRS.contains(&segment.to_ascii_lowercase().as_str()))
}

fn dirname(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(dir, _)| dir)
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

struct FileInfo {
    size: u64,
    extension: String,
    base: String,
}

/// What the names in a file tree say, and which files are worth reading.
pub struct FilePlan {
    hits: Vec<Hit>,
    files: HashMap<String, FileInfo>,
    unknown_executables: Vec<String>,
    main_files: HashSet<String>,
    generic: bool,
    /// The files to read, most likely first.
    pub reads: Vec<String>,
}

/// Every name in the tree, at any depth, plus the choice of up to [`MAX_READS`] small files to
/// read: the egg's main file, scripts and configs inside a folder that matched a tool name or
/// next to a Linux executable (where a miner keeps its config.json), scripts at the top level,
/// any shell script. Library folders (.local, node_modules, site-packages, ...) are never read;
/// their names are still checked.
pub fn plan_files(tree: &[TreeEntry], main_files: HashSet<String>, generic: bool) -> FilePlan {
    let mut hits = Vec::new();
    let mut tool_dirs: HashSet<String> = HashSet::new();
    let mut executable_dirs: HashSet<String> = HashSet::new();
    let mut unknown_executables = Vec::new();
    let mut files: HashMap<String, FileInfo> = HashMap::new();
    let mut file_order: Vec<String> = Vec::new();

    for entry in tree {
        let path = entry.path.trim_matches('/');
        if path.is_empty() {
            continue;
        }

        let mime = entry.mime.to_ascii_lowercase();
        let base = basename(path);
        let extension = extension(base);
        let is_executable = !entry.directory
            && EXECUTABLE_MIMES.contains(&mime.split(';').next().unwrap_or_default().trim());

        if ROOTFS_MARKER.is_match(path.as_bytes()) {
            hits.push(Hit::new("vps", Review, "Linux root filesystem", path));
        }

        let media =
            mime.starts_with("image/") || mime.starts_with("audio/") || mime.starts_with("video/");
        if !DATA_EXTENSIONS.contains(&extension.as_str()) && !media {
            for found in signatures::match_name(base, is_executable) {
                let location = if entry.directory {
                    format!("{path}/")
                } else {
                    path.to_string()
                };
                hits.push(Hit::from_match(found, location));
                tool_dirs.insert(if entry.directory {
                    path.to_string()
                } else {
                    dirname(path).to_string()
                });
            }
        }

        if entry.directory {
            continue;
        }

        if is_executable && !in_library(path) {
            executable_dirs.insert(dirname(path).to_string());
            unknown_executables.push(path.to_string());
        }

        if !files.contains_key(path) {
            file_order.push(path.to_string());
        }
        files.insert(
            path.to_string(),
            FileInfo {
                size: entry.size,
                extension,
                base: base.to_ascii_lowercase(),
            },
        );
    }

    let mut scored: Vec<(String, u32)> = Vec::new();
    for path in file_order {
        let file = &files[&path];
        if file.size == 0 || file.size > MAX_READ_BYTES {
            continue;
        }

        let dir = dirname(&path);
        let top_level = dir == ".";
        let script = SCRIPT_EXTENSIONS.contains(&file.extension.as_str());
        let config = CONFIG_EXTENSIONS.contains(&file.extension.as_str()) || file.base == ".env";

        let score = if main_files.contains(&path) {
            100
        } else if tool_dirs.contains(dir) && (script || config) {
            90
        } else if executable_dirs.contains(dir) && (script || config) {
            80
        } else if in_library(&path) {
            continue;
        } else if top_level && script {
            70
        } else if file.extension == "sh" || file.extension == "bash" {
            60
        } else if top_level
            && ["package.json", "requirements.txt", ".env", "pyproject.toml"]
                .contains(&file.base.as_str())
        {
            50
        } else if generic && top_level && config {
            40
        } else if generic && script && path.matches('/').count() <= 2 {
            30
        } else {
            continue;
        };

        scored.push((path, score));
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let reads = scored
        .into_iter()
        .take(MAX_READS)
        .map(|(path, _)| path)
        .collect();

    FilePlan {
        hits,
        files,
        unknown_executables,
        main_files,
        generic,
        reads,
    }
}

impl FilePlan {
    /// Adds what one read file says.
    pub fn add_content(&mut self, path: &str, content: &[u8]) {
        for found in signatures::match_content(content) {
            self.hits.push(Hit::from_match(found, path));
        }

        let extension = self
            .files
            .get(path)
            .map(|file| file.extension.as_str())
            .unwrap_or_default();
        let top_level_or_main = !path.contains('/') || self.main_files.contains(path);

        if matches!(extension, "js" | "mjs" | "cjs")
            && top_level_or_main
            && let Some(what) = signatures::obfuscated_javascript(content)
        {
            self.hits
                .push(Hit::new("obfuscation", Critical, what, path));
        }

        if extension == "py"
            && let Some(what) = signatures::obfuscated_python(content)
        {
            self.hits.push(Hit::new("obfuscation", Review, what, path));
        }
    }

    pub fn findings(self) -> Vec<Finding> {
        let mut findings = findings_from_hits("files", self.hits);

        // A Linux executable a Python or Node.js server did not get from pip/npm is worth a look,
        // but on its own it is a review: ffmpeg, yt-dlp and cloudflared builds are common.
        let flagged: HashSet<String> = findings
            .iter()
            .flat_map(|finding| {
                finding.evidence["matches"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|hit| hit["where"].as_str().map(str::to_string))
            .collect();
        let unknown: Vec<String> = self
            .unknown_executables
            .into_iter()
            .filter(|path| !flagged.contains(path))
            .collect();

        if self.generic && !unknown.is_empty() {
            let mut summary = format!(
                "Linux executable outside any package folder ({})",
                unknown[0]
            );
            if unknown.len() > 1 {
                summary.push_str(&format!(" and {} more", unknown.len() - 1));
            }

            findings.push(Finding {
                rule: "files.executable".into(),
                category: "executable".into(),
                severity: Review,
                source: "files".into(),
                summary: clip(&summary, 480),
                evidence: json!({ "executables": unknown.iter().take(20).collect::<Vec<_>>() }),
            });
        }

        findings
    }
}

/// The directories of a file-only listing (Wings' search returns files), so their names are
/// matched too: `xmrig-6.26.0/` holds files.
pub fn with_directories(files: Vec<TreeEntry>) -> Vec<TreeEntry> {
    let mut directories: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for file in &files {
        let mut dir = dirname(file.path.trim_matches('/'));
        while dir != "." && !dir.is_empty() {
            if !seen.insert(dir.to_string()) {
                break;
            }
            directories.push(dir.to_string());
            dir = dirname(dir);
        }
    }

    let mut tree: Vec<TreeEntry> = directories
        .into_iter()
        .map(|path| TreeEntry {
            path,
            directory: true,
            size: 0,
            mime: String::new(),
        })
        .collect();
    tree.extend(files);
    tree
}

// --- Console ---------------------------------------------------------------------------------

pub const CONSOLE_LINES: u64 = 200;

static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\r").expect("compiles")
});
static RELAYED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)<[^<>\s]{1,32}>|issued server command|\[Not Secure\]|\bsaid:|\[CHAT\]|\bwhispers\b",
    )
    .expect("compiles")
});
static XMRIG_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}\]\s+(net|cpu|miner|randomx|msr|config|rx|signal|opencl|cuda|hwloc|nvml|dmi)\s")
        .expect("compiles")
});
static XMRIG_BANNER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s\*\s+ABOUT\s+XMRig/\d").expect("compiles"));
static PROXY_BANNER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:Xray \d+\.\d+\.\d+ \(Xray, Penetrates Everything\.\)|V2Ray \d+\.\d+\.\d+ \(|sing-box version \d)")
        .expect("compiles")
});
static SING_BOX_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[+-]\d{4} \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} (?:INFO|WARN|ERROR|DEBUG|TRACE) (?:inbound|outbound|router|dns)/")
        .expect("compiles")
});
static MASSCAN_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^Discovered open port \d+/(?:tcp|udp) on \d{1,3}(?:\.\d{1,3}){3}")
        .expect("compiles")
});
static NMAP_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Nmap scan report for \S").expect("compiles"));
static TUNNEL_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Registered tunnel connection|\.trycloudflare\.com|started tunnel|url=https://\S+\.ngrok|login to server success|start proxy success")
        .expect("compiles")
});

/// The last lines of a server's console, for tools that announce themselves.
///
/// A console also prints what other people typed: Minecraft chat, a Discord bot echoing
/// messages. So a signature in free text is never more than a review here, and a line that looks
/// relayed (`<Player> ...`, "issued server command") is skipped entirely. Only output a person
/// cannot fake with one chat line is critical: several of XMRig's own timestamped log lines from
/// different modules, the Xray/V2Ray start banner at the start of a line, sing-box's log format,
/// or a run of masscan/nmap results.
pub fn inspect_console(lines: &[String]) -> Vec<Finding> {
    let console = |line: &str| format!("console: {}", clip(line.trim(), 120));

    let mut hits = Vec::new();
    let mut xmrig_lines = 0;
    let mut xmrig_modules: HashSet<String> = HashSet::new();
    let mut xmrig_banner = false;
    let mut xmrig_first: Option<String> = None;
    let mut counts: BTreeMap<&'static str, (u32, String)> = BTreeMap::new();

    for raw in lines {
        let cleaned = ANSI.replace_all(raw.as_bytes(), &b""[..]);
        let line = String::from_utf8_lossy(&cleaned);
        let line = line.trim_end();
        if line.is_empty() || RELAYED.is_match(line.as_bytes()) {
            continue;
        }

        if let Some(captures) = XMRIG_LINE.captures(line.as_bytes()) {
            xmrig_lines += 1;
            xmrig_modules.insert(String::from_utf8_lossy(&captures[1]).into_owned());
            xmrig_first.get_or_insert_with(|| line.to_string());
        }
        if XMRIG_BANNER.is_match(line.as_bytes()) {
            xmrig_banner = true;
            xmrig_first.get_or_insert_with(|| line.to_string());
        }
        if PROXY_BANNER.is_match(line.as_bytes()) {
            hits.push(Hit::new(
                "proxy",
                Critical,
                "VPN/proxy core started",
                console(line),
            ));
        }
        for (tool, regex) in [
            ("sing-box", &*SING_BOX_LINE),
            ("masscan", &*MASSCAN_LINE),
            ("nmap", &*NMAP_LINE),
        ] {
            if regex.is_match(line.as_bytes()) {
                counts
                    .entry(tool)
                    .or_insert_with(|| (0, line.to_string()))
                    .0 += 1;
            }
        }
        if TUNNEL_LINE.is_match(line.as_bytes()) {
            hits.push(Hit::new("tunnel", Review, "tunnel output", console(line)));
        }
        for found in signatures::match_content(line.as_bytes()) {
            let mut hit = Hit::from_match(found, console(line));
            hit.severity = Review;
            hits.push(hit);
        }
    }

    if (xmrig_lines >= 3 && xmrig_modules.len() >= 2) || (xmrig_banner && xmrig_lines >= 1) {
        hits.push(Hit::new(
            "miner",
            Critical,
            "XMRig output",
            console(xmrig_first.as_deref().unwrap_or_default()),
        ));
    }
    let count = |tool: &str| counts.get(tool).map_or(0, |(count, _)| *count);
    let first = |tool: &str| {
        counts
            .get(tool)
            .map(|(_, line)| console(line))
            .unwrap_or_default()
    };
    if count("sing-box") >= 3 {
        hits.push(Hit::new(
            "proxy",
            Critical,
            "sing-box output",
            first("sing-box"),
        ));
    }
    if count("masscan") >= 5 || count("nmap") >= 3 {
        let tool = if count("masscan") >= 5 {
            "masscan"
        } else {
            "nmap"
        };
        hits.push(Hit::new(
            "scanner",
            Critical,
            format!("{tool} results"),
            first(tool),
        ));
    }

    findings_from_hits("console", hits)
}

// --- Processes -------------------------------------------------------------------------------

static INTERPRETER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:python[0-9.]*|pypy[0-9.]*|node|nodejs|bun|deno|perl|ruby|php[0-9.]*|bash|sh|dash|ash)$").expect("compiles")
});
static TEMP_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:/tmp/|/var/tmp/|/dev/shm/|/run/shm/)").expect("compiles"));

/// Common stratum ports. Several are also game ports, so a connection to one is only a review.
const STRATUM_PORTS: &[u16] = &[
    3333, 4444, 5555, 6666, 7777, 9999, 10128, 10343, 13333, 14433, 14444, 17777, 20128, 45560,
    45700,
];

fn process_hits(process: &wings_api::fork::NodeProcess, in_container: bool) -> Vec<Hit> {
    let comm: String = process.comm.chars().take(64).collect();
    let exe_raw = process
        .exe
        .strip_suffix(" (deleted)")
        .unwrap_or(&process.exe);
    let deleted = process.deleted || process.exe.ends_with(" (deleted)");
    let exe: String = exe_raw.chars().take(512).collect();
    let cmd: String = process.cmd.chars().take(1024).collect();
    let cpu = process.cpu.max(0.0);

    let location = format!(
        "process '{comm}' (pid {}, {}% CPU)",
        process.pid,
        cpu.round() as i64
    );
    let shown = clip(if cmd.is_empty() { &exe } else { &cmd }, 200);
    let hit = |category: &'static str, severity: Severity, label: &str, text: Option<String>| Hit {
        category,
        severity,
        label: label.to_string(),
        location: location.clone(),
        text: Some(text.unwrap_or_else(|| shown.clone())),
    };

    let mut hits = Vec::new();
    let argv: Vec<&str> = cmd.split_whitespace().collect();
    let argv0 = basename(argv.first().copied().unwrap_or_default());
    let exe_base = basename(&exe).to_ascii_lowercase();

    let mut names: Vec<String> = Vec::new();
    for name in [
        comm.to_ascii_lowercase(),
        exe_base.clone(),
        argv0.to_ascii_lowercase(),
    ] {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    for name in &names {
        for found in signatures::match_name(name, true) {
            hits.push(hit(found.category, found.severity, found.label, None));
        }
    }

    // `python3 xmrig.py`, `node proxy.js`: the script is the program. Matched as a file name,
    // not as a command, so `java -jar xray-detector.jar` stays a Minecraft plugin.
    if INTERPRETER.is_match(argv0.as_bytes()) || INTERPRETER.is_match(exe_base.as_bytes()) {
        if let Some(script) = argv
            .iter()
            .skip(1)
            .find(|arg| !arg.is_empty() && !arg.starts_with('-'))
        {
            for found in signatures::match_name(basename(script), false) {
                hits.push(hit(found.category, found.severity, found.label, None));
            }
        }
    }

    for found in signatures::match_content(cmd.as_bytes()) {
        hits.push(hit(found.category, found.severity, found.label, found.text));
    }

    // Strings Wings found in the binary itself: a renamed XMRig still says XMRig inside.
    let tags: Vec<&str> = process.tags.iter().map(|tag| tag.as_str()).collect();
    if tags
        .iter()
        .any(|tag| ["xmrig", "stratum", "donate-level"].contains(tag))
    {
        hits.push(hit(
            "miner",
            Critical,
            "XMRig-based binary",
            Some(tags.join(", ")),
        ));
    } else if tags
        .iter()
        .any(|tag| ["randomx", "cryptonight"].contains(tag))
    {
        let severity = if cpu >= 50.0 { Critical } else { Review };
        hits.push(hit(
            "miner",
            severity,
            "mining code in the binary",
            Some(tags.join(", ")),
        ));
    }

    // Hiding. Inside a container there are no kernel threads at all, and on the host a real one
    // has no executable (Wings leaves those out), so the name alone is the tell.
    if signatures::is_kernel_thread_name(&comm) || cmd.starts_with('[') {
        hits.push(hit(
            "evasion",
            Critical,
            "disguised as a kernel thread",
            None,
        ));
    }
    if exe.starts_with("/memfd:") && !comm.starts_with("runc") {
        let severity = if cpu >= 25.0 || !in_container {
            Critical
        } else {
            Review
        };
        hits.push(hit("evasion", severity, "runs from memory only", None));
    }

    let temp = TEMP_DIR.is_match(exe.as_bytes());
    let in_place_severity = if in_container { Review } else { Critical };
    if deleted && (if in_container { cpu >= 25.0 } else { temp }) {
        hits.push(hit(
            "evasion",
            in_place_severity,
            "binary deleted after start",
            None,
        ));
    } else if temp {
        hits.push(hit(
            "evasion",
            in_place_severity,
            "runs from a temp folder",
            None,
        ));
    }

    hits
}

/// Connection patterns the Terms forbid: scanning, brute forcing, botnet spreading, spam, floods.
fn network_hits(net: &wings_api::fork::NodeContainerNet, cpu: f64) -> Vec<Hit> {
    let port = |port: u16| net.ports.get(&port).copied().unwrap_or(0);
    let mut hits = Vec::new();
    let mut add = |category: &'static str, severity: Severity, label: &str, location: String| {
        hits.push(Hit::new(
            category,
            severity,
            label,
            format!("network: {location}"),
        ));
    };

    let connecting = net.syn_sent_ips;
    if connecting >= 100 {
        add(
            "scanner",
            Critical,
            "port scan",
            format!("{connecting} hosts contacted at once"),
        );
    } else if connecting >= 30 {
        add(
            "scanner",
            Review,
            "many connection attempts",
            format!("{connecting} hosts contacted at once"),
        );
    }

    let ssh = port(22);
    if ssh >= 15 {
        add(
            "scanner",
            Critical,
            "SSH brute forcing",
            format!("SSH connections to {ssh} hosts"),
        );
    } else if ssh >= 5 {
        add(
            "scanner",
            Review,
            "SSH connections",
            format!("SSH connections to {ssh} hosts"),
        );
    }

    let telnet = port(23) + port(2323);
    if telnet >= 10 {
        add(
            "malware",
            Critical,
            "botnet spreading over Telnet",
            format!("Telnet connections to {telnet} hosts"),
        );
    }

    let rdp = port(3389);
    if rdp >= 10 {
        add(
            "scanner",
            Critical,
            "RDP brute forcing",
            format!("RDP connections to {rdp} hosts"),
        );
    }

    let mail = port(25) + port(465) + port(587);
    if mail >= 20 {
        add(
            "spam",
            Critical,
            "bulk mail",
            format!("mail connections to {mail} servers"),
        );
    } else if mail >= 5 {
        add(
            "spam",
            Review,
            "mail sending",
            format!("mail connections to {mail} servers"),
        );
    }

    let pps = net.tx_pps.unwrap_or(0);
    if pps >= 150_000 {
        add(
            "ddos",
            Critical,
            "outgoing flood",
            format!("{pps} packets/s sent"),
        );
    } else if pps >= 50_000 {
        add(
            "ddos",
            Review,
            "heavy outgoing traffic",
            format!("{pps} packets/s sent"),
        );
    }

    if cpu >= 100.0 {
        let pools: Vec<String> = STRATUM_PORTS
            .iter()
            .filter(|stratum| port(**stratum) > 0)
            .map(|stratum| stratum.to_string())
            .collect();
        if !pools.is_empty() {
            add(
                "miner",
                Review,
                "full CPU and a connection to a common mining port",
                pools.join(", "),
            );
        }
    }

    hits
}

/// What runs in one server's container and what it connects to: what the file checks cannot
/// see (a renamed miner, one deleted after it started, one running from memory, a process
/// wearing a kernel thread's name).
pub fn inspect_container(container: &wings_api::fork::NodeContainer) -> Vec<Finding> {
    let hits = container
        .processes
        .iter()
        .flat_map(|process| process_hits(process, true))
        .collect();
    let cpu: f64 = container
        .processes
        .iter()
        .map(|process| process.cpu.max(0.0))
        .sum();

    let mut findings = findings_from_hits("process", hits);
    findings.extend(findings_from_hits(
        "network",
        network_hits(&container.net, cpu),
    ));
    findings
}

/// The node's own processes. Nothing to suspend here: these become node alerts.
pub fn inspect_host(processes: &[wings_api::fork::NodeProcess]) -> Vec<Finding> {
    findings_from_hits(
        "host",
        processes
            .iter()
            .flat_map(|process| process_hits(process, false))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, size: u64, mime: &str) -> TreeEntry {
        TreeEntry {
            path: path.into(),
            directory: false,
            size,
            mime: mime.into(),
        }
    }

    fn process(comm: &str, exe: &str, cmd: &str, cpu: f64) -> wings_api::fork::NodeProcess {
        wings_api::fork::NodeProcess {
            pid: 42,
            ppid: 1,
            comm: comm.into(),
            exe: exe.into(),
            cmd: cmd.into(),
            cpu,
            deleted: false,
            tags: Vec::new(),
        }
    }

    #[test]
    fn hits_collapse_per_category() {
        let findings = findings_from_hits(
            "files",
            vec![
                Hit::new("miner", Review, "mentions XMRig", "README.md"),
                Hit::new("miner", Critical, "XMRig", "xmrig-6.26.0/"),
                Hit::new("miner", Critical, "XMRig", "xmrig-6.26.0/xmrig"),
                Hit::new("tunnel", Review, "ngrok", "ngrok"),
            ],
        );

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, "files.miner");
        assert_eq!(findings[0].severity, Critical);
        assert_eq!(
            findings[0].summary,
            "Crypto miner (XMRig: xmrig-6.26.0/) and 1 more"
        );
        assert_eq!(findings[0].evidence["matches"].as_array().unwrap().len(), 3);
        assert_eq!(findings[1].severity, Review);
    }

    #[test]
    fn clip_counts_characters() {
        assert_eq!(clip("abc", 3), "abc");
        assert_eq!(clip("abcd", 3), "ab…");
        assert_eq!(clip("äöüß", 3), "äö…");
    }

    #[test]
    fn startup_values() {
        let findings = inspect_startup(
            "./xray run -c config.json",
            &[(
                "GIT_ADDRESS".into(),
                "https://github.com/someone/xmrig".into(),
            )],
        );
        assert!(
            findings
                .iter()
                .any(|f| f.rule == "startup.proxy" && f.severity == Critical)
        );
        assert!(findings.iter().any(|f| f.rule == "startup.miner"));

        assert!(inspect_startup("java -Xms128M -jar server.jar", &[]).is_empty());
    }

    #[test]
    fn a_miner_folder_in_the_tree() {
        let tree = with_directories(vec![
            entry("app.py", 100, "text/x-python"),
            entry("xmrig-6.26.0/xmrig", 8_000_000, "application/x-executable"),
            entry("xmrig-6.26.0/config.json", 2_000, "application/json"),
        ]);
        let mut plan = plan_files(&tree, HashSet::new(), true);

        assert_eq!(
            plan.reads.first().map(String::as_str),
            Some("xmrig-6.26.0/config.json")
        );
        plan.add_content(
            "xmrig-6.26.0/config.json",
            br#"{"donate-level": 1, "algo": "rx/0"}"#,
        );
        let findings = plan.findings();

        let miner = findings.iter().find(|f| f.category == "miner").unwrap();
        assert_eq!(miner.severity, Critical);
        // The executable was flagged by name, so it is not also an unknown one.
        assert!(!findings.iter().any(|f| f.rule == "files.executable"));
    }

    #[test]
    fn library_folders_are_not_read() {
        let tree = with_directories(vec![
            entry("node_modules/x/index.js", 100, "text/javascript"),
            entry("index.js", 100, "text/javascript"),
        ]);
        let plan = plan_files(&tree, HashSet::new(), true);
        assert_eq!(plan.reads, ["index.js"]);
    }

    #[test]
    fn an_unknown_executable_is_a_review_on_generic_eggs() {
        let tree = vec![entry("bin/tool", 1_000, "application/x-executable")];
        let findings = plan_files(&tree, HashSet::new(), true).findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "files.executable");
        assert_eq!(findings[0].severity, Review);

        assert!(
            plan_files(&tree, HashSet::new(), false)
                .findings()
                .is_empty()
        );
    }

    #[test]
    fn console_chat_is_never_critical() {
        let findings = inspect_console(&["<Steve> download xmrig from supportxmr.com".to_string()]);
        assert!(findings.is_empty());

        let findings =
            inspect_console(&["someone said xmrig is fast, stratum+tcp://a.b:1".to_string()]);
        assert!(findings.iter().all(|f| f.severity == Review));
    }

    #[test]
    fn xmrig_output_is_critical() {
        let lines: Vec<String> = [
            "[2026-10-03 12:00:00.123]  net      use pool gulf.moneroocean.stream:10128",
            "[2026-10-03 12:00:01.123]  cpu      use profile rx",
            "[2026-10-03 12:00:02.123]  miner    speed 10s/60s/15m 1234.5 n/a n/a H/s",
        ]
        .iter()
        .map(|line| line.to_string())
        .collect();

        let findings = inspect_console(&lines);
        let miner = findings.iter().find(|f| f.category == "miner").unwrap();
        assert_eq!(miner.severity, Critical);
    }

    #[test]
    fn processes() {
        let container = wings_api::fork::NodeContainer {
            id: "abc".into(),
            server: None,
            processes: vec![
                process("java", "/usr/bin/java", "java -jar server.jar", 80.0),
                process("kworker/0:1", "/home/container/.x", "[kworker/0:1]", 390.0),
                process(
                    "python3",
                    "/usr/bin/python3",
                    "python3 xmrig.py --threads 4",
                    10.0,
                ),
            ],
            net: Default::default(),
        };

        let findings = inspect_container(&container);
        let evasion = findings.iter().find(|f| f.category == "evasion").unwrap();
        assert_eq!(evasion.severity, Critical);
        assert!(
            findings
                .iter()
                .any(|f| f.category == "miner" && f.severity == Critical)
        );

        let quiet = wings_api::fork::NodeContainer {
            processes: vec![process(
                "java",
                "/usr/bin/java",
                "java -jar xray-detector.jar",
                80.0,
            )],
            ..container
        };
        assert!(inspect_container(&quiet).is_empty());
    }

    #[test]
    fn network() {
        let mut net = wings_api::fork::NodeContainerNet::default();
        net.ports.insert(22, 40);
        net.syn_sent_ips = 150;
        let hits = network_hits(&net, 10.0);
        assert!(
            hits.iter()
                .any(|h| h.label == "SSH brute forcing" && h.severity == Critical)
        );
        assert!(hits.iter().any(|h| h.label == "port scan"));

        let mut net = wings_api::fork::NodeContainerNet::default();
        net.ports.insert(3333, 1);
        assert!(network_hits(&net, 50.0).is_empty());
        assert_eq!(network_hits(&net, 100.0).len(), 1);
    }
}
