# Creating servers (players)

Players create their own servers from the dashboard: they name the server, pick a game, a location
and its size (a plan, or their own CPU, memory and disk) and accept the game's EULA when it has one.
The panel finds a node, takes free allocations, creates the server and starts it after its install.
What an account may create is its **allowance**.

It is off until an admin turns it on (Admin › Settings › Hosting › Games); until then only admins
create servers, the native way.

## Plans

A plan is a server size players pick from: CPU (percent of one thread), memory and disk (MiB), a
name, a description, a sort order and whether players see it. Picking one fills in the size; players
may also type their own CPU, memory and disk instead, within their allowance. Admin › Plans lists, creates, edits and
deletes them (`plans` table, `shared/src/models/plan.rs`). Deleting a plan changes no server: a
server keeps its limits, the plan is only what it was created with.

## Games

Admin › Settings › Hosting › Games (`shared/src/settings/games.rs`):

| Setting | Means |
|---|---|
| Players Create Servers | Turns the create page on |
| Players Resize Servers | Owners may change their servers' CPU, memory and disk within their allowance ([server-settings.md](server-settings.md)) |
| Players Delete Servers | Owners may delete their own servers ([server-settings.md](server-settings.md)) |
| Players Move Servers, Hours Between Moves, Nodes Tried per Move, Moves at Once per Node, Minutes Without Progress, Minutes to Finish | Owners may move their servers to another location; when a move's copy stalls ([transfers.md](transfers.md)) |
| Games | The catalog, in order: an egg, the name and description players see (empty uses the egg's), the icon on its card (a link, `https://…` or `/…`; none shows a gamepad), the egg variable the software choice is written to, and the software choices |
| Allocations, databases, backups, schedules | What every new server may have; allocations is at least 1 plus the ports its deployment needs |
| Servers per Account | 0 is no limit |
| CPU, memory, disk per account | The allowance without noprofit.space; 0 is no limit |

A software choice (a **variant**) has a value (written to the game's variable, `purpur`), a label
(`Purpur`), and optionally a startup command and an image. Both may name one of the egg's own
startup commands or images (`Java 21`), else they are used as written; without them the egg's first
is used. The first variant is the default. A game without a variable or variants runs what its egg
sets up. The save refuses an egg listed twice, two variants with the same value, variants without
a variable and an icon that is not a link.

A Minecraft game (a Java Edition server egg, [minecraft.md](minecraft.md#which-servers)) is created with its first variant: its server's
setup asks for the software ([minecraft.md](minecraft.md#setup)), so the create page does not. Other
games with several variants are asked on the create page.

## Allowance

`shared/src/allowance.rs`. The limit is:

- with noprofit.space connected, the account's plan from noprofit.space (`cpu`, `memory`, `disk`:
  this panel's share of the pool the account shares with its other services); an account without a
  plan creates nothing. The pool is shown whole: what the other services use (`elsewhere_*`) is added
  to both the limit and what is used (`Allowance::elsewhere`), so the sidebar reads like the Pelican
  panel's and what is left stays the share less this panel's servers;
- without it, the games settings' CPU, memory and disk per account (the same for every account).

What is used is the sum of the servers the account owns, read from the primary database, so a server
created a moment before is counted. A new server must fit what is left; a resize ([server-settings.md](server-settings.md))
counts the old size, and shrinking always fits, so an account over its limit after its plan got
smaller (or without a plan any more) can still make room. When the limit goes down, the servers give
back what is over by themselves ([allowance-cuts.md](allowance-cuts.md)). Create and resize hold a lock per account (`allowance::{user}` in the cache) for
the check and the write, so two requests at once cannot both spend the same room.

## What happens on create

`shared/src/server_creation.rs`, in this order:

1. The page is on, the account is neither suspended nor frozen, the name is 1 to 191 characters
   (trimmed), the account's standing allows creating ([standing.md](standing.md); admins are not
   limited, a standing nobody can tell limits nobody), the account's lock is free.
2. The game is in the catalog, the EULA is accepted when the egg has the `eula` feature, the variant
   exists, the location is public and its tier is unlocked for the account (every account has the
   empty tier; `plus` comes with Plus, `premium` with Premium, which opens `plus` too), and the size fits the allowance:
   a public plan's, or without a plan the player's own CPU, memory and disk, each at least 1.
3. A node: the location's nodes ranked like an admin deploy ([placement.md](placement.md)), only
   nodes with the role `game`. On the first node with enough free allocations: the egg
   configuration's deployment (port ranges, a second port written to a variable), else a port
   inside the deployment port ranges, made when the node has no free one, and two in a row for a
   FiveM or Multi Theft Auto egg, the second written to its variable
   ([allocations.md](allocations.md)). When none fits, the error says why (every node paused, full,
   in maintenance, no free allocations). The allocations are claimed with the server; a create that
   loses one to another create at the same moment picks again (3 tries). They are locked: the
   player cannot remove them.
4. The server is created through the native `Server::create` (Wings creates it and installs it),
   with the variant's variable and the deployment's variables set, the plan's limits, the games
   settings' feature limits, and "start after install" on, except for a Minecraft egg: that server
   is created without its install (`skip_installer`), not started, and the create page opens its
   setup, which installs it with what is picked there and starts it
   ([minecraft.md](minecraft.md#setup)).
5. With the EULA accepted, the install callback writes `eula.txt` (`eula=true`) into the server
   before Wings starts it (`after_install`, in `remote/servers/{server}/install.rs`). Wings waits
   for that callback before the start, so the file is there for the first boot. A Minecraft
   server's skipped install is reported too, which writes it at once (else the setup's install
   does).

The account's activity log gets `server:create`.

## Routes and permissions

| Route | Permission | Gives |
|---|---|---|
| `GET /api/client/servers/create` | `servers.deploy` | `{options}`: enabled, the account's allowance, the games (icon, EULA, whether the setup asks the software, variants), the public locations (flag, tier, unlocked, `available`, `full`, best response time) and the public plans (fits the allowance). With `?cpu&memory&disk`, `available` means a node there has the memory and disk left for that size (counted like a deploy); `full` is a location without any open node |
| `POST /api/client/servers` | `servers.deploy` | Body `{name, egg_uuid, location_uuid, plan_uuid?, cpu?, memory?, disk?, variant, eula}`: a plan, or `cpu`, `memory` and `disk`; `{server}` on success; 400 for a bad name or size, 403 when off or the account is blocked, 409 when no node or allowance fits, 429 when another create of the account is running |
| `GET/POST /api/admin/plans`, `GET/PATCH/DELETE /api/admin/plans/{plan}` | `plans.read`, `plans.create`, `plans.update`, `plans.delete` | The plans |

`servers.deploy` is a user permission (an API key or a role can lack it). The settings for players
(`games.create_enabled`, `games.allow_resizing`, `games.allow_deleting`) are in the public settings.

## The page

`/create` (`frontend/src/pages/dashboard/create/`), opened by "Create server" above the server list
when it is on. It is laid out like the Pelican panel's create page, one section card each:

- **Details**: the Server Name, filled with a random word (the dice draws another).
- **Game**: a card per game with its icon (or a gamepad), name and description.
- **Server software**, only for a game with several variants whose server has no setup to ask.
- **Location**: a card per location with its flag, a latency meter (four bars under 40 ms, three
  under 80, two under 150, then one), "Full" or "No capacity for the selected resources" when it
  cannot take the server, and its tier pill (Free, Plus, Premium, the uptime behind the (i)). A free
  one that cannot take it is dimmed; a locked paid one shows "Get Plus" / "Get Premium" (the
  membership names) to the membership page. The locations are asked again for the chosen size
  half a second after it changes. The meter is the visitor's own ping (see *Ping* below), and until
  they pick a location themselves the closest one that can take the server is selected.
- **Resources**: the plans ("Exceeds your remaining allowance" when one does not fit), then CPU (%),
  Memory and Disk Space (MB) inputs. A plan fills them in; editing them keeps the plan selected
  only while all three are its own.
- **The EULA** (Minecraft), ticked by default.
- The deploy row: why it cannot be deployed yet (the allowance or server limit, the game, the
  location, the name, the EULA) and "Deploy server". When the allowance is what stops it (a
  resource used up, or the size over what is left, not the server count) and credits are on, a
  link "Get more in the Resource Store" sits beside the reason, while Offers Where a Limit Is Felt
  is on ([promos.md](promos.md#offers)). The same button sits in the header as the
  secondary action, with the reason as its tooltip; Ctrl/Cmd+S deploys too. A refused create shows
  "Could not create server" with the reason. It opens the new server's console.

## Ping

A location's `latency` in the API is the panel's: the fastest answer of its nodes' Wings to the
panel, measured from wherever the panel runs. That is no user's ping (from a panel in Europe, a
location in Asia reads several hundred ms to everyone), so the pages do not show it. Each location
also carries `probes`: up to four of its nodes' Wings addresses (https, at the root; a node in
maintenance is left out). The browser times them (`frontend/src/lib/locationPing.ts`): the first
request to a node opens the connection (DNS, TCP, TLS) and is left out, the fastest of the next
three counts, and a location's ping is its fastest node's. The answer is opaque (`no-cors`), any
status will do. Each node is timed once per page load. The create page, the Transfer page and the
limbo page show this ping; a node that does not answer shows none. The create page also selects
the closest location that can take the server (unlocked, with room for the chosen size) until the
user picks one. `latency` stays for neo's tools and for moves off a lapsed membership.

## Without noprofit.space

The allowance comes from the games settings, the same for every account. Nothing else changes.

## Check

- The location cards show a ping a few seconds after the page opens, and the closest location that
  can take the server is selected; picking another one keeps it.
- Admin › Plans: create two plans, edit one, change the order; hide one and it is gone from the
  create page.
- Admin › Settings › Hosting › Games: add a game with two variants and a variable, set a per-account
  memory limit, turn "Players Create Servers" on and save. Saving an egg twice is refused.
- As a player: "Create server" shows above the server list; create a server, it opens the console,
  installs and starts; with a Minecraft egg it opens the setup at once without installing,
  `eula.txt` says `eula=true` and the server is installed by the setup and starts after it without
  the EULA prompt.
- Create servers until the memory limit: the next plan says "Exceeds your remaining allowance", typing
  more memory than is left greys the deploy button with "This would exceed your resource or server
  limit.", and a create through the API is refused with "not enough memory left in your account".
- Type a size no node of a location has room for: after half a second that location says "No
  capacity for the selected resources".
- With the memory allowance used up: "Get more in the Resource Store" beside the reason, leading
  to Account › Resource Store; at the server limit, or with credits or the offers off, no link.
- Pause every node of a location (Admin › Nodes): the location shows "Full".
- Admin › Settings › Hosting › Games: give a game an icon link; its card shows the picture, without
  one a gamepad.
