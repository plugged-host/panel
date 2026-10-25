# Start queue

When a node is busy, starts on it take turns instead of piling more load onto it at once. Start
gate 20 ([start-gates.md](start-gates.md)), in `shared/src/start_queue.rs`.

## Who waits

A start, and a restart unless that is turned off, waits when all of these hold:

- the queue is on (Admin › Settings › Hosting › Start Queue; off by default);
- the user is not an admin;
- the server is not suspended and has no status (installing, restoring, moving);
- the server's start was not let go less than `pass_minutes` ago (the free pass);
- others already wait on the server's node, or its newest sample (from the last 5 minutes) has
  memory or CPU use over the line.

The **line** is `line_percent` (85), lowered on a small node by the placement settings' offset, like
the deployment pause ([placement.md](placement.md)). A node without a fresh sample, or one that
does not answer, holds nobody back on the click.

The power API then answers `202 {"gate": "queue", "url": "/server/<id>/queue"}` and the browser
goes to the queue page. Pressing Start again while waiting keeps the place (a restart replaces the
start); there is one row per server in `server_start_queue`, reused for every wait.

## The line

Members go first: the second membership (priority 2), then the first (1), then everyone else, each
by arrival. The priority is the account's when it joins, and rises when a membership is bought while
waiting (the page's next poll). With memberships off everyone has 0.

## Release

The `release_start_queue` task runs every 10 seconds:

| When | Does |
|---|---|
| The queue was turned off | Lets every waiting start go (reason `off`) |
| A start waited `max_wait` seconds | Lets it go whatever the node looks like (reason `timeout`) |
| The node let a start go less than `release_interval` seconds ago | Nothing |
| The node, asked right then, is under the line | Lets the front of its line go (reason `room`) |

Letting a start go claims the row with a conditional update, then sends the power action to Wings
directly, past the other gates. A server that got suspended or busy while waiting is not started.
The queue page polls every 5 seconds, sees the release and goes to the console.

Moving the server (Transfer, [transfers.md](transfers.md)) takes it out of the line; the move
starts it on its new node.

Server activity: `server:queue.join`, `server:queue.release` (`reason`, `waited` in seconds),
`server:queue.leave`.

## The queue page

`/server/<id>/queue`, hidden from the sidebar, laid out like the Pelican panel's queue page. While
it is open the server's sidebar is not shown (one screen, one purpose; `ServerRouter.tsx`).
Top to bottom:

- **Status card**: "Getting your server ready" with what is held (a start, or a restart that keeps
  the server running until then), the place in line ("You are next" at the front, "Priority" under
  it for members), a bar to `max_wait`, "Waiting m:ss" and "Starts within N seconds at the
  latest". The clock moves every second in the browser and is re-seeded from the panel on every
  poll (5 seconds); reaching `max_wait` asks the panel at once.
- **Why**: while the node has a sample from the last 5 minutes, which of memory or CPU is over the
  line (or "The node has room again"), with both readings as meters, red over the line.
- The first two units of the room's ads ([ads.md](ads.md#waiting-room)).
- **Membership**: members see a receipt ("You have … Plus"); others, with memberships on, the
  first membership's offer and its price, which leads to the membership page. The second
  membership is never offered here.
- **Move to a quieter node**: with "Offer a Move" and players' moves on, to whoever has
  `settings.transfer` (only to members by default), for a server whose disk limit is under "Largest
  Server" and that is not cooling down from a move. It names the quietest other game node of the
  server's location that takes new servers, has room for it and is under its line now (lowest of
  memory and CPU, `move_target`). Taking it, after a confirmation, re-checks all of this, moves the
  server there (that node first, then the location's others, `server_transfers::request_preferring`)
  and opens the Transfer page; the move takes the server out of the line and starts it on its new
  node. A server too big for the move gets a line saying waiting is faster.
- The rest of the ads, then "Cancel and go back" (leaves the line, back to the console) and how
  long the pass after a release lasts.

A release (seen by the poll) goes to the console; a page with nothing to wait for (`left`, `none`)
goes there at once. Leaving is a full page load when the room's page-level ad scripts ran.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/queue` | `control.start` or `control.restart` | `{queue}`: `status` (`waiting`, `released` for 2 minutes after a release, `left`, `none`), `action` (`start`, `restart`), `position` (1 is next), `waited`, `remaining`, `max_wait` (seconds), `priority`, `memory_percent`, `cpu_percent` (the node's fresh reading, else `null`), `line`, `reason` (`memory`, `cpu` or `null`), `membership_offer`, `plus_cost`, `pass_minutes`, `move_node` (the node the move goes to, `null` without an offer), `move_too_big` |
| `DELETE /api/client/servers/{server}/queue` | `control.start` or `control.restart` | Leaves the line; 409 when the server is not waiting |
| `POST /api/client/servers/{server}/queue/transfer` | `control.start` or `control.restart`, and `settings.transfer` | Takes the move: `202 {request}` like the Transfer page's move (`server:transfer.request` with `from_queue`); 409 when the server is not waiting or no quieter node can take it, else the move's own errors |

## Settings

Admin › Settings › Hosting › Start Queue (`shared/src/settings/queue.rs`):

| Setting | Default | Range |
|---|---|---|
| Queue Starts on Busy Nodes | off | |
| Queue Restarts | on | |
| Line (%) | 85 | 1 to 100 |
| Longest Wait (Seconds) | 600 | 30 to 86400 |
| Seconds Between Starts | 20 | 0 to 3600 |
| Free Pass (Minutes) | 10 | 0 to 1440 |
| Offer a Move | on | |
| Only for Members | on | |
| Largest Server (MiB) | 3072 | 0 (any size) to 1048576 |

## Tables

`server_start_queue`: `server_uuid` (key), `user_uuid` (who pressed Start), `node_uuid` (the node
the wait is judged against), `action` (`start`/`restart`), `status` (`waiting`, `released`,
`left`), `priority`, `queued`, `last_seen`, `released`, `reason` (`room`, `timeout`, `off`).

## Check

- Admin › Settings › Hosting › Start Queue: on, Line 1 %. As a player, Start a stopped server: the
  queue page opens with "You are next"; within about 20 seconds it does not move (the node is
  over 1 %). Set Longest Wait to 30: the server starts within 40 seconds, the page goes to the
  console, and the activity log shows `server:queue.release` with reason `timeout`.
- Start again within 10 minutes: no queue (free pass).
- Line back to 85: a start on a quiet node goes straight through.
- With two servers on the same node waiting, the one whose owner has a membership goes first.
- Cancel and go back: back on the console, the server stays off.
- The queue page shows no sidebar; the status card's clock moves every second; with the node
  over the line, the reason names memory or CPU and its meter is red.
- As a member, with another node of the location under the line: the page offers "Move to a
  quieter node" naming it; Move the server (confirm) opens the Transfer page and the server moves
  there. A server with a disk limit of 3072 MiB or more gets the "too large" line instead.
- As an admin, Start never queues.
