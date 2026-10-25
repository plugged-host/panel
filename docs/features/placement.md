# Placement and the deployment pause

Where a new server goes, and when a busy node stops taking new ones. Both read the node telemetry
([node-telemetry.md](node-telemetry.md)).

## Ranking

A server deployed to a location (an admin deploy, or a player's create) goes to one of the
location's nodes that is deployable (deployment enabled, not in maintenance, not paused) and has
room for its memory and disk. Upstream ranks them by how full they would be (memory and disk
allocated); this fork adds the node's real load: `pressure`, the higher of CPU and memory use
averaged over 5 minutes (`shared/src/models/node/mod.rs`, `by_location_uuids_most_eligible`).

- The order is by the fullest of allocated memory, allocated disk and pressure, the emptiest node
  first.
- With randomness, each node's chance is weighted by what is left of the same three, so a node
  whose machine is busy gets fewer servers even when little is allocated on it.
- A node without a sample in the last 5 minutes counts as idle; one that stopped answering counts as
  full (100).

Players' servers only go to nodes with the role `game`.

## The deployment pause

After every sample (the `sample_nodes` task, once a minute) the panel pauses and reopens nodes by
itself (`shared/src/models/node/placement.rs`, `pause_overloaded`). A paused node takes no new
servers; its servers keep running.

| When | Does |
|---|---|
| Memory use is over the line | Pauses, reason `memory` |
| CPU use is over the line (memory is not) | Pauses, reason `cpu` |
| The node missed `unreachable_after` samples in a row | Pauses, reason `unreachable` |
| Disk use (`disk_used` / `disk_total` of the sample) is over the disk line (memory and CPU are not) | Pauses, reason `disk` |
| A load pause, now under the line and the 5-minute average too | Reopens |
| An `unreachable` or `disk` pause, and the node answers under that line | Reopens |
| No sample in the last 3 minutes | Changes nothing |

The **line** is `pause_percent` (85), lowered by `small_offset` (10) on a small node: less memory
than `small_memory_gb` (34 GiB) or fewer threads than `small_threads` (9), so a small node is
paused at 75%. Waiting for the 5-minute average before reopening keeps a node from flapping.

The **disk guard** (`disk_percent`, 85) is the Pelican panel's storage guard (`NodeCapacity`): a
node whose disk is fuller than that takes no new servers, moves or restores, since every one of them
picks its node through the pause (creates, players' and admins' moves, Lighten Load, the start
queue's moves, failover and limbo restores; `Node::by_location_uuids_most_eligible`). It works with
"Pause Busy Nodes" off too, and 0 turns it off. A node whose sample has no disk numbers is never
paused for its disk (a `disk` pause then reopens), and an unreachable node is left to the
`unreachable` pause. Like the Pelican panel's strict mode, a location whose every node is too full
takes nothing; nothing falls back to a full node. Resizing a server picks no node, so the guard does
not apply to it.

The reason is stored in `nodes.deployment_paused_reason`. A node an admin paused (on the node form)
has no reason and is never reopened by the panel; an admin unpausing a node the panel paused clears
the reason, and the panel pauses it again on the next sample when it is still over the line. With
the pause turned off, the panel reopens the nodes it paused and leaves the rest alone. Each change is
logged (`tracing`, with the node's name).

When every node of a location is paused, a deploy there fails with "every node in the selected
location(s) is too busy for new servers right now", and the player's create page shows the location
as full.

## Settings

Admin › Settings › Hosting › Placement (`shared/src/settings/placement.rs`):

| Setting | Default | Range |
|---|---|---|
| Pause Busy Nodes | on | |
| Line (%) | 85 | 10 to 100 |
| Lower Line by (Points) | 10 | 0 to 50 |
| Memory Under (GiB) | 34 | 0 to 4096, 0 is never small |
| Threads Under | 9 | 0 to 1024, 0 is never small |
| Missed Checks | 3 | 1 to 60 |
| Disk Line (%) | 85 | 0 to 100, 0 is off; independent of "Pause Busy Nodes" |
| Deployment Port Ranges | `30000-50000,25000-30000` | Where the panel makes allocations by itself ([allocations.md](allocations.md)) |

## Without noprofit.space

Nothing changes: placement only talks to the nodes.

## Check

- Admin › Settings › Hosting › Placement: set "Line (%)" to a value under a test node's memory use
  and save. Within two minutes the node's Overview shows "Deployment Paused (memory)".
- Set it back to 85: the node reopens once its 5-minute average is under the line.
- Pause a node by hand on its form: the panel leaves it paused whatever its load.
- Stop Wings on a test node: after three missed checks it is paused (unreachable); start Wings and
  it reopens on the next sample.
- Set "Disk Line (%)" under a test node's disk use (its Overview shows the disk): within two minutes
  the node shows "Deployment Paused (disk)", and creating a server or moving one to its location
  picks another node (or says the location is full). Set it back: the node reopens on the next
  sample. With "Pause Busy Nodes" off the disk pause still happens; with "Disk Line" 0 it reopens.
