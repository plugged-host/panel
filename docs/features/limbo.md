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

The limbo page is laid out like the Pelican panel's restore page: "Server hibernated", one card
with what happened, when it was parked and (with tokens on) the restore cost and the user's balance
side by side, and Restore server in the page header behind a confirmation. When the server's own
node cannot take it, "Choose a location" lists the user's locations with room as rows (flag, name,
tier, latency) and Restore waits for one to be picked. A failed click says why in a notification
(too few tokens, no room there, or not started; no tokens are spent), and the last restore that
failed later stays on the card. While the server is being parked the card says so and the header
offers Keep server (a start, which stops the park); while it comes back the card says it is on its
way, and the page goes to the console once it is back. `/limbo` on a server that is not in limbo
goes to the console.

Restore server:

1. Costs tokens: `cost_early` (5) within `cost_window_hours` (72) of parking, `cost_late` (10)
   after. Admins and a panel with tokens off pay nothing. The tokens are spent in the same
   transaction that claims the server (`restoring`), so a restore cannot be paid twice.
2. Goes back to the server's own node, with its old address, when that node answers and has room.
   Otherwise the page asks for a location (one the owner may create in, with room): the server is
   moved there like an admin move (`Server::rehome`: new allocations, made inside the deployment
   port ranges when missing, with port variables and locks following them,
   [allocations.md](allocations.md); its backups along), and gets a new address.
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

## Deleted after a while

With **Delete After (Days)** above 0, a server `parked` for that many days (from
`server_limbo.parked`) is deleted with its files by `limbo::purge_due`, which the `settle_limbo`
task runs every minute (20 per run, also with limbo off). Its backups, the final one included, go to
the deleted servers' hold like any deleted server's ([backups page](backups-page.md)), so the owner
can still download the newest until the hold ends. The limbo page says when the server will be
deleted, and a restore is refused (409) once that time has passed. A restore under way
(`restoring`, `unpacking`) stops the clock. A server whose node cannot delete it is tried again the
next minute. The Pelican panel's `LIMBO_DELETE_AFTER_HOURS` is the same setting; the importer does
not carry settings, so set it again after an import.

## Parked on a node

The Pelican panel parked a server differently: Wings moved it whole to a node it kept for parked
servers (tagged `limbo`), and suspended it. An import ([import.md](import.md)) keeps those servers
where they are, so nothing is copied at the switch:

- The nodes it parks on become `limbo` nodes (the node role, [node-telemetry.md](node-telemetry.md)):
  the ones with its tag (`LIMBO_NODE_TAG`, `limbo` when unset), the ones its rows record
  (`limbo_node_id`) for servers parked or on their way, and any node a parked server sits on away
  from the node it came from. A location with only limbo nodes is made admins' only.
- A server it had parked there (`limbo`, `hibernating` or `restoring`, on such a node and not its
  own) is `parked` since it was parked, its limbo suspension lifted (the limbo page shows only for a
  server that is not suspended), no final backup. `server_limbo.home_node_uuid` is the node it came
  from (when that node was imported and is not a limbo node) and `home_ports` its ports there,
  primary first.
- A server it had stopped for being idle is `stopped` since then. A server limbo left on its own
  node (a park or a restore that had landed back) is an ordinary server, limbo's suspension lifted.
- A restore that had not landed stays parked; the tokens paid for it are not refunded (the report
  counts them). A move to the limbo node that had not landed leaves the server on its own node.

Restoring such a server (`limbo::way` is `Move`: its node's role is `limbo` and it has no final
backup) is a move off that node instead of an unpack:

1. The limbo node has to answer (it sends the files), else 409 "the node that keeps the server's
   files does not answer right now". The cost is the same; the tokens are spent with the claim.
2. Home is the home node's location, that node first; it is offered when that location has an
   eligible game node that answers. Otherwise the owner picks a location, as above.
3. The claim queues a transfer request with origin `limbo` (`server_transfers::limbo_request`),
   which runs like any move ([transfers.md](transfers.md)): game nodes only, next node on failure.
   On the home node the server gets its old ports back when they are all free on one address and
   it has as many allocations as it had there (`limbo::home_allocations`, the egg variables that
   held a port follow); otherwise its ports are picked as for any move. Only an admin cancels it.
4. Once the move is done the server is an ordinary server again, not started (like the Pelican
   panel's restore): its idle clock restarts, `server:limbo.restored` (`node`). The move does not
   count toward the owner's move cooldown.
5. A move that fails, is cancelled or does not start within 15 minutes parks the server again on
   the limbo node with the tokens back. **A restore never deletes anything on a limbo node**: the
   failed move leaves the files where they were (only deleting the server, by hand or after
   `delete_after_days` counted from `parked`, removes them).

An admin who moves a parked server off a limbo node (Admin › Servers › Transfer) restores it too.
`settle_limbo` never parks onto a limbo node, and never picks a server on one: new parks make a
final backup as above. Abuse scans and Lighten Load leave servers in limbo (other than `stopped`)
alone. System backup policies leave out servers in limbo too, except the ones parked on a limbo
node (they are on Wings there, as the Pelican panel's mirror kept them): a policy covering the
limbo node, its location or the server backs them up.

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
| Delete After (Days) | 0 (never) | 0 to 3650 |
| Early Restore (Tokens) | 5 | 0 to 100000 |
| Late Restore (Tokens) | 10 | 0 to 100000 |
| Early Window (Hours) | 72 | 0 to 8760 |

## Tables

- `servers.last_activity` (the idle clock), `servers.limbo_state` (`stopped`, `parking`, `parked`,
  `restoring`, `unpacking`, or `NULL`).
- `server_limbo`: one row per server that was ever in limbo: `since` (the current state's start),
  `backup_uuid` (the final backup), `parked` (set once Wings deleted the server), `paid` and
  `payer_uuid` (the restore under way), `error` and `failed` (the last failure), `updated`;
  `home_node_uuid` and `home_ports` for a server parked on a limbo node (above).

## Known limits

- A parked server still counts against its owner's allowance (it keeps its resources).
- Changing the CPU of an account (burst cores, an allowance cut) asks Wings about every server of
  it, and fails for parked ones; the change applies when they are restored.
- Servers parked by this panel keep their node (they are not on any limbo node), so deleting that
  node needs the parked servers moved or deleted first, like any other server. Only an import parks
  servers on a limbo node; this panel never adds to one.
- The limbo node needs the Wings fork like any node (it sends the files on a restore).
- An import starts every server's idle clock (`last_activity`) at the import, so a server the
  Pelican panel had stopped is parked `park_minutes` after the import at the earliest.

## Check

- Admin › Settings › Hosting › Limbo: on, Stop After 1, Park After 2. Leave a test server running
  and untouched (it needs an off-node backup configuration): after an hour it is stopped and its
  activity shows `server:limbo.stop`; after two it is parked. The server list says Parked, every
  page of the server shows the limbo page, the node's free memory went up.
- Restore server (header, confirm) with enough tokens: the tokens go down, the card says the
  server is on its way, then the page goes to the console; the files are as they were, the
  address is the old one.
- Fill the server's node (or turn its location's nodes off for deployment): the limbo page asks for
  a location, Restore stays disabled until one is picked; restoring there gives the server a new
  address.
- With too few tokens, Restore and confirm: "You need N tokens to restore this server.", nothing
  spent.
- Start a server while it is being parked: it starts, and its backup list keeps the final backup,
  unlocked.
- As an admin, restoring costs nothing; admins' own servers are never stopped.
- After an import of a Pelican panel with parked servers: Admin › Nodes shows the limbo node(s)
  with Role Limbo; a server on one shows the limbo page (not suspended). Restore it: Admin ›
  Overview › Capacity shows a move Started By "Limbo Restore", the card says it is on its way, the
  server is back on its home node with its old address, stopped; its files are as they were. Turn
  the home node off for deployment first: the page asks for a location instead. Stop Wings on the
  limbo node: Restore says the node does not answer, no tokens spent.
