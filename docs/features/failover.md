# Node failover

When a node is down, its servers are restored on the other nodes of its location from their newest
backup on storage those nodes reach (`shared/src/node_failover.rs`). An admin starts it from the
node's **Failover** tab, or the panel starts it on its own after an outage (or, in the notify mode,
only reports the outage). A drill does the same for one server of a node that is up, to check the
setup; a preview (dry run) shows what a failover would do without doing anything.

## What it needs

- **Backups off the node.** Only a backup another node can restore counts: S3, Proxmox Backup
  Server, or restic with a repository that is not a path on the node (`sftp:backup@192.0.2.10:/srv/restic`,
  `s3:https://s3.example.com/restic`). Set it up like the copies of the
  [Backups page](backups-page.md#setting-up-the-copies): a backup configuration on such storage,
  default for the location, and a system backup policy every 30 minutes. A server without such a
  backup fails over with "no finished backup on storage other nodes reach" and stays where it is.
- **Another node in the location** with the same role that answers, has room for the server
  (memory, disk) and free ports.
- For removing the old copies afterwards, the old node runs the [Wings fork](../wings.md)
  (`POST /api/system/orphans`). On stock wings-rs the row says to remove the files by hand.

## How a server moves

A failover is a `node_failovers` row with one `server_failovers` row per server. The
`settle_failovers` task (every 15 seconds) moves each server along, **Servers Restored at Once**
at a time per failover:

| Status | What happens |
|---|---|
| `queued` | Waiting for its turn. |
| `rehoming` | The newest successful off-node backup is picked; a node that still answers (a drill, a failover by hand) kills the server first. The best node of the location takes the server's row (`Server::rehome`): the same ports when they are free there, else others like a [transfer](transfers.md) (made inside the deployment port ranges when missing, port variables and locks follow: [allocations.md](allocations.md)); the backups on shared storage move with it. That node's Wings creates the server without its install script. |
| `restoring` | The backup is restored there, the server's directory emptied first. The server stays stopped. |
| `done` | Restored. The server's activity gets `server:failover`. |
| `failed` | The error says why (no backup, no node with room, Wings could not restore, timeouts: 10 minutes to create, 6 hours to restore). `server:failover.failed`. A server that failed before its row moved is still on the old node. **Try Again** resumes it (below). |
| `cancelled` | Cancelled before it moved, or deleted meanwhile. |
| `planned` | Only in a `notify` row: where a failover would put the server and from which backup (or the error it would hit). Nothing moved. |

**Try Again** on a `failed` server (`node_failover::resume`, the Pelican panel's
`FailoverService::resume`) puts it back into its failover, which runs again until it is done:

- a server already on its new node (it failed while restoring) goes back to `rehoming`, with its
  error and its server status cleared, so its backup is restored again (the backup must still
  exist);
- a server still on the old node starts over as `queued` (a new backup and node are picked).

A deleted server, one that is on a third node now, or a failover of the node running meanwhile
(another one) refuses it with the reason (409).

Files changed after the backup are lost. A server that got other ports has a new address: its
owner sees it on the server's page as usual.

When the old node answers again, the task removes each `done` server's copy there (the Wings fork
unloads it and deletes its files and container) and fills **Old Copy Removed**. A server that was
moved back to the old node in the meantime is left alone. A node that rebooted does not start the
old copies (Wings loads its servers from the panel, which lists them on their new node); a node
that only lost its network may run them until the cleanup, with no players able to reach them.

## Automatic failover

With **Node Failover** set to automatic, the task starts a failover for a node that has been
unreachable for **Automatic Failover After** checks in a row (one a minute; a refused port does
not count, the machine is up then), if:

- no other failover is running (one at a time, the longest-gone node first);
- at least half of the other nodes answer (when many nodes look down at once, the panel's own
  network is the likelier cause);
- the node was not failed over since it last answered.

### The notify mode

**By hand, and report a down node** (`notify`, the Pelican panel's `FAILOVER_MODE=notify`) detects
a down node the same way (minutes, half of the other nodes answering) but moves nothing: once per
outage it writes a `notify` row with the status `notified` whose servers are `planned` (the
[preview](#the-preview) at that moment), logs a warning and writes `node:failover.notify` (node,
row, servers, how many could be restored) to the admin activity log
(`notify_admins_node_down`). A failover by hand elsewhere does not hold the report back, and a
report does not count as a failover: switching to automatic later still fails the node over during
the same outage. Failovers by hand, drills and Try Again work as in the manual mode.

## The Failover tab

Admin › Nodes › a node › Failover (`nodes.read` to see, `nodes.failover` to act) lists the node's
last 10 failovers with each server's status, new node, the backup used and when the old copy was
removed. **Preview** shows what a failover would do now; **Fail Over** starts one for every server
on the node; **Drill** for one server (it is stopped on this node first and stays on the new node
afterwards); **Cancel Failover** stops a running one: queued servers stay, servers under way
finish; **Try Again** under a failed server resumes it. The page refreshes every 15 seconds while a
failover runs. Admin activity: `node:failover`, `node:failover.cancel`, `node:failover.resume`.

### The preview

**Preview** (`node_failover::plan`, the Pelican panel's `failover --dry-run`, but per server) lists
each server of the node with the node a failover would put it on and the backup it would restore,
or why it would fail ("no finished backup on storage other nodes reach", "no other node of the
location answers and has room and free ports"). Nothing is changed. It picks like the failover:
another node of the location with the same role that answers, is not paused and has the memory and
disk left, the least loaded first, counting the servers placed before it (`plan_destinations`,
pure, unit-tested). The failover itself adds placement's randomness and checks the ports when it
moves a server, so it may choose another node of the same location; the preview does not check
ports. Limbo-parked servers and devboxes are left out, as by the failover.

| Route | Does |
|---|---|
| `GET /api/admin/nodes/{node}/failover` | `{mode, failovers: [{uuid, mode, status, started_by, created, finished, servers: [...]}]}` |
| `POST /api/admin/nodes/{node}/failover` | `{drill_server_uuid?}` → `{failover}`; 403 when failover is off, 409 when one runs or the node has no servers |
| `POST /api/admin/nodes/{node}/failover/cancel` | `{cancelled}` (servers that will not move); 409 when none runs |
| `GET /api/admin/nodes/{node}/failover/plan` | `nodes.read`; `?drill_server_uuid=` for one server; `{plan: [{server_uuid, server_name, destination_node_uuid, destination_node_name, backup_uuid, backup_created, error}]}`; 409 when the node has no servers |
| `POST /api/admin/nodes/{node}/failover/resume` | `nodes.failover`; `{server_failover_uuid}` → `{server}`; 403 when failover is off, 404 not this node's, 409 not failed or not resumable |

## Settings

Admin › Settings › Hosting › Recovery:

| Field | Key | Default |
|---|---|---|
| Node Failover: off, by hand, by hand and report a down node (`notify`), automatically and by hand | `recovery::failover_mode` | by hand |
| Automatic Failover After (Minutes) (also when a down node is reported) | `recovery::failover_after` | 10 (3 to 1440) |
| Servers Restored at Once | `recovery::failover_parallel` | 3 (1 to 20) |

## Without noprofit.space

Nothing differs.

## Check

- Set up an S3 or remote restic backup configuration for a location with two nodes, and a system
  backup policy; wait for a backup of a test server.
- Node › Failover › Drill on that server: it is stopped, then shows Moving, Restoring, Restored;
  the server is on the other node, its files are those of the backup, it starts.
- The old node's Wings no longer lists the server; Old Copy Removed has a time.
- Drill a server without an off-node backup: Failed, "no finished backup on storage other nodes
  reach"; the server is still on its node.
- Admin › Settings › Hosting › Recovery › Node Failover off: the buttons are disabled and the tab
  says so.
- Automatic: stop Wings' machine (not just Wings) on a test node with the setting on and the
  minutes at 3: after about 3 minutes a failover starts; when the node is back, the old copies go.
- Preview on a node with servers: each server with its new node and backup date, or the reason it
  would fail; nothing changes on the tab or the nodes.
- Notify: set Node Failover to "By hand, and report a down node", minutes 3, and stop a test
  node's machine: after about 3 minutes the tab shows a "Down, Reported" entry with every server
  "Would Move" and its planned node, Admin › Activity has `node:failover.notify`, and nothing
  moved. A second entry does not appear for the same outage.
- Try Again: make a drill fail (for example delete the backup it restores while it restores, or
  stop the new node's Wings), then press Try Again under the failed server: it goes back to
  Moving or Queued and finishes.
