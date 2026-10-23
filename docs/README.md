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
| **Removed features**: devices, mounts, the extension manager, the update checker, Tundra's private network, the server firewall, managed database instances, backup groups; telemetry is opt-in | [upstream.md](upstream.md#removed-features) |
| **German stays complete**: a build fails while a German string is missing | [development.md](development.md#strings) |

More features arrive in phases (self-service servers, storage failover, the start queue, credits and
tokens, limbo, ads, Minecraft and FiveM tools, subdomains, an AI assistant, Discord rewards). Each one
gets its page in [features/](features/) in the same commit as its code.

## Licence

MIT, like Calagopus (see [LICENSE](../LICENSE)). The Calagopus copyright stays in every file it
covers; plugged.host's changes are under the same licence.
