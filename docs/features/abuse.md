# Abuse protection

The panel looks for what the Terms forbid on every server: crypto miners, VPN/proxy servers, DDoS
tools, port scanners and brute forcers, malware and botnets, bulk mail. What it finds is a
**finding** in Admin › Abuse. In `suspend` mode a critical finding suspends the server (kind
`abuse`, see [suspension](suspension.md)) and the server is deleted after the configured hours
unless an admin steps in.

All rules live in the panel (`shared/src/abuse/`, unit tests in each file). Wings only collects:
file listings and contents, console lines, and on nodes running the
[Wings fork](../wings.md) the processes and connections of each container.

## Checks

| Check | When | Looks at |
|---|---|---|
| Full scan (`scan_server`) | every server that is not suspended, every three hours (`scan_abuse`, 00:07, 03:07, ...); on demand from Admin › Abuse › Scan a Server | startup values, every file name in the tree, up to 30 likely files' contents, the last 200 console lines |
| CPU watch (`watch`) | every minute (`watch_abuse`) | each server's CPU against its limit |
| Process check (`watch`) | every minute, nodes running the Wings fork only | what runs in each container and what it connects to, the node's own processes |

**Startup.** The startup command and the variables that name what runs or what gets installed
(`STARTUP_CMD`, `JS_FILE`, `PY_FILE`, `PY_PACKAGES`, `GIT_ADDRESS`, ...) are matched against tool
names and text signatures.

**Files.** Wings' search with no filter lists the whole tree (with sizes and the type sniffed from
each file's first bytes) in one call. A server too big to list in 30 seconds gets a breadth-first
walk instead: 400 folders, 6 deep, skipping bulk data (region files, `node_modules`, caches). Every
name is checked; then up to 30 files of at most 512 KiB are read, most likely first: the egg's main
file, scripts and configs inside a folder that matched a tool name or next to a Linux executable
(where a miner keeps its `config.json`), top-level scripts, any shell script. Library folders are
never read. On a bot egg (Python, Node.js, Bun, Deno or a generic one, matched by the egg's name)
top-level configs and shallow scripts are read too, and an unknown Linux executable is a review
finding. A whole Linux root filesystem inside a server is a `vps` finding on any egg.

**Names.** A distinctive name (`xmrig`, `t-rex`, `masscan`) counts wherever it appears. An ordinary
word that is also a tool (`xray` is an anti-xray Minecraft plugin, `hydra` a mod) counts only for a
Linux executable or a process named exactly that.

**Contents.** Text signatures need all their parts in one file: a mining pool address with a wallet,
`stratum+tcp://`, an Xray/V2Ray config with an inbound, a DDoS script's attack methods, obfuscated
JavaScript (javascript-obfuscator's `_0x` names, 15 or more) and Python hidden behind
`exec(base64...)`. They run on bytes with linear-time regular expressions, so a hostile file cannot
slow the scan.

**Console.** A console also prints what players type, so a signature in free text is at most a
review, and a line that looks relayed (`<Player> ...`, "issued server command") is skipped. Only
output nobody fakes with one chat line is critical: several of XMRig's own log lines from different
modules, the Xray/V2Ray start banner, sing-box's log format, a run of masscan or nmap results.

**CPU.** A miner pins its CPU for hours. A server at the CPU threshold (default 90 % of its limit)
has its console read at once and every 10 minutes after; one that stays there for the configured
minutes (default 20) gets a review finding (`cpu.pinned`) and the full scan, at most every 2 hours.
A little under the threshold keeps the streak (miners wobble), under three quarters of it ends it.
Full CPU alone never suspends: a Minecraft server generating chunks looks the same.

**Processes and connections** (the Wings fork's `GET /api/system/processes`, one call per node):
what the file checks cannot see: a renamed miner (Wings reports strings found in the binary, and a
renamed XMRig still says XMRig), a binary deleted after it started, one running from memory or a
temp folder, a process named like a kernel thread, a script run by an interpreter whose file name
matches. Connections: 100 hosts contacted at once is a port scan, SSH to 15 hosts or RDP to 10 is
brute forcing, Telnet to 10 is botnet spreading, mail to 20 servers is bulk mail, 150 000 packets/s
out is a flood (smaller numbers are reviews). Full CPU with a connection to a common stratum port is
a review. The node's own processes become node findings (alerts only). A stock Wings answers 404
and is skipped.

## Severity and findings

| Severity | Means |
|---|---|
| `critical` | what the Terms forbid outright; suspends in `suspend` mode |
| `review` | worth a look: tunnels, Tor and torrent clients (the Terms do not ban them), unknown executables, sustained full CPU, anything only seen in the console |

One row per server (or node) and rule in `abuse_findings`, keyed by `fingerprint`
(`server:<uuid>:<rule>`): the same rule seen again updates the row (`hits`, `last_seen`, the latest
summary and evidence) instead of adding one. Statuses:

| Status | Means |
|---|---|
| `open` | seen, nothing done yet |
| `actioned` | the server was suspended for it (`action`: `suspended` or `already suspended`) |
| `dismissed` | an admin called it a false positive; stays dismissed when seen again and never suspends |
| `resolved` | an admin closed it, or it was not seen for 7 days (closed by the full scan); reopens when seen again |

Evidence is a JSON object: for signature findings `matches`, at most 15 of `{severity, label,
where, match}`; for `cpu.pinned` the average CPU, the limit, the minutes and the samples.

## Acting

The mode (Admin › Settings › Security › Abuse) decides:

- **`suspend`**: a critical finding that is not dismissed suspends the server under a per-server
  lock, with the findings' summaries as the reason and the deletion time set to now plus the
  configured hours (0: never deleted). The purge (`purge_suspended_servers`, see
  [suspension](suspension.md)) deletes it then, unless it was unsuspended or a finding on it was
  dismissed (which clears the deletion time). Unsuspending without dismissing gets the server
  suspended again on the next hit. A server mid-transfer cannot be suspended; the finding stays
  open and the next check tries again.
- **`report`** (the default, so a fresh install shows what it would do first): findings are recorded
  and alerted, nothing is suspended.
- **`off`**: nothing is checked.

Admin › Abuse › Scan a Server records findings like the scheduled scan but never suspends: the admin
decides. Review findings never suspend.

**Owner sweep.** Someone caught mining on one server usually runs the same kit on the rest: after a
new critical finding, the owner's other servers get the full scan right away in the background,
once per owner per hour.

**Alerts.** With an Alert Webhook (a Discord webhook URL) set, the panel posts an embed for a
suspension (red), a new critical finding in `report` mode ("would suspend", yellow), a node finding,
and the deletion of an abuse-suspended server. One finding alert per server per 30 minutes;
suspensions and deletions always post.

## Admin › Abuse

The findings, newest first, filtered by status (open by default) and searchable by summary, server,
owner or node name. A row opens the finding with its evidence; its menu resolves or dismisses it.
"Scan a Server" runs the full scan on one server and shows what it found (a scan slower than 45
seconds keeps running and its findings appear in the list).

Permissions: `abuse.read` (see findings), `abuse.manage` (dismiss, resolve, scan).

## Settings

Admin › Settings › Security › Abuse (`shared/src/settings/abuse.rs`, section `abuse`):

| Setting | Default | Means |
|---|---|---|
| Mode | `report` | `suspend`, `report` or `off`, see above |
| Delete After (Hours) | 24 | hours from an abuse suspension to deletion; 0 never deletes |
| Alert Webhook | none | Discord webhook URL; stored encrypted, shown as stored, removed with "Remove Webhook" |
| CPU Threshold | 90 | percent of its CPU limit (50–100) that starts a CPU streak |
| CPU Minutes | 20 | minutes at the threshold (5–1440) before the review finding and the full scan |

## API

| Method | Path | Permission |
|---|---|---|
| `GET` | `/api/admin/abuse/findings?page&per_page&search&status` | `abuse.read` |
| `PATCH` | `/api/admin/abuse/findings/{finding}` `{status: "dismissed" \| "resolved"}` | `abuse.manage` |
| `POST` | `/api/admin/abuse/scan` `{server_uuid}` | `abuse.manage` |

Admin activity: `abuse:finding.dismiss`, `abuse:finding.resolve`, `abuse:scan`.
