# Limbo

A server nobody uses is stopped, and later parked: backed up off its node and removed from it, so
the node's disk and room go to servers people play on. Its owner restores it from the server page.
Start gate 10 ([start-gates.md](start-gates.md)), in `shared/src/limbo.rs`.

## The idle clock

`servers.last_activity` is the newest server activity a person caused: a power action, a console
command, a file change, an SFTP login, a setting. A create handler on `ServerActivity` keeps it (the
activity log itself is pruned), at most once a minute per server. The panel's own activity (no
user) does not count, so a server that only runs schedules is idle. New servers start their clock
when they are created; servers that existed before the `limbo` migration started theirs then.

## Who is touched

The `settle_limbo` task runs once a minute while limbo is on (Admin › Settings › Hosting › Limbo;
off by default) and looks at servers that are:

- not suspended, without a status (installing, restoring) and not being moved;
- not owned by an admin;
- not owned by a member, while Spare Members is on and memberships are on;
- not parked already, and not failed to park in the last day.

| Idle for | Running | Does |
|---|---|---|
| `stop_minutes` (2880, two days) | yes | Stops it (`stopped`, `server:limbo.stop`) |
| `park_minutes` (4320, three days) | no | Parks it (below) |

The windows are minutes so a test panel can park a server within the hour; until 2026-10-24 they
were hours (`stop_hours`, `park_hours`), and a stored hour value still counts (times 60) until the
tab is saved again.

A node that does not answer is skipped: it says nothing about its servers. At most `max_per_run`
servers are stopped per run, and at most `max_per_run` parks run at once. Any activity by a person
clears `stopped`; a stopped server is otherwise an ordinary server.

## Parking

1. The final backup is made (`parking`, `server:limbo.parking`) with the server's backup
   configuration, and locked. It has to leave the node: S3, or restic with a repository that is
   not on the node (`node_failover::off_node`, the same rule as failover). A server whose backups
   stay on its node, or whose configuration is in maintenance, is never parked.
2. Once the backup is done, the server is claimed (`parked`) and Wings deletes it (a 404 counts as
   deleted). Its row, allocations, subusers, databases and backups stay; the node's disk is freed,
   and the node's room (`server_usage`, placement, transfers, resizes) no longer counts it. Node
   failover leaves it alone: there is nothing of it on the node. `server:limbo.park`.
3. A park that cannot finish is cancelled and the server stays where it is: the backup was deleted
   or failed, or did not finish within 6 hours. The error is kept (`server_limbo.error`) and the
   server is tried again after a day. After 3 failed parks in an hour and none that worked, parking
   waits for the hour to pass.

Starting the server while it is `parking` cancels the park: the server stays, the backup becomes an
ordinary, unlocked backup. Once the park has claimed the server to delete it, a start is held like
any start of a parked server.

## Restoring

A start of a server in limbo (`parking` past its claim, `parked`, `restoring`, `unpacking`) is held:
the power API answers `202 {"gate": "limbo", "url": "/server/<id>/limbo"}`. Every page of the
server shows the limbo page instead, the websocket is not opened, and the server's API answers 409
"server is parked" except for the server itself, its limbo, power and activity routes.

On the limbo page, Restore Server:

1. Costs tokens: `cost_early` (5) within `cost_window_hours` (72) of parking, `cost_late` (10)
   after. Admins and a panel with tokens off pay nothing. The tokens are spent in the same
   transaction that claims the server (`restoring`), so a restore cannot be paid twice.
2. Goes back to the server's own node, with its old address, when that node answers and has room.
   Otherwise the page asks for a location (one the owner may create in, with room): the server is
   moved there like an admin move (`Server::rehome`: new allocations, its backups along), and gets
   a new address.
3. Wings creates the server without installing it (`restoring`), then the final backup is restored
   into it (`unpacking`). When the restore is done the server is an ordinary server again: its idle
   clock restarts, the backup is unlocked, `server:limbo.restored`.

A restore that fails (Wings does not create the server within 15 minutes, or the backup does not
restore) deletes the half-made copy, parks the server again, gives the tokens back
(`limbo_refund`) and shows the error on the page (`server:limbo.restore-failed`).

Server activity: `server:limbo.stop`, `server:limbo.parking`, `server:limbo.park`,
`server:limbo.restore` (`node`, `cost`), `server:limbo.restored`, `server:limbo.restore-failed`.
Token history: "Server restored" (`limbo`), "Restore refunded" (`limbo_refund`).

The server list shows Being Parked, Parked and Being Restored instead of the usage meters.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/limbo` | `control.start` or `control.restart` | `{limbo}`: `state` (`stopped`, `parking`, `parked`, `restoring`, `unpacking` or `null`), `last_activity`, `parked`, `cost`, `tokens` (the user's, `null` with tokens off), `home_available`, `home_location_uuid`, `locations` (as on Create Server), `error` |
| `POST /api/client/servers/{server}/limbo` | `control.start` or `control.restart` | `{location_uuid?}`: restores the server, home without a location; 409 when it is not parked, its node has no room and no location was picked, the location cannot take it, or the user has too few tokens |

## Settings

Admin › Settings › Hosting › Limbo (`shared/src/settings/limbo.rs`):

| Setting | Default | Range |
|---|---|---|
| Park Idle Servers | off | |
| Stop After (Hours) | 48 | 1 to 8760 |
| Park After (Hours) | 72 | 1 to 8760 |
| Servers per Minute | 3 | 1 to 100 |
| Spare Members | on | |
| Early Restore (Tokens) | 5 | 0 to 100000 |
| Late Restore (Tokens) | 10 | 0 to 100000 |
| Early Window (Hours) | 72 | 0 to 8760 |

## Tables

- `servers.last_activity` (the idle clock), `servers.limbo_state` (`stopped`, `parking`, `parked`,
  `restoring`, `unpacking`, or `NULL`).
- `server_limbo`: one row per server that was ever in limbo: `since` (the current state's start),
  `backup_uuid` (the final backup), `parked` (set once Wings deleted the server), `paid` and
  `payer_uuid` (the restore under way), `error` and `failed` (the last failure), `updated`.

## Known limits

- A parked server still counts against its owner's allowance (it keeps its resources).
- Changing the CPU of an account (burst cores, an allowance cut) asks Wings about every server of
  it, and fails for parked ones; the change applies when they are restored.
- There is no separate limbo node: a parked server keeps its node, so deleting that node needs the
  parked servers moved or deleted first, like any other server.

## Check

- Admin › Settings › Hosting › Limbo: on, Stop After 1, Park After 2. Leave a test server running
  and untouched (it needs an off-node backup configuration): after an hour it is stopped and its
  activity shows `server:limbo.stop`; after two it is parked. The server list says Parked, every
  page of the server shows the limbo page, the node's free memory went up.
- Restore Server with enough tokens: the tokens go down, the page shows the progress, then goes to
  the console; the files are as they were, the address is the old one.
- Fill the server's node (or turn its location's nodes off for deployment): the limbo page asks for
  a location; restoring there gives the server a new address.
- With too few tokens the Restore button is disabled and Earn Tokens links to the tokens page.
- Start a server while it is being parked: it starts, and its backup list keeps the final backup,
  unlocked.
- As an admin, restoring costs nothing; admins' own servers are never stopped.
