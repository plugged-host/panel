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

Moving the server (Move Server, [transfers.md](transfers.md)) takes it out of the line; the move
starts it on its new node.

Server activity: `server:queue.join`, `server:queue.release` (`reason`, `waited` in seconds),
`server:queue.leave`.

## The queue page

`/server/<id>/queue`, hidden from the sidebar: the place in line, how long the server waited and
the longest it will wait. With memberships on, an account without the second membership is offered
the next one (it goes ahead). With Move Server on and the `settings.transfer` permission, the page
links to it (only for members, by default). Leave the Queue takes the server out without starting
it.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/queue` | `control.start` or `control.restart` | `{queue}`: `status` (`waiting`, `released` for 2 minutes after a release, `left`, `none`), `position` (1 is next), `waited`, `remaining` (seconds), `priority`, `membership_offer`, `move_offer` |
| `DELETE /api/client/servers/{server}/queue` | `control.start` or `control.restart` | Leaves the line; 409 when the server is not waiting |

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

## Tables

`server_start_queue`: `server_uuid` (key), `user_uuid` (who pressed Start), `node_uuid` (the node
the wait is judged against), `action` (`start`/`restart`), `status` (`waiting`, `released`,
`left`), `priority`, `queued`, `last_seen`, `released`, `reason` (`room`, `timeout`, `off`).

## Check

- Admin › Settings › Hosting › Start Queue: on, Line 1 %. As a player, Start a stopped server: the
  queue page opens with "Place 1 in Line"; within about 20 seconds it does not move (the node is
  over 1 %). Set Longest Wait to 30: the server starts within 40 seconds, the page goes to the
  console, and the activity log shows `server:queue.release` with reason `timeout`.
- Start again within 10 minutes: no queue (free pass).
- Line back to 85: a start on a quiet node goes straight through.
- With two servers on the same node waiting, the one whose owner has a membership goes first.
- Leave the Queue: back on the console, the server stays off.
- As an admin, Start never queues.
