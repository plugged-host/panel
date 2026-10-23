# Moving servers

Players move their own servers to another location, or to another node of the same one; admins
lighten a busy node by moving some of its servers away, and see what every location can still take.
Every move is upstream's native transfer (`Server::transfer`, Wings copies the files and backups to
the new node); the request around it lives in `server_transfers.rs` and the table
`server_transfer_requests`.

## Settings

Admin › Settings › Games › Moving Servers (`shared/src/settings/games.rs`):

| Setting | Default | Means |
|---|---|---|
| Players Move Servers | off | The Move Server page for owners and subusers with `settings.transfer` |
| Hours Between Moves | 24 | After a player's move finished, the next one waits this long (0 = no wait). Admins never wait |
| Nodes Tried per Move | 2 | A failed copy is tried again on the next node of the location until this many nodes were tried |
| Moves at Once per Node | 2 | How many queued moves leave one node at the same time |

## A player's move

The server's **Move Server** page (`/server/<id>/transfer`, permission `settings.transfer`) lists the
locations like the create page: a location with a tier the owner has not unlocked is locked, one without a node taking new servers is "Full right now". Picking the current
location moves the server to another node there.

1. `POST /api/client/servers/{server}/transfer` checks, in order: moving is on (or the caller is an
   admin), the server is not suspended, installing or restoring, no move of it is under way, the
   cooldown is over, the location is public and in the owner's tiers.
2. Whether the server is running is read from Wings (`restart`).
3. A row is written and the first attempt starts right away; when no node can take the server the
   row is closed as failed and the answer says why (the deployment blocker: "every node is full",
   "paused", ...).

An attempt picks the most eligible node of the location ([placement.md](placement.md)) that is a
game node, is not the server's node and was not tried before, then:

- **Ports**: the server keeps its port numbers when the new node has all of them free on one
  address; else it gets what the egg configuration deploys (the ports also go into the egg
  variables the configuration names, once the move is done); else any free ports.
- **Backups**: the server's own backups move with it and are deleted on the old node; shared
  backups stay.
- The copy is `tar.zst`.

While Wings copies, the server is off and its pages are blocked like for an admin's transfer; the
console shows upstream's progress bar, now with a Cancel button for the player too.

## Settling

The `settle_transfers` task (every 15 seconds, `backend/src/tasks.rs`) looks at running moves whose
native transfer is over (the server's `destination_node` is cleared):

- the server is on the new node: the move is **completed**, the ports are written to the variables,
  and a server that ran before is started again;
- it is still on the old node and the move was cancelled: **cancelled**;
- else the copy failed: the next node is tried while fewer than "Nodes Tried per Move" were tried,
  then **failed**. A failed or cancelled server that ran before is started again where it is.

A suspended server is never started again.

Then it starts queued moves (from Lighten Load) while their source node has fewer than "Moves at
Once per Node" running. A queued server that moved or changed meanwhile is closed as cancelled.

## Cancelling

`POST /api/client/servers/{server}/transfer/cancel`: a queued move is cancelled at once; a running
one is cancelled through Wings (`DELETE /api/servers/{server}/transfer` on the old node) and settles
as cancelled when Wings has stopped. A player cannot cancel a move an admin started (a Lighten Load
move, or a transfer from Admin › Servers): "an admin is moving this server". Admin › Servers'
own Cancel marks the move as cancelled too, so it is not tried on another node.

## Lighten Load

Admin › Nodes › a node › Servers › **Lighten Load** (`POST /api/admin/nodes/{node}/lighten`,
permission `nodes.transfers`): queues moves of up to 100 of the node's servers to the other nodes of
its location, with the same node role.

| Servers | Picks |
|---|---|
| Stopped servers | Servers Wings reports as off; nobody is disconnected and they stay off |
| Running servers using the most memory | Running servers, biggest memory use first; each is stopped, moved and started again |

Servers that are moving, installing or restoring are left out. The admin activity log gets
`node:lighten`.

## Capacity

Admin › Overview › **Capacity** (`GET /api/admin/system/capacity`, permission `nodes.read`):

- per location: open nodes (deployment on, no maintenance, not paused) of all its nodes, free
  memory and disk (limit minus what its servers were given, over nodes with a limit), and the
  average 5-minute load of its answering nodes ([node-telemetry.md](node-telemetry.md));
- the moves under way first, then the last finished ones (50 in all): server, from and to, status
  (and the error under it), how many nodes were tried, who started it.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/transfer` | `settings.transfer` | `{transfer: {enabled, current_location_uuid, locations, cooldown_until, request}}`; `request` is the move under way, else the last one |
| `POST /api/client/servers/{server}/transfer` | `settings.transfer` | Body `{location_uuid}`; 202 `{request}`; 403 when off or the tier is locked, 409 busy or no node, 429 in the cooldown |
| `POST /api/client/servers/{server}/transfer/cancel` | `settings.transfer` | 403 for an admin's move, 409 when nothing moves |
| `POST /api/admin/nodes/{node}/lighten` | `nodes.transfers` | Body `{pick: "stopped" \| "busiest", count}`; `{queued}` |
| `GET /api/admin/system/capacity` | `nodes.read` | `{locations, transfers}` |

The two client routes about a move under way skip the server's state check (`IGNORED_STATUS_PATHS`)
so that its state and its Cancel stay reachable while the server is transferring.

The server's activity log gets `server:transfer.request` (with the location) and
`server:transfer.cancel`.

## Without noprofit.space

Nothing changes. Locations with a tier stay locked for players until the economy unlocks tiers
(`server_creation::tier_unlocked`), with or without noprofit.space.

## Check

Needs two nodes of the role `game`, best in two locations.

- Admin › Settings › Games: turn "Players Move Servers" on, "Hours Between Moves" 1.
- As a server's owner: the sidebar shows Move Server; the current location says "Current location".
- Pick the other location, Move Server, confirm: the console shows the transfer bar with Cancel;
  afterwards the server is on the other node (Admin › Servers), with the same port if it was free
  there, and runs again if it ran before.
- Move Server again right away: the locations are greyed out and "Next move possible: in 1 hour".
- Start a move and press Cancel in the console bar: the server stays where it was and the page says
  "The move was cancelled."
- Admin › Nodes › a node › Servers › Lighten Load, Stopped servers, 2: Admin › Overview › Capacity
  shows two queued or running moves, then completed.
- Lighten Load with "Running servers using the most memory" on a node with a running server: the
  server stops, moves and starts again; its owner cannot cancel it.
- Turn a destination node off during a move (or block its Wings port): the move is tried on the
  next node, then shows failed with the reason.
