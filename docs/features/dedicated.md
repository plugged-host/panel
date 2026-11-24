# Dedicated plans (paid)

A dedicated plan is a paid server size whose **CPU threads and memory are the server's alone**. Disk
is shared like on every node. Players know them as Paid: the product name is Admin › Settings ›
Hosting › Games › Paid Plans Name (`games::dedicated_name`); empty, its default, it is the panel's
name and "Paid" ("plugged.host Paid", `dedicated::product_name`). Migration `paid_name` dropped the
old default, "Dedicated". A paid server is badged **Paid** in the server list.

Dedicated plans are kept apart from the free ones everywhere:

- **Admin › Paid Plans** (`/admin/dedicated-plans`, `plans.*`) lists, creates and edits only
  them, sized in CPU threads with a price, locations and nodes; Admin › Plans only the free ones.
- **Players** get the Paid page (`/account/dedicated`, [below](#the-paid-page)), in the sidebar right
  after Servers (as on switched.cloud) while a public dedicated plan exists or the account owns a
  paid server (`paid_servers` on the account).
- With noprofit.space connected, players **order a public plan with a price themselves** and pay
  it monthly with their noprofit.space Funds ([Paid by the player](#paid-by-the-player)). Without
  noprofit.space, or for a plan without a price, they order it with a support ticket and an admin
  creates the server (Admin › Servers › Create, with the plan). Players never type a size. A
  non-public plan is a custom plan an admin uses for one customer.

## Where players meet Paid

Each spot shows while a public dedicated plan exists (`games.dedicated_offered` in `/api/settings`)
with the lowest price of those plans (`games.dedicated_from`, one cached query a minute,
`dedicated::offer`):

- **Server list**: **Get a paid server** beside Create server, always enabled; the empty list shows
  both buttons.
- **Create page**: a card at the top: the product name, "CPU threads and memory of your own. No
  start queue, no waiting room.", "From {price} / month", **See plans**.
- **Start queue and waiting room**: "Paid servers never wait here. {name} from {price} / month."
  and **See plans** (a new tab in the waiting room, which holds the page).

## The Paid page

Top to bottom, in the same order and cards as switched.cloud's: what Paid adds, checked (dedicated CPU threads, dedicated memory, no start queue and no
waiting room, never parked by Limbo and outside the free allowance); **Your Funds** with Add Funds
(noprofit.space's Funds page) and "Every month is taken from your Funds when the last one ends.
Cancel any time." (without noprofit.space: plans are ordered through support); **Your paid servers**
(`servers` of `GET /api/client/servers/create/dedicated`, those the account owns): name, plan and
price, the status ("Paid until …", "Ends on …", in red "Not paid since …: deleted on … unless
paid"), **Renewal: On/Off** (off asks first) and **Pay next month now** (asks first with the price);
the **plans** as cards with price, threads, memory, disk, locations ("(sold out)") and **Order**
(disabled with "Sold out" when no location has room); a link to the payment history on
noprofit.space.

**Order {plan}** asks for the name, game, software, location and EULA, then sums up: the price a
month, the account's Funds, what is left after paying or "You need {missing} more Funds." with Add
Funds (the button is disabled then), and how renewal works. **Pay {price} and create** pays and
opens the new server.

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

## Paid by the player

`POST /api/client/servers/create/dedicated` (`shared::dedicated::order`):

1. The plan must be public, dedicated and have a price; the account must be linked to
   noprofit.space (402 otherwise) and a node of the plan in the location must have room (409,
   nothing charged).
2. The first month's price is taken from the account's Funds (`calagopus/funds/spend` on
   noprofit.space, key `dedicated:order:<uuid>`; 402 when short).
3. The server is made like a free one (game, software, EULA, Minecraft setup), but with the plan's
   size on a dedicated node of the plan, pinned to its own threads
   (`server_creation::create_dedicated`), not counted in the allowance and whatever the location's
   tier. If it cannot be made, the month is given back (`calagopus/funds/refund`, key
   `dedicated:refund:<uuid>`; a refund noprofit.space refuses is logged as an error to refund by
   hand).
4. `server_subscriptions` records it: `price` (the month's price at the order), `paid_until` (30
   days on), `renews`.

The Funds noprofit.space answers with after a spend are stored on the account's link, so the
sidebar shows them before the next sync.

The `settle_paid_servers` task (every 5 minutes, `shared::dedicated::settle`) takes every month
that ended:

- **Renewing:** the price is taken from the owner's Funds again (once per month: the key is the
  server and the end of the month). Paid on time, the next month follows on; paid late, it starts
  then.
- **Too few Funds, or not renewing:** the server is suspended (kind `unpaid`) with a deletion time
  [`GRACE_DAYS`](../../shared/src/dedicated.rs) (3) days on; the suspension purge deletes it then.
  A renewing one is tried again every run, so Funds added in time bring it back by themselves.

**Pay next month now** (`dedicated::pay_ahead`) pays the month after the paid one at once, with
the same key the task would use (server and `paid_until`), so that month is never taken twice; an
unpaid month is paid the same way and brings the server back. More than a month ahead is refused
(409). Renewal stays as it is.

The owner sees the month on the server's Settings › Billing ("{price} / month from your Funds.",
the status, the same two buttons as the Paid page and a link to it) and on the Paid page. Renewal
off runs to the end of the paid month; turned back on after the month ended, it pays at once and
lifts the `unpaid` suspension. The suspended page has **Pay now** for the same. Only the owner
changes it (it is their money): others read "Only the owner can change its billing." Admin-made
dedicated servers have no subscription and are never charged.

### Notifications

From the same task (`shared/src/notifications.rs`):

- `paidServerEndsSoon`, [`REMIND_DAYS`](../../shared/src/dedicated.rs) (3) days before a month
  ends, once per month (`server_subscriptions.reminded_until`, migration `paid_reminders`): with
  renewal on and Funds short, "{name} renews soon: on {date} it renews for {price}, your Funds are
  {funds}. Add Funds before then."; with renewal off, "{name} ends on {date}." Nothing when the
  Funds cover it.
- `paidServerRenewed`: "{price} was taken from your Funds. Paid until {date}." (every paid month,
  ahead or late included).
- `paidServerUnpaid`: "{name} is stopped until it is paid, and deleted on {date}. Add Funds and it
  comes back by itself." (renewal off: it comes back when renewal is turned on).

## Routes

| Route | Permission | What |
|---|---|---|
| `GET/POST /api/admin/plans`, `GET/PATCH/DELETE /api/admin/plans/{plan}` | `plans.*` | Plans with `dedicated`, `price`, `location_uuids`, `node_uuids` |
| `POST /api/admin/servers` | `servers.create` | `plan_uuid` of a dedicated plan: the plan's size, threads picked on `node_uuid` (404 for an unknown plan, 409 with the reason when the node is not the plan's, has no threads or memory left, or has no telemetry yet) |
| `GET /api/admin/plans?dedicated=true\|false` | `plans.read` | Only the dedicated or only the free plans |
| `GET /api/client/servers/create/dedicated` | `servers.read` | `plans`: the public dedicated plans with threads, memory, disk, price and their locations (`available` when a node there has room); `self_service`: noprofit.space is connected, so plans with a price are ordered here; `servers`: the account's paid servers (`uuid`, `uuid_short`, `name`, `plan`, `price`, `paid_until`, `renews`, `unpaid_since`, `delete_at`) |
| `POST /api/client/servers/create/dedicated` | `servers.deploy` | Order a plan: `plan_uuid`, `location_uuid`, `name`, `egg_uuid`, `variant`, `eula`. 402 too few Funds or no noprofit.space account, 409 sold out |
| `GET /api/client/servers/{server}/billing` | the server | `subscription`: `price`, `paid_until`, `renews`, `unpaid_since`, `delete_at` (`null` when nobody pays for it here) |
| `POST /api/client/servers/{server}/billing` | the owner | `renews`: cancel or keep; on for an ended month pays it now (402 too few Funds). Reachable while suspended |
| `POST /api/client/servers/{server}/billing/pay` | the owner | Pay the next month now, or the unpaid one (402 too few Funds, 409 more than a month paid already). Reachable while suspended |
| `GET /api/client/servers/create` | `servers.deploy` | `dedicated_offered`: whether the page links to the dedicated plans |
| `GET /api/settings` | none | `games.dedicated_name` (the product name), `games.dedicated_offered`, `games.dedicated_from` (lowest price, cents; cached for a minute): the sidebar entry and the spots that lead to Paid |
| `GET /api/client/account` | signed in | `paid_servers`: the account owns a paid server (the sidebar entry stays) |

Without self-service, **Order through support** on the Paid page opens a billing ticket
filled in with the plan and its locations (`/account/support?subject=…&body=…`).

## Without noprofit.space

Dedicated plans still work, ordered through support and created by an admin; nothing is charged
by the panel.

## Check

- Admin › Nodes: set a node's role to Dedicated. It takes no new free servers.
- Admin › Paid Plans: create a plan with 2 threads, 4096 MB, a price, public. It is not on
  Admin › Plans or among the free plans on the create page; within a minute the sidebar gets the
  Paid page, which shows it with its price, and the server list, create page, start queue and
  waiting room show "from" that price. Without noprofit.space, Order opens the support form filled in.
- Admin › Servers › Create: pick the plan and the dedicated node. The server gets two pinned threads
  (a whole core); a second one gets another core. Fill the node's memory: the next create says the
  node has no free memory.
- With noprofit.space and Funds on the account: order a priced plan from the dedicated page. The
  price leaves the Funds (noprofit.space › Funds history), the server is made on the dedicated node
  with pinned threads, and Settings › Billing shows the month. Set `paid_until` in the past in
  `server_subscriptions` with too few Funds: within 5 minutes it is suspended as unpaid with a
  deletion date and a "not paid" notification; add Funds and press Pay now: it is back and paid 30
  days on, with a "renewed" notification.
- Pay next month now on the Paid page: the Funds go down by the price and Paid until moves 30 days;
  a second press says the next month is paid already.
- Set `paid_until` 2 days ahead with renewal off: within 5 minutes one "ends on" notification
  arrives, and no second one.
- The owner's allowance does not count the server; it has the Paid badge in the server list; its
  settings page offers no resize; its transfer page says moves are off.
