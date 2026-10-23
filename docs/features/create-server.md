# Creating servers (players)

Players create their own servers from the dashboard: they pick a game, its software, a location and
a plan, name the server and accept the game's EULA when it has one. The panel finds a node, takes
free allocations, creates the server and starts it after its install. What an account may create is
its **allowance**.

It is off until an admin turns it on (Admin › Settings › Games); until then only admins create
servers, the native way.

## Plans

A plan is a server size players pick from: CPU (percent of one thread), memory and disk (MiB), a
name, a description, a sort order and whether players see it. Admin › Plans lists, creates, edits and
deletes them (`plans` table, `shared/src/models/plan.rs`). Deleting a plan changes no server: a
server keeps its limits, the plan is only what it was created with.

## Games

Admin › Settings › Games (`shared/src/settings/games.rs`):

| Setting | Means |
|---|---|
| Players Create Servers | Turns the create page on |
| Players Resize Servers | Owners may change their servers' CPU, memory and disk within their allowance ([server-settings.md](server-settings.md)) |
| Players Delete Servers | Owners may delete their own servers ([server-settings.md](server-settings.md)) |
| Players Move Servers, Hours Between Moves, Nodes Tried per Move, Moves at Once per Node | Owners may move their servers to another location ([transfers.md](transfers.md)) |
| Games | The catalog, in order: an egg, the name and description players see (empty uses the egg's), the egg variable the software choice is written to, and the software choices |
| Allocations, databases, backups, schedules | What every new server may have; allocations is at least 1 plus the ports its deployment needs |
| Servers per Account | 0 is no limit |
| CPU, memory, disk per account | The allowance without noprofit.space; 0 is no limit |

A software choice (a **variant**) has a value (written to the game's variable, `purpur`), a label
(`Purpur`), and optionally a startup command and an image. Both may name one of the egg's own
startup commands or images (`Java 21`), else they are used as written; without them the egg's first
is used. The first variant is the default. A game without a variable or variants runs what its egg
sets up. The save refuses an egg listed twice, two variants with the same value and variants without
a variable.

## Allowance

`shared/src/allowance.rs`. The limit is:

- with noprofit.space connected, the account's plan from noprofit.space (`cpu`, `memory`, `disk`);
  an account without a plan creates nothing;
- without it, the games settings' CPU, memory and disk per account (the same for every account).

What is used is the sum of the servers the account owns, read from the primary database, so a server
created a moment before is counted. A new server must fit what is left; a resize ([server-settings.md](server-settings.md))
counts the old size, and shrinking always fits, so an account over its limit after its plan got
smaller (or without a plan any more) can still make room. When the limit goes down, the servers give
back what is over by themselves ([allowance-cuts.md](allowance-cuts.md)). Create and resize hold a lock per account (`allowance::{user}` in the cache) for
the check and the write, so two requests at once cannot both spend the same room.

## What happens on create

`shared/src/server_creation.rs`, in this order:

1. The page is on, the account is neither suspended nor frozen, the account's lock is free.
2. The game is in the catalog, the EULA is accepted when the egg has the `eula` feature, the variant
   exists, the location is public and its tier is unlocked for the account (every account has the
   empty tier; plans with tiers arrive with plugged.host Plus in Phase 3), the plan is public, and
   the plan fits the allowance.
3. A node: the location's nodes ranked like an admin deploy ([placement.md](placement.md)), only
   nodes with the role `game`. On the first node with enough free allocations: the egg
   configuration's deployment (port ranges, a second port written to a variable), else any free
   allocation. When none fits, the error says why (every node paused, full, in maintenance, no free
   allocations).
4. The server is created through the native `Server::create` (Wings creates it and installs it),
   with the variant's variable and the deployment's variables set, the plan's limits, the games
   settings' feature limits, and "start after install" on.
5. With the EULA accepted, the install callback writes `eula.txt` (`eula=true`) into the server
   before Wings starts it (`after_install`, in `remote/servers/{server}/install.rs`). Wings waits
   for that callback before the start, so the file is there for the first boot.

The account's activity log gets `server:create`.

## Routes and permissions

| Route | Permission | Gives |
|---|---|---|
| `GET /api/client/servers/create` | `servers.deploy` | `{options}`: enabled, the account's allowance, the games, the public locations (flag, tier, unlocked, has an open node, best response time) and the public plans (fits the allowance) |
| `POST /api/client/servers` | `servers.deploy` | Body `{name, egg_uuid, location_uuid, plan_uuid, variant, eula}`; `{server}` on success; 403 when off or the account is blocked, 409 when no node or allowance fits, 429 when another create of the account is running |
| `GET/POST /api/admin/plans`, `GET/PATCH/DELETE /api/admin/plans/{plan}` | `plans.read`, `plans.create`, `plans.update`, `plans.delete` | The plans |

`servers.deploy` is a user permission (an API key or a role can lack it). The settings for players
(`games.create_enabled`, `games.allow_resizing`, `games.allow_deleting`) are in the public settings.

## The page

`/create` (`frontend/src/pages/dashboard/create/`), opened by "Create server" above the server list
when it is on. Game, software (when there are several), location (flag, response time, "Needs plus"
or "Full right now" when it cannot be picked), plan ("More than you have left" when it does not
fit), then name and the EULA box, and one button. What is left of the allowance is shown on top. It opens the new server's console.

## Without noprofit.space

The allowance comes from the games settings, the same for every account. Nothing else changes.

## Check

- Admin › Plans: create two plans, edit one, change the order; hide one and it is gone from the
  create page.
- Admin › Settings › Games: add a game with two variants and a variable, set a per-account memory
  limit, turn "Players Create Servers" on and save. Saving an egg twice is refused.
- As a player: "Create server" shows above the server list; create a server, it opens the console,
  installs and starts; with a Minecraft egg, `eula.txt` says `eula=true` and the server starts
  without the EULA prompt.
- Create servers until the memory limit: the next plan says "More than you have left" and a create through the API
  is refused with "not enough memory left in your account".
- Pause every node of a location (Admin › Nodes): the location shows "Full right now".
