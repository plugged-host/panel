# Wings

Nodes run Wings, the daemon that manages the game-server containers. This panel works with
Calagopus's [wings-rs](https://github.com/calagopus/wings); plugged.host runs a fork of it,
[plugged-host/wings](https://github.com/plugged-host/wings), which adds what some of this panel's
features need. Like the panel, the fork is edited directly in its source and merges upstream's
`main`.

## What the fork adds

| Addition | Used by | Without it |
|---|---|---|
| `GET /api/public/resources`, **without a token** and for any origin: the node's memory and disk (the filesystem holding the servers' files) as total, used, free and allocated bytes, and its CPU (model, threads, use, threads allocated). Allocated = the servers' limits added up; servers without a limit are not counted. `api.public_resources: false` in the node's config turns it off (404) | Anyone: a status page, a list of which location has room, without access to the panel | Nothing in the panel needs it |
| `GET /api/system/load`: load averages, threads, CPU use | Node telemetry | The node has no load figures |
| `GET /api/system/processes`: every container's processes (CPU, executable, command line, a deleted binary, miner strings found in the executable), each container's outgoing connections and send rate, mapped to its server; and the host's processes. Replaces the Python node agent of plugged.host's Pelican panel: the panel asks, nothing is installed on the node | Abuse protection | The node is not watched for processes and connections; the file scans still run |
| `GET /api/servers/{uuid}/processes`: the same for one server | Abuse protection (an admin's look at one server) | |
| `POST /api/system/orphans` with a server's configuration (what `GET /api/remote/servers` gives): removes that server from the node, loaded or not: its files, container and state. The panel no longer lists it on the node, so stock Wings cannot delete it | [Node failover](features/failover.md), once the old node is back | The old copy's files stay on the node; the failover row says so |
| A schedule's restart step restarts a running server only; a stopped one stays stopped ("server is not running.") | [Start gates](features/start-gates.md) | A scheduled restart could start a stopped server past the gates |
| `build.cpu_weight` in a server's configuration: the container's CPU shares (Docker's 2 to 262144, 1024 a default container's), on create and live on every sync. The panel sends 1024 per paid core | [Burst cores](features/economy.md#burst-cores): paid CPU wins over burst when the node is busy | Burst works, but every container weighs the same |
| `build.cpu_burst`: the part of `cpu_limit` that is burst; `/api/public/resources` leaves it out of the allocated CPU | Burst cores | Allocated CPU counts the burst too |

`curl https://node.example.com:8080/api/public/resources` answers:

```json
{
  "memory": {"total": 67108864000, "used": 30064771072, "free": 37044092928, "allocated": 53687091200},
  "disk": {"total": 1000204886016, "used": 412316860416, "free": 536870912000, "allocated": 644245094400},
  "cpu": {"model": "AMD Ryzen 9 5950X 16-Core Processor", "threads": 32, "used_percent": 23.4, "allocated_threads": 24.0}
}
```

Bytes throughout; disk `free` is what non-root users can still write. It is cached for 5 seconds
(`Cache-Control`).

CPU use in the process reports is measured since the previous report. When the node was not asked for
five minutes it measures for two seconds first, so the first call takes that long.

Features that need the fork check what a node can do and skip a node without it, so a panel with
stock wings-rs nodes keeps working; the feature's page says what it misses then.

## The panel's side

- `api-clients/wings-api` is the generated client for Wings' API (regenerated from upstream's
  OpenAPI spec). The fork's endpoints are hand-written in `api-clients/wings-api/src/fork.rs`, which
  the generator leaves alone, so a regeneration keeps them.
- The panel never upgrades Wings itself here: plugged.host's nodes set
  `ignore_panel_wings_upgrades: true`, so the panel cannot replace the fork with an upstream
  release.

## Running a node

Calagopus's documentation covers installing Wings and enrolling a node. A node can run wings-rs next
to Pelican's Wings when their ports, config path (`--config`), data directory and Docker network
differ; the fork's `plugged-host/README.md` shows one such layout.
