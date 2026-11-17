# Dedicated plans (paid)

A dedicated plan is a paid server size whose **CPU threads and memory are the server's alone**. Disk
is shared like on every node. plugged.host calls them "plugged.host Paid". The name players see is
Admin › Settings › Hosting › Games › Dedicated Plans Name (`games::dedicated_name`, default
"Dedicated").

Only admins make dedicated plans (Admin › Plans), and only admins create their servers (Admin ›
Servers › Create, with the plan). Players never type a size or create one themselves. A public
dedicated plan is shown with its price on the create page and ordered with a support ticket. A
non-public one is a custom plan an admin uses for one customer. The panel charges nothing: the price
is shown, billing happens elsewhere.

## Plans

`plans` gets `dedicated`, `price` (cents a month, euro, may be empty), `location_uuids` and
`node_uuids` (migration `dedicated_plans`). For a dedicated plan:

- `cpu` is whole threads, 100 each (400 = four threads). The admin API refuses anything else.
- **Locations** limit where its servers go (empty: any location with a dedicated node).
  **Nodes** limit it to some dedicated nodes (empty: every dedicated node of those locations).
- **Public**: shown to players with its price. Off: a custom plan for one customer.
- A dedicated plan is never one of the free plans players pick (`Plan::all_public` leaves it out,
  and `POST /api/client/servers` refuses it).

## Nodes

A node with the role **Dedicated** (`nodes.role = 'dedicated'`, Admin › Nodes › a node) only takes
servers of dedicated plans. Players' free servers, moves, limbo restores and failovers only ever
pick `game` nodes, so nothing else lands there.

On a dedicated node (`shared/src/dedicated.rs`):

- **Threads.** A new server is pinned (`servers.pinned_cpus`, Wings' cpuset) to threads no other
  server of the node is pinned to, whole cores first (a thread and its sibling `n / 2` later),
  single threads only when no whole core is left. Thread 0 stays with the system on a node with more
  than two threads. The thread count comes from node telemetry (`node_telemetry.cpu_threads`), so a
  node has to have been sampled once.
- **Memory.** A server only goes where the memory of all the node's servers (and those moving to
  it), plus its own, fits in the node's memory (`nodes.memory`, without overallocation).
- The placement holds a lock per node (`dedicated::<node>`) until the server exists, so two creates
  never get the same threads.

Pin every server of a dedicated node. A server an admin puts there by hand without pinned threads
may run on any thread.

## Servers

`servers.dedicated` and `servers.plan_uuid` (the plan, while it exists; deleting the plan changes no
server). A dedicated server:

- is not counted in its owner's allowance and is never shrunk by an allowance cut
  ([allowance-cuts.md](allowance-cuts.md));
- is never stopped or parked by limbo ([limbo.md](limbo.md));
- skips the start queue and the ad waiting room ([start-queue.md](start-queue.md), [ads.md](ads.md));
- is not resized from its settings page (409 "a dedicated server keeps its plan's size", for staff
  too) and not moved from its transfer page or by "Best node". An admin changes its size and node on
  Admin › Servers, picking the threads by hand;
- is left out of node failover (its threads would have to be picked on the other node).

## Routes

| Route | Permission | What |
|---|---|---|
| `GET/POST /api/admin/plans`, `GET/PATCH/DELETE /api/admin/plans/{plan}` | `plans.*` | Plans with `dedicated`, `price`, `location_uuids`, `node_uuids` |
| `POST /api/admin/servers` | `servers.create` | `plan_uuid` of a dedicated plan: the plan's size, threads picked on `node_uuid` (404 for an unknown plan, 409 with the reason when the node is not the plan's, has no threads or memory left, or has no telemetry yet) |
| `GET /api/client/servers/create` | `servers.deploy` | `dedicated_plans`: the public dedicated plans with threads, memory, disk, price and their locations (`available` when a node there has room) |

The create page shows them under the dedicated name. **Order through support** opens a billing
ticket filled in with the plan and its locations (`/account/support?subject=…&body=…`).

## Without noprofit.space

Nothing changes: dedicated plans need no noprofit.space account.

## Check

- Admin › Nodes: set a node's role to Dedicated. It takes no new free servers.
- Admin › Plans: create a dedicated plan with 2 threads (cpu 200), 4096 MB, a price, public. It is
  missing from the free plans on the create page and shown under the dedicated name with its price.
  Order it: the support form opens filled in.
- Admin › Servers › Create: pick the plan and the dedicated node. The server gets two pinned threads
  (a whole core); a second one gets another core. Fill the node's memory: the next create says the
  node has no free memory.
- The owner's allowance does not count the server; its settings page offers no resize; its transfer
  page says moves are off.
