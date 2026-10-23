# Node failover

When a node is down, its servers are restored on the other nodes of its location from their newest
backup on storage those nodes reach (`shared/src/node_failover.rs`). An admin starts it from the
node's **Failover** tab, or the panel starts it on its own after an outage. A drill does the same
for one server of a node that is up, to check the setup.

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
| `rehoming` | The newest successful off-node backup is picked; a node that still answers (a drill, a failover by hand) kills the server first. The best node of the location takes the server's row (`Server::rehome`): the same ports when they are free there, else others like a [transfer](transfers.md); the backups on shared storage move with it. That node's Wings creates the server without its install script. |
| `restoring` | The backup is restored there, the server's directory emptied first. The server stays stopped. |
| `done` | Restored. The server's activity gets `server:failover`. |
| `failed` | The error says why (no backup, no node with room, Wings could not restore, timeouts: 10 minutes to create, 6 hours to restore). `server:failover.failed`. A server that failed before its row moved is still on the old node. |
| `cancelled` | Cancelled before it moved, or deleted meanwhile. |

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

## The Failover tab

Admin › Nodes › a node › Failover (`nodes.read` to see, `nodes.failover` to act) lists the node's
last 10 failovers with each server's status, new node, the backup used and when the old copy was
removed. **Fail Over** starts one for every server on the node; **Drill** for one server (it is
stopped on this node first and stays on the new node afterwards); **Cancel Failover** stops a
running one: queued servers stay, servers under way finish. The page refreshes every 15 seconds
while a failover runs. Admin activity: `node:failover`, `node:failover.cancel`.

| Route | Does |
|---|---|
| `GET /api/admin/nodes/{node}/failover` | `{mode, failovers: [{uuid, mode, status, started_by, created, finished, servers: [...]}]}` |
| `POST /api/admin/nodes/{node}/failover` | `{drill_server_uuid?}` → `{failover}`; 403 when failover is off, 409 when one runs or the node has no servers |
| `POST /api/admin/nodes/{node}/failover/cancel` | `{cancelled}` (servers that will not move); 409 when none runs |

## Settings

Admin › Settings › Hosting › Recovery:

| Field | Key | Default |
|---|---|---|
| Node Failover: off, by hand, automatically and by hand | `recovery::failover_mode` | by hand |
| Automatic Failover After (Minutes) | `recovery::failover_after` | 10 (3 to 1440) |
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
