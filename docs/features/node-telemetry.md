# Node telemetry and placement fields

The panel checks every node once a minute and keeps what it saw. One sampler serves everything that
needs to know how a node is doing: the node's Statistics tab, the status page, placement of new
servers, the deployment pause and node failover. Nodes and locations also carry a few fields that
placement reads.

## The sample

The `sample_nodes` task (every minute, at second 30, on the primary instance) asks each node's Wings,
16 at a time, with 5 seconds to answer (`shared/src/node_telemetry.rs`):

| Asked | Gives |
|---|---|
| `GET /api/system/stats` | CPU use (all threads, 0 to 100), threads, memory and disk used/total; the time it took is the response time |
| `GET /api/servers/utilization` | how many servers are running (starting and running count, stopping too) |
| `GET /api/system/load` (Wings fork only) | the 1-minute load average; a stock Wings node answers 404 and the load stays empty |

When Wings does not answer, a TCP connect to its port tells the two failures apart:

- **Wings not listening** (`refused`): the machine is up but nothing listens on the port. Wings is
  stopped, crashed or still starting.
- **Unreachable**: no answer at all. The machine or its network is gone.

Node failover only acts on the second kind (`consecutive_outages`); a refused port breaks a run of
outages.

## Tables

| Table | Holds |
|---|---|
| `node_telemetry` | One row per node: the newest sample, `pressure`, `consecutive_failures` (samples in a row without an answer), `consecutive_outages` (samples in a row with the machine gone), `error`, `last_reachable` |
| `node_telemetry_samples` | Every sample for 7 days (`RETENTION_DAYS`); `prune_node_telemetry` deletes older ones daily at 04:15 |

`pressure` is the higher of CPU and memory use in percent, averaged over the answered samples of the
last 5 minutes (`PRESSURE_MINUTES`). It is the number the deployment pause and placement compare
against, so one busy minute does not move servers around.

Both tables go when their node is deleted.

## Node and location fields

| Field | Edited in | Means |
|---|---|---|
| `nodes.role` | Node form | `game` takes players' servers; `internal` is for admins only (tests, builds, a node being set up) and is never picked for players |
| `nodes.tags` | Node form | Short labels (`nvme`, `ddos-protected`), stored lower case, sorted, without duplicates; at most 32 of 1 to 64 letters, digits, `-`, `_`, `.` |
| `nodes.srv_target` | Node form | The host name server subdomains' SRV records point to; empty uses the node's address |
| `nodes.deployment_paused` | Node form | A paused node takes no new servers. Unlike the native "deployment enabled", the panel sets and clears it itself from `pressure` once placement is ported (Phase 2); an admin can set it by hand |
| `locations.tier` | Location form | The plan tier a player needs to put servers there (`plus`, `premium`); empty for every player |
| `locations.sort` | Location form | Order in the location picker, lowest first |
| `locations.public` | Location form | Players can pick it; admins can always use every location |

The fields are part of the native node and location models (`shared/src/models/node/mod.rs`,
`shared/src/models/node/placement.rs`, `shared/src/models/location.rs`), so the admin API, the
`nodes create` command, import and duplicate carry them too.

## Allocations

New allocations default to the IP `0.0.0.0` (every address of the node) with the node's name as
their alias: the SRV target, else the host of the public URL, else of the URL
(`frontend/src/lib/domain/allocation.ts`), in the "Create allocations" dialog and when a paired
node's allocations are made. Players see the alias, which keeps working when the node's address
changes. Both stay editable.

## Routes and permissions

| Route | Permission | Gives |
|---|---|---|
| `GET /api/admin/nodes/{node}/telemetry?hours=24` | `nodes.read` | `{latest, bucket_minutes, history}`: the newest sample and the history of the last `hours` (1 to 168), averaged into at most 360 points |

The node's **Statistics** tab shows it under the live graphs: the state now (answering, Wings not
listening, unreachable), response time, pressure, load, running servers, and charts of CPU, memory,
disk, response time and running servers for 1 hour, 24 hours or 7 days. It refreshes every minute.
The node's **Overview** shows the role, the tags, the SRV target and a "Deployment Paused" badge.

## Without noprofit.space

Nothing changes: telemetry only talks to the nodes.

## Check

- Admin › Nodes › a node › Statistics: "Health and History" shows "Answering" and fills a point per
  minute; switching 1 hour / 24 hours / 7 days redraws the charts.
- Stop Wings on a test node: within two minutes the card says "Wings not listening" and "Missed
  checks in a row" counts up; start it again and it says "Answering".
- Edit a node: role, tags (type `NVMe`, it is saved as `nvme`), SRV target and "Deployment Paused"
  save and show on the Overview.
- Edit a location: tier, sort order and public save.
