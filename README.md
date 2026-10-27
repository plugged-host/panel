<div align="center">

# plugged.host Calagopus

[![Rust](https://img.shields.io/badge/rust-stable-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/plugged-host/panel?color=blue)](./LICENSE)
[![Release](https://img.shields.io/github/v/release/plugged-host/panel)](https://github.com/plugged-host/panel/releases)

**The game-server panel behind [plugged.host](https://plugged.host): a similar frontend, a brand new backend.**

</div>

This is plugged.host's panel since v3: a fork of [Calagopus](https://github.com/calagopus/panel), the
Rust (Tokio + Axum) and React (Mantine) reimplementation of Pterodactyl, with everything plugged.host's
Pelican panel did built into the panel's own source. It replaces the Pelican plugins
([plugged-host/plugins](https://github.com/plugged-host/plugins), deprecated). Pages look like the
Pelican panel's and load up to 88 % faster.

<div align="center">

![The dashboard](./README-assets/dashboard.gif)

</div>

## What it adds over Calagopus

- **Creating servers** from a game, its software, a location and a plan, within each account's allowance;
  placement on the least loaded node, a deployment pause for busy nodes, moves between locations.
- **Minecraft pages** (plugins and mods from Modrinth, CurseForge and SpigotMC, game settings, players,
  worlds, modpacks) and **FiveM pages** (server.cfg as a form, txAdmin recipes, players, artifacts).
- **Server settings** on one page, **custom startup commands** checked before they are applied, **server
  chat**, **subdomains**, **VS Code devboxes**.
- **Abuse protection** (files, startup, console, CPU and, with the Wings fork, processes and connections)
  and **account standing**: abuse suspensions count against the account.
- **Limbo** for idle servers, a **start queue**, **node failover** from off-node backups, **node
  telemetry**, a **status page** export.
- **Credits, tokens and memberships**, ads and sponsored links, **Discord** rewards and a partner program.
- **neo**, an assistant on the dashboard, on every server and on Discord; **rux**, a Discord moderation bot.
- **Importing a Pelican panel** with the Pelican plugins' data: `panel-rs import pelican`.
- Theme v2, 11 languages, [noprofit.space](https://noprofit.space) sign-in as an option: every feature
  works without it.

Each feature has its own page in [docs/features/](./docs/features/); the full list is in
[docs/README.md](./docs/README.md).

## Running it

- **Releases**: [GitHub releases](https://github.com/plugged-host/panel/releases) carry the panel binary
  (frontend included). A release is named by its version only (`v3.0.0`); every fix is a new version.
- **Nodes** run [plugged-host/wings](https://github.com/plugged-host/wings), which some features need
  ([docs/wings.md](./docs/wings.md)).
- **Installing** works as for Calagopus ([calagopus.com/docs](https://calagopus.com/docs/panel/installation));
  this fork's settings, optional services and branding are in [docs/forking.md](./docs/forking.md).

## Working on it

- [docs/development.md](./docs/development.md): running the panel from a fresh clone, tests, migrations.
- [docs/contributing.md](./docs/contributing.md): where code goes, naming, what a change brings along.
- [docs/upstream.md](./docs/upstream.md): merging Calagopus and every change made to its files.
- [AGENTS.md](./AGENTS.md): the short rules for AI agents.

Questions and ideas: the [plugged.host Discord](https://discord.gg/plugged-host-1502647500295635085).

## Acknowledgments

Built on [Calagopus](https://github.com/calagopus/panel) by 0x7d8 and its contributors, which builds on
[Pterodactyl](https://github.com/pterodactyl/panel) and [Pelican](https://github.com/pelican-dev/panel).

## License

MIT, like Calagopus: see [LICENSE](./LICENSE).
