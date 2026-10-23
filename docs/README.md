# plugged.host panel: documentation

This repository is the panel behind [plugged.host](https://plugged.host), a free game-server host. It
is a fork of [Calagopus](https://github.com/calagopus/panel) (MIT, a Rust/Axum + React/Mantine panel
compatible with Pterodactyl and Pelican eggs). plugged.host's changes are made **directly in the
source**, not as Calagopus extensions: every feature is written as if Calagopus had always had it,
where Calagopus keeps its own features.

These pages are for people and AI agents who want to work on the panel or run their own fork of it.
Calagopus's own documentation ([calagopus.com/docs](https://calagopus.com/docs)) covers everything
this fork leaves as it is: installing, nodes (Wings), eggs, backups, the API.

## Pages

| Page | What it covers |
|---|---|
| [architecture.md](architecture.md) | The crates and folders, how a request flows, where each kind of code lives, settings, tasks, events, permissions, strings |
| [development.md](development.md) | Running the panel from a fresh clone, tests, checks, migrations, adding strings |
| [contributing.md](contributing.md) | Placement and naming rules, the `plugged.host` marker, what a change brings along, commit messages, security reports |
| [upstream.md](upstream.md) | Merging Calagopus, the conflict hot spots, the migration chain, every change made to upstream files |
| [forking.md](forking.md) | Running your own brand: name, logo, colours, product names, version, languages, optional services |
| [noprofit.md](noprofit.md) | The optional noprofit.space integration (sign-in, account sync, SFTP passwords) and its wire format |
| [wings.md](wings.md) | What the Wings fork adds and which features need it |
| [features/](features/) | One page per feature: what it does, settings, tables, routes, permissions, tasks, how it behaves without noprofit.space, and what to check |

For AI agents: [`AGENTS.md`](../AGENTS.md) at the root has the hard rules in short, and
[`.claude/rules/`](../.claude/rules/) holds notes that apply to particular folders.

## What this fork adds over Calagopus

| Feature | Page |
|---|---|
| **Theme v2**: the look of plugged.host's Pelican panel (royal-blue accent, obsidian surfaces, Space Grotesk + JetBrains Mono, Tabler icons, flat sidebar), dark only, with an admin-chosen accent colour | [features/theme.md](features/theme.md) |
| **Release** `v{VERSION}.{commit}`, shown in the footer and by `panel-rs version` | [features/release.md](features/release.md) |
| **noprofit.space** sign-in, account sync and SFTP passwords, optional | [noprofit.md](noprofit.md) |
| **Start gates**: one place where a server start can be held back (limbo, the start queue, the ad waiting room), with the other ways to start a server closed for non-admins | [features/start-gates.md](features/start-gates.md) |
| **Node telemetry**: every node checked once a minute (answering, Wings not listening, unreachable; CPU, memory, disk, load, response time), a week of history on the Statistics tab; node role, tags, SRV target and deployment pause; location tier, sort order and public | [features/node-telemetry.md](features/node-telemetry.md) |
| **Creating servers**: players pick a game, its software, a location and a plan and get a server within their allowance (the noprofit.space plan, or the games settings); Admin › Plans, Admin › Settings › Games; `eula.txt` written for a player who accepted the EULA | [features/create-server.md](features/create-server.md) |
| **Server settings for players**: SFTP access, resize within the allowance, switch the software, delete the server; each with its own permission | [features/server-settings.md](features/server-settings.md) |
| **Placement and the deployment pause**: new servers go to the least loaded node (allocated room and real CPU/memory use), busy or silent nodes stop taking new servers and reopen by themselves; Admin › Settings › Placement | [features/placement.md](features/placement.md) |
| **Suspension details**: why, since when, by whom and until when a server is suspended, shown to its owner; servers past their deletion time are purged | [features/suspension.md](features/suspension.md) |
| **Delete a user with their servers** from the user's delete dialog | [features/admin-users.md](features/admin-users.md) |
| **Status page**: the website, the panel and every node checked each minute, incidents with updates, overrides, 90 days of uptime, served as `/data/*.json` and an RSS feed for a public status page; Admin › Status and Admin › Settings › Status Page | [features/status.md](features/status.md) |
| **Abuse protection**: every server's startup, files and console scanned every three hours, CPU watched every minute and, with the Wings fork, what runs in each container and what it connects to; critical findings suspend (then delete) or only report, Discord alerts; Admin › Abuse and Admin › Settings › Abuse | [features/abuse.md](features/abuse.md) |
| **Health checks**: background task heartbeats and node telemetry age on Admin › Overview › Health | [features/health.md](features/health.md) |
| **Removed features**: devices, mounts, the extension manager, the update checker, Tundra's private network, the server firewall, managed database instances, backup groups; telemetry is opt-in | [upstream.md](upstream.md#removed-features) |
| **German stays complete**: a build fails while a German string is missing | [development.md](development.md#strings) |

More features arrive in phases (self-service servers, storage failover, the start queue, credits and
tokens, limbo, ads, Minecraft and FiveM tools, subdomains, an AI assistant, Discord rewards). Each one
gets its page in [features/](features/) in the same commit as its code.

## Licence

MIT, like Calagopus (see [LICENSE](../LICENSE)). The Calagopus copyright stays in every file it
covers; plugged.host's changes are under the same licence.
