# Wings

Nodes run Wings, the daemon that manages the game-server containers. This panel works with
Calagopus's [wings-rs](https://github.com/calagopus/wings); plugged.host runs a fork of it,
[plugged-host/wings](https://github.com/plugged-host/wings), which adds what some of this panel's
features need. Like the panel, the fork is edited directly in its source and merges upstream's
`main`.

## What the fork adds

| Addition | Used by | Without it |
|---|---|---|
| Nothing yet beyond plugged.host's build and service files | | |

Each addition is listed here when it lands, with the feature that needs it. Planned:
`GET /api/servers/{uuid}/processes` and `GET /api/system/load` (abuse protection, node telemetry),
and a `build.cpu_weight` server field that sets the container's CPU shares (burst cores).

Features that need the fork check what a node can do and skip a node without it, so a panel with
stock wings-rs nodes keeps working; the feature's page says what it misses then.

## The panel's side

- `api-clients/wings-api` is the generated client for Wings' API. An endpoint the fork adds is
  added there in the same style (it is regenerated from Wings' OpenAPI spec upstream; regenerate
  ours from the fork's spec, or add the method by hand next to its neighbours).
- The panel never upgrades Wings itself here: plugged.host's nodes set
  `ignore_panel_wings_upgrades: true`, so the panel cannot replace the fork with an upstream
  release.

## Running a node

Calagopus's documentation covers installing Wings and enrolling a node. A node can run wings-rs next
to Pelican's Wings when their ports, config path (`--config`), data directory and Docker network
differ; the fork's `plugged-host/README.md` shows one such layout.
