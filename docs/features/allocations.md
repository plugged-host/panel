# Allocations

How a server gets its ports, as on the Pelican panel: allocations are made on the node when none is
free, inside the deployment port ranges; FiveM and Multi Theft Auto servers get a second port in
their variable; egg variables that hold a port follow the server when it moves; and an admin can
lock an allocation so its server's users cannot remove it. The code is `shared/src/allocations.rs`
(pure parts unit-tested at its bottom).

## Deployment port ranges

Admin › Settings › Hosting › Placement › Ports (`placement::port_ranges`):

| Setting | Default | Accepts |
|---|---|---|
| Deployment Port Ranges | `30000-50000,25000-30000` | Ranges and single ports from 1024 up, comma separated (`25565, 27015-27030`); earlier ones are used first |

A saved value that no longer reads (edited by hand in the database) counts as the default. The
ranges only decide where the panel makes allocations by itself; an admin still creates any
allocation by hand on a node's Allocations tab.

## Allocations made when needed

Who asks for ports here:

| Who | Gets |
|---|---|
| A player's create ([create-server.md](create-server.md)), when the egg configuration has no deployment with a primary | One port, two in a row for an egg with a [second port](#second-port) |
| A move ([transfers.md](transfers.md)), node failover ([failover.md](failover.md)) and a limbo restore ([limbo.md](limbo.md)), when the server's own port numbers are not all free on one address and the egg configuration does not deploy | As many ports as it has, its own numbers where it can |
| A player's Add on the Network page, when the egg configuration has no allocation settings | One port |
| The Bedrock (Geyser) port ([minecraft.md](minecraft.md)) | One port, noted "Bedrock (Geyser)" |
| A devbox's SSH port ([devboxes.md](devboxes.md)) | One port |

The **address** is the one most of the node's allocations use, with their alias. A node without any
gets `0.0.0.0` with its host name as alias (its SFTP host, else its Wings URL's host), which is how
the Pelican panel binds its servers. A player's added port uses the address of the server's primary.

A port is **free** when no server has an allocation on it (on that address, or on any address that
overlaps it, since `0.0.0.0` overlaps every one), nothing listens on it on the node (Wings'
`used_ports`, the same check as upstream's deployments), and no other pick reserved it. A node that
does not answer that check gives no new ports: the create tries the next node.

| Pick | Takes |
|---|---|
| One port for a new server | A random free allocation inside the ranges, else a random port of the first range with room, made |
| Several in a row for a new server | The lowest run of free ports in a row that needs the fewest new allocations, ranges in order; else any free ones |
| A move | For each of its ports (primary first) the same number, when an allocation there is free (inside the ranges or not) or one can be made inside them; else a free allocation inside the ranges; else a new one |
| A player's added port | The first free one above the primary's port, else the first of the ranges |

Existing allocations are used before new ones are made, so a node keeps the allocations an admin
made. When the ranges are full the create says "not enough free allocations", a player's Add says
"every port this node may use is taken", and a move falls back to upstream's pick (any free
allocation of the node) before trying another node.

**Two requests at once.** The picks for one node take a lock (`nodes::{node}::allocations` in the
cache, 30 s), and a picked port is reserved for 2 minutes (`nodes::{node}::reserved_ports`) until the
server claims it. The allocation is made with `ON CONFLICT DO NOTHING` on the node's unique
address and port, and a pick that loses that race is made again (3 tries). Claiming the allocation
for the server is the unique index on `server_allocations.allocation_uuid`: a create or an added port
that loses it picks again (3 tries).

**Egg configurations first.** An egg configuration with an allocation deployment (Admin › Egg
Configurations) keeps upstream's behaviour: its ranges, its variables, its "user self assign" for a
player's Add (off refuses, on uses its range). The ranges here are for everything without one.

## Second port

A FiveM or RedM egg (the `cfx` feature) gets its second port in `TXHOST_TXA_PORT` (txAdmin), an egg
that declares `SERVER_WEBPORT` (Multi Theft Auto's resource server) in that one, like the Pelican
panel's `secondPortVariable`. The server is created with two ports in a row, and the second one goes
into the variable. An egg configuration whose deployment writes a variable itself wins.

## Ports that follow the server

When a server moves (a player's move, Lighten Load, a queued move, failover, a limbo restore) to
other port numbers, every egg variable whose name contains `PORT` and whose value is one of its old
ports gets the new port in the same place (the primary's for the primary, the second's for the
second, in the order of the Network page), like the Pelican panel's `followPorts`. An egg
configuration's own variables win. A move writes them when it is done (activity
`server:transfer.port-variable`); failover and limbo restores write them with the new node.

A transfer an admin starts by hand from Admin › Servers (upstream's form, allocations of their
choosing) keeps upstream's behaviour and changes no variable.

## Locked allocations

`server_allocations.locked`, as Pelican's `allocations.is_locked`:

- Every allocation a server is created with is locked (any create: an admin's, a player's, a
  devbox), and an allocation an admin adds on Admin › Servers › a server › Allocations is locked
  unless the request says `locked: false`. A port a player adds is not.
- A locked allocation shows a lock beside its port on the Network page and on the admin page.
  Players cannot remove it: the Remove menu item is off, a bulk removal skips it, and
  `DELETE /api/client/servers/{server}/allocations/{allocation}` answers 400 "this allocation is
  locked, only an admin can remove it". An admin with `servers.allocations` may remove it there too,
  as on the Pelican panel.
- Admin › Servers › a server › Allocations: Lock / Unlock in the row's menu
  (`PATCH /api/admin/servers/{server}/allocations/{allocation}` with `locked`, permission
  `servers.allocations`; the activity log names it).
- A move, failover or limbo restore gives the new allocations the lock and notes of the ones they
  replace, index for index.
- The API objects (`ApiServerAllocation`, the client's and the admin's) carry `locked`.
- The importer reads Pelican's `allocations.is_locked` onto the server's allocation (Pterodactyl
  has none: unlocked).

## Network addresses

The server's Network page shows, next to each allocation's host name (its alias, else its IP), the
IP address players reach it on, dimmed in brackets, as the Pelican panel's Network tab did
(`shared/src/network_address.rs`): what the shown host resolves to, else what the node's host name
(public URL, else URL) resolves to, else the allocation's IP when it is not `0.0.0.0`. Nothing is
added when that is the shown text itself. Lookups take at most 5 seconds and are remembered for 5
minutes (a failed one too). Only `GET /api/client/servers/{server}/allocations` fills the
allocation's `resolved_ip`; it is `null` everywhere else. Streamer mode hides it like the address.

## Without noprofit.space

Nothing changes: allocations only talk to the nodes and the database.

## Check

- Admin › Settings › Hosting › Placement: the Ports section shows `30000-50000,25000-30000`. Save
  `80-90`: refused (from 1024 up). Save `40000-40010` and keep it for the next checks.
- Create a server as a player on a node with no free allocation in that range (for example a test
  node whose allocations are all used): it gets a port from 40000 to 40010, and the node's
  Allocations tab lists the new allocation with the address and alias its other allocations use.
- Create a FiveM server: it has two ports in a row, and its `TXHOST_TXA_PORT` variable (Startup)
  is the second.
- On the Network page of a server whose egg has no allocation settings, click Add: a port above the
  primary appears, unlocked. The primary shows a lock with "Locked: only an admin can remove it";
  its Remove item is off; selecting it with others and removing skips it with the locked notice.
- Admin › Servers › that server › Allocations: Unlock the primary's row (toast "Allocation
  unlocked."); the lock is gone on the player's page. Lock it again.
- Move a server whose ports are taken on the destination to another location: it arrives with new
  ports, a variable that held its old port (for example `TXHOST_TXA_PORT`) holds the new one, and
  the locks and notes are as before.
- Set the ranges back to `30000-50000,25000-30000`.
- A server whose allocation has a host name alias: the Network page shows `play.example.com
  (192.0.2.10)`; one without an alias shows only its IP.
