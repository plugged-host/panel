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
| **Creating servers**: players pick a game, its software, a location and a plan and get a server within their allowance (the noprofit.space plan, or the games settings); Admin › Plans, Admin › Settings › Hosting › Games; `eula.txt` written for a player who accepted the EULA | [features/create-server.md](features/create-server.md) |
| **Server settings for players**: SFTP access, resize within the allowance, switch the software, delete the server; each with its own permission | [features/server-settings.md](features/server-settings.md) |
| **Placement and the deployment pause**: new servers go to the least loaded node (allocated room and real CPU/memory use), busy or silent nodes stop taking new servers and reopen by themselves; Admin › Settings › Hosting › Placement | [features/placement.md](features/placement.md) |
| **Moving servers**: players move their server to another location (cooldown, retried on another node, cancel); admins lighten a busy node and see the free resources per location (Admin › Overview › Capacity) | [features/transfers.md](features/transfers.md) |
| **Allowance cuts**: when an account's plan or the games allowance gets smaller, its servers give back what is over, evenly, never below a floor | [features/allowance-cuts.md](features/allowance-cuts.md) |
| **Backups page and deleted servers**: the newest backup of each server for download, and a deleted server's system backups kept for its owner for 7 days | [features/backups-page.md](features/backups-page.md) |
| **Welcome tour**: a guide drawn over the server list and the first console (off by default, versioned, replay from the account menu) | [features/welcome-tour.md](features/welcome-tour.md) |
| **Node failover**: a down node's servers restored on the other nodes of its location from their newest off-node backup, by hand, automatically or as a drill; old copies removed when the node is back | [features/failover.md](features/failover.md) |
| **Credits, tokens and memberships**: the Resource Store (CPU, memory and disk for credits, on noprofit.space's curve), tokens from sponsored links (Linkvertise, Vylo), two memberships that open location tiers and add CPU, admin view per user | [features/economy.md](features/economy.md) |
| **Limbo**: servers nobody used for 48 hours are stopped, after 72 hours backed up off their node and removed from it; the owner restores them from the server page for a few tokens, home or to a location with room; Admin › Settings › Hosting › Limbo | [features/limbo.md](features/limbo.md) |
| **Ads**: Adsterra units in sandboxed frames that fold when empty, Monetag's tags, the owner's own sponsored banners (Admin › Sponsored Banners), an ad-funded waiting room on start, a note for ad blockers and a chat line on Minecraft servers; the second membership sees none; Admin › Settings › Ads | [features/ads.md](features/ads.md) |
| **Start queue**: starts on a busy node take turns, members first, at most one per node every 20 seconds and none waiting longer than 10 minutes; the queue page offers a membership or a move; Admin › Settings › Hosting › Start Queue | [features/start-queue.md](features/start-queue.md) |
| **Suspension details**: why, since when, by whom and until when a server is suspended, shown to its owner; servers past their deletion time are purged | [features/suspension.md](features/suspension.md) |
| **Delete a user with their servers** from the user's delete dialog | [features/admin-users.md](features/admin-users.md) |
| **Status page**: the website, the panel and every node checked each minute, incidents with updates, overrides, 90 days of uptime, served as `/data/*.json` and an RSS feed for a public status page; Admin › Status and Admin › Settings › General › Status Page | [features/status.md](features/status.md) |
| **Abuse protection**: every server's startup, files and console scanned every three hours, CPU watched every minute and, with the Wings fork, what runs in each container and what it connects to; critical findings suspend (then delete) or only report, Discord alerts; Admin › Abuse and Admin › Settings › Security › Abuse | [features/abuse.md](features/abuse.md) |
| **Health checks**: background task heartbeats and node telemetry age on Admin › Overview › Health | [features/health.md](features/health.md) |
| **Minecraft pages** on servers whose egg has the `eula` feature: Plugins & Mods (what is installed, identified by hash or by the jar's descriptor, with updates, missing dependencies, turn off and delete; search and install from Modrinth, CurseForge and SpigotMC), Game Settings (`server.properties` and the Bukkit, Spigot, Paper and Purpur files, comments kept), Players (whitelist, operators, bans, live by console or in the files), Worlds (download, load another, new world with its generation settings, upload a world archive, delete to the trash), Modpacks (Modrinth `.mrpack` and CurseForge, switching loader and version), Bedrock players (Geyser + Floodgate on their own port), the setup a player's new server starts with (software or modpack, Bedrock, accounts, whitelist, operators) and the Minecraft egg, the mclo.gs log check above the console (secrets masked) and the Minecraft version in Settings › Software; Admin › Settings › Hosting › Minecraft | [features/minecraft.md](features/minecraft.md) |
| **FiveM pages** on servers whose egg has the `cfx` feature: Server Configuration (`server.cfg` as a form, only changed lines rewritten, with your databases, keys, resources, group memberships and access control), Frameworks (txAdmin's recipes installed in one click into a new folder, with a new or existing database), Players (live from FXServer), Artifacts (the FXServer build: follow Recommended or Latest, stay, or install a build, with Cfx's support dates; FiveM for GTAV Enhanced), the FiveM egg; Admin › Settings › Hosting › FiveM | [features/fivem.md](features/fivem.md) |
| **Subdomains**: players name their server (`play.example.com`) under the panel's Cloudflare domains, with SRV records for Minecraft, FiveM and other games; a token deposit per subdomain, refunded on deletion; the records follow the server across moves, failovers and limbo restores; Admin › Domains, Admin › Settings › Hosting › Subdomains, a per-server limit | [features/subdomains.md](features/subdomains.md) |
| **VS Code devboxes**: a small Linux box next to a game server with its files at `/server`, opened in VS Code over SSH with a connect code or in the browser through an edge; personal or one shared per server, `ph` for the game server's state, console and power within what the person may do; stopped when idle, deleted when unused; abuse findings block the payer; Admin › Settings › Hosting › Devboxes | [features/devboxes.md](features/devboxes.md) |
| **neo**: the assistant in a chat on the dashboard and on every server, which reads the console and files and acts with the person's own permissions (changes wait for approval unless they allow otherwise), answers streamed while written, images, history, per-person switches, models and limits from provider slots or noprofit.space; Admin › Settings › neo | [features/neo.md](features/neo.md) |
| **Discord**: players link their Discord account; being in the server adds a tier to their allowance, each boost level pays credits once, friends invited who stay pay credits (held, checked against alts, taken back when they leave); a bot answers `/invites`, `/topinvites` and `/setboost`; subdomains for linked members only, if wanted; one gateway on the primary instance for the bots, off by default; Admin › Settings › Accounts › Discord | [features/discord.md](features/discord.md) |
| **rux**: the moderation bot on Discord: admins (the panel's, or noprofit.space's) have it act in plain words by DM, @mention or "hey rux", members of its moderator roles have it moderate their server; on up to ten servers, on neo's AI providers (premium models first for orders, never for its message checks), read-only while noprofit.space keeps its settings; the Discord section's moderation bot, off by default; Admin › Settings › Accounts › rux | [features/rux.md](features/rux.md) |
| **neo on Discord**: account support on the Discord server for anyone, linked or not (DMs, mentions, replies, optionally its name), with the linked account's neo limits or a share of them, read-only account lookups in DMs, screenshots, replies split for Discord that never ping; Admin › Settings › AI › neo on Discord | [features/neo-discord.md](features/neo-discord.md) |
| **Removed features**: devices, mounts, the extension manager, the update checker, Tundra's private network, the server firewall, managed database instances, backup groups; telemetry is opt-in | [upstream.md](upstream.md#removed-features) |
| **German stays complete**: a build fails while a German string is missing | [development.md](development.md#strings) |

More features arrive in phases (an AI assistant). Each one gets its page in
[features/](features/) in the same commit as its code.

## Licence

MIT, like Calagopus (see [LICENSE](../LICENSE)). The Calagopus copyright stays in every file it
covers; plugged.host's changes are under the same licence.
