# Moving servers

Players move their own servers to another location, or to another node of the same one; admins
lighten a busy node by moving some of its servers away, and see what every location can still take.
Every move is upstream's native transfer (`Server::transfer`, Wings copies the files and backups to
the new node); the request around it lives in `server_transfers.rs` and the table
`server_transfer_requests`.

## Settings

Admin › Settings › Hosting › Games › Moving Servers (`shared/src/settings/games.rs`):

| Setting | Default | Means |
|---|---|---|
| Players Move Servers | off | The Transfer page (and the strip during a move) for owners and subusers with `settings.transfer` |
| Hours Between Moves | 24 | After a player's move finished, the next one waits this long (0 = no wait). Admins never wait |
| Nodes Tried per Move | 2 | A failed copy is tried again on the next node of the location until this many nodes were tried |
| Moves at Once per Node | 2 | How many queued moves leave one node at the same time |

## A player's move

The server's **Transfer** page (`/server/<id>/transfer`, permission `settings.transfer`, truck icon)
is laid out like the Pelican panel's (`server-transfer.blade.php`), with its wording, and has two
faces. With no move under way:

1. how the last move ended, for 10 minutes after (`OUTCOME_MINUTES`): a callout, green "Moved" with
   the new address, yellow "Move failed" with why (`reasons.*`), grey "Move cancelled";
2. the current location (flag, name) and its facts: address, data (what Wings counts on the node),
   backups (count · size) and "Offline for about" (the estimate below);
3. "Pick a location": one full-width row per location with its flag, name, tier badge (the settings'
   tier name, `tierName()`), "Current" / "Another node here" on the server's own location,
   "Locked" (a tier the owner has not unlocked) or "Full" (no node taking new servers), the latency
   and a radio. Picking the current location moves the server to another node there. When a move
   cannot start, the reason stands above the rows in yellow (`blocked`, below) and the rows are
   greyed out;
4. "Move to <location>" (filled, at the end of the row; full width on a phone) with a confirmation
   naming the location and the time offline;
5. three short notes: nothing gets lost, automatic retries, the new address;
6. the last move's Activity (its timeline), while its outcome shows.

While a move runs (the page polls every 2 seconds) a card shows From → To with the flags, the state
and its percent over a bar, the steps (Prepare, Stop, Copy, Verify, Start; backups travel inside the
copy, so they have no step of their own), the facts (data copied / in all, speed, time left, the
attempt from the second node on) and "Cancel move" (gray, small, while Wings can still drop the copy;
only an admin cancels an admin's move); under it the move's Activity. Both faces end with
"Earlier moves": the last 6 finished moves (from → to, the result, when).

`blocked` says why a move cannot start, in the Pelican panel's order: `disabled` (moving is off and
the caller is no admin), `busy`, `suspended` (admins excepted), `installing`, `restoring`,
`backupRunning` (a backup of the server started in the last day has not finished), `cooldown` (with
the time left; admins excepted). A server without an allocation moves fine here, so the Pelican
panel's `no_allocation` does not exist.

The offline estimate is the server's data plus the backups the move copies, at 40 MiB/s, plus 45
seconds for the stop, the check and the start (the Pelican panel's figure when it has no measured
moves; v3 does not record a move's bytes, so it has none).

1. `POST /api/client/servers/{server}/transfer` checks, in order: moving is on (or the caller is an
   admin), the server is not suspended, installing or restoring, no move of it is under way, no
   backup of it is being made, the cooldown is over, the location is public and in the owner's
   tiers.
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

While Wings copies, the server is off and its pages are blocked like for an admin's transfer, except
the console and the Transfer page (`ServerStateGuard.tsx`).

## Progress

Where a move is comes from the old node: `GET /api/transfers` on its Wings lists the outgoing
transfer with the bytes copied and in all (0 in all while Wings stops the server, then the files,
logs and backups). `progress_of()` turns that into the state, the step and the percent, like the
Pelican panel's bar: queued 0, preparing (or "Trying another node" from the second node on) 2,
stopping 4, copying 6 to 90 by the bytes, verifying on the new node 92, starting 97; once the
native transfer is over and the panel has not settled it yet, "Checking the copy". The speed counts
from the start of the attempt, the time left is the rest at that speed.

## The strip

Every server page shows a strip above its header while the server moves, and for 2 minutes after
(`STRIP_MINUTES`), like the Pelican panel's `TransferStrip`: a dot (accent while it runs, green when
it moved, yellow when it did not), the state and where to, a thin bar, the percent while it runs,
and "Details", which opens the Transfer page (`elements/TransferStrip.tsx`, from
`GET …/transfer/status`, polled every 3 seconds while it runs and every 15 while it shows the
outcome). It is not on the Transfer page itself, and not shown when moving is off, the user lacks
`settings.transfer` or the server is in limbo. Upstream's "transferring" banner (with Cancel) still
shows for a move the strip does not cover, like an admin's transfer while moving is off.

## Activity

Each step of a move is written to the server's activity log with the request's uuid
(`{"request": "<uuid>", ...}`), and the Transfer page reads them back as the move's timeline (the
last 40, newest first, under a first line "Move to <location> requested" or "Transfer started by an
administrator"). No table holds them, so the timeline needs no schema change.

| Event | Data | Line |
|---|---|---|
| `server:transfer.attempt` | `attempt`, `max`, `location` | "Node reserved in …", from the second node on "Attempt 2 of 3" |
| `server:transfer.attempt-failed` | `attempt`, `reason` | "Attempt 1 failed: the copy was interrupted" |
| `server:transfer.verified` | | "Verified on the new node" |
| `server:transfer.port-variable` | `name`, `port` | "SERVER_PORT set to 25565" |
| `server:transfer.complete` | `address` | "Moved, new address …" |
| `server:transfer.restart` / `.restart-failed` | `moved` | Started on the new node, or running again where it was (or not) |
| `server:transfer.cancel` | `ended` | "Cancelling", or "Move cancelled" for a queued move |
| `server:transfer.cancelled` | | "Move cancelled" |
| `server:transfer.fail` | `reason` | "Move failed, the server stayed where it was" |

Wings does not tell the panel when it stops the server or starts copying, so those lines of the
Pelican panel's timeline (stopping, stopped, killed, copying, verifying) are not there; the progress
card shows those steps.

A request's `error` is named for translation (`reason`, a key of `pages.server.transfer.reasons`)
when this module wrote it: `noNode`, `copyFailed`, `skippedMoved`; anything else shows as written.

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
| `GET /api/client/servers/{server}/transfer` | `settings.transfer` | `{transfer: {enabled, current_location_uuid, current_location, locations, cooldown_until, blocked, max_attempts, data_bytes, backups, estimate_seconds, request, progress, events, history}}`; `request` is the move under way, else the last one (with `reason`, `from_location`, `to_location`) |
| `GET /api/client/servers/{server}/transfer/status` | `settings.transfer` | `{status: {request, progress} \| null}`: the move under way, or one that ended in the last 2 minutes (the strip) |
| `POST /api/client/servers/{server}/transfer` | `settings.transfer` | Body `{location_uuid}`; 202 `{request}`; 403 when off or the tier is locked, 409 busy, backup running or no node, 429 in the cooldown |
| `POST /api/client/servers/{server}/transfer/cancel` | `settings.transfer` | 403 for an admin's move, 409 when nothing moves |
| `POST /api/admin/nodes/{node}/lighten` | `nodes.transfers` | Body `{pick: "stopped" \| "busiest", count}`; `{queued}` |
| `GET /api/admin/system/capacity` | `nodes.read` | `{locations, transfers}` |

The three client routes about a move under way skip the server's state check
(`IGNORED_STATUS_PATHS`) so that its state, the strip and Cancel stay reachable while the server is
transferring.

The server's activity log gets `server:transfer.request` (with the location), and the steps above.

## Without noprofit.space

Nothing changes. Locations with a tier stay locked for players until the economy unlocks tiers
(`server_creation::tier_unlocked`), with or without noprofit.space.

## Check

Needs two nodes of the role `game`, best in two locations.

- Admin › Settings › Hosting › Games: turn "Players Move Servers" on, "Hours Between Moves" 1.
- As a server's owner: the sidebar shows Transfer with a truck; the page has the current location
  card (address, data, backups, offline for about), one row per location (the own location says
  "Another node here" when it has another open node, else "Current"), "Pick a location" on the
  button, the three notes, and no Activity or Earlier moves yet.
- Pick the other location: the row gets the accent border and the button says "Move to <it>";
  confirm: the page shows From → To, the state with its percent and the steps, then data, speed and
  time left while it copies, and the Activity; every other server page shows the strip with
  "Details". Afterwards the server is on the other node (Admin › Servers), with the same port if it
  was free there, and runs again if it ran before.
- The Transfer page then shows the green "Moved" callout with the new address, the Activity, and
  the move under Earlier moves; the strip on other pages turns green and goes after 2 minutes.
- Transfer again right away: "This server was moved recently. You can move it again in 59m …" in
  yellow, the rows greyed out.
- Start a backup, then open Transfer: "A backup is running. Try again once it is done."
- Start a move and press Cancel move: the server stays where it was, the callout is grey "Move
  cancelled", and the strip on other pages shows it with no bar.
- Admin › Nodes › a node › Servers › Lighten Load, Stopped servers, 2: Admin › Overview › Capacity
  shows two queued or running moves, then completed.
- Lighten Load with "Running servers using the most memory" on a node with a running server: the
  server stops, moves and starts again; its owner cannot cancel it.
- Turn a destination node off during a move (or block its Wings port): the move is tried on the
  next node, then shows failed with the reason.
