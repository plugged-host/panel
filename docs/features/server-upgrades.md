# Server upgrades

What a user buys with tokens for one server: a keep-awake pass, a boost, an extra port, database or
backup slot, a bigger deposit for a short subdomain name, and a colour for the server's card. In
`shared/src/server_upgrades.rs` (its pure parts unit-tested at its bottom), the settings in
`shared/src/settings/upgrades.rs`.

| Upgrade | Default price | Runs | Does |
|---|---|---|---|
| Keep awake | 3 tokens | 7 days | [Limbo](limbo.md) neither stops, parks nor deletes the server as idle while it runs. Sold only while limbo is on |
| Boost | 3 tokens | 24 hours | +2048 MiB memory and +100 % CPU on top of the paid limits, outside the allowance. Sold only while the node has room for it |
| Extra port | 2 tokens | 30 days | +1 to the server's allocation limit, at most 3 at once |
| Extra database | 2 tokens | 30 days | +1 to the server's database limit, at most 3 at once |
| Extra backup slot | 2 tokens | 30 days | +1 to the server's backup limit, at most 3 at once |
| Short subdomain name | a deposit of 15 tokens (3 characters), 30 (2 characters) | while the subdomain exists | Refunded on deletion like any [subdomain](subdomains.md) deposit |
| Card colour | 2 tokens | for good | A colour from a fixed palette as a thin line along the top of the server's card in the server list; changing it later is free |

Nothing is sold while tokens are off (Admin › Settings › Economy › Tokens); what runs keeps
running until its end.

## Buying

- **Who**: the owner, or a subuser with `settings.upgrades`. **The buyer pays** with their own
  tokens (unlike a subdomain deposit, which is always the owner's).
- **Money safety**: the purchase locks the server row, then the buyer's user row, checks the price
  the user saw against the price now (409 "the price changed to N tokens" when it moved, and the
  section shows the new one) and the balance (409 "this costs N tokens, you have M"), and writes the
  ledger row (`server_upgrade`, "Keep awake: <server>") and the effect in one transaction.
- **Again while one runs**: buying a keep-awake pass or a boost while one runs adds its time to the
  end (from now once the end is past). An extra slot is renewed on its own ("Add 30 days"), and
  "Buy" adds another one, up to the setting's most.
- A **parked** server ([limbo.md](limbo.md)) is sold only the keep-awake pass (it keeps it from
  being deleted, and from being parked again once it is back) and the card colour. Its upgrades
  route is reachable while it is parked.

## The boost

`servers.boost_memory` and `servers.boost_cpu` hold the running boost; the panel adds them to the
limits Wings gets (`build.memory_limit`, and `build.cpu_limit` before the [burst
cores](economy.md#burst-cores), whose CPU weight counts the boost as paid CPU), the same path burst
cores take, and syncs the server, so a running container gets them without a restart. The game
itself may need a restart to use more memory (a Java server's `-Xmx` is read at start); the row says
so. The client API's `limits.memory` and `limits.cpu` include the boost, so the server card, the
console and the graphs show it. Admin › Servers shows the paid limits.

It is sold only while the node has room for it:

- the node takes new servers (not paused for its load or by an admin; a disk pause does not
  matter);
- its allocated memory (every server on it that is not parked, with overheads and running boosts)
  plus the boost stays within the node's memory, like a resize;
- its newest telemetry (at most 3 minutes old, answering) stays under the deployment pause line
  ([placement.md](placement.md), 85 % by default, lower on a small node) with the boost on top:
  memory used plus the boost's memory, and CPU plus the boost's share of the node's threads.

A server without a memory and a CPU limit is not offered one. Making a boost longer needs no room
check (it holds its room already). When it ends the columns go back to 0 and the server is synced:
Wings applies the lower limits to the running container.

## Extra slots

Buying one raises the server's own limit (`allocation_limit`, `database_limit`, `backup_limit`) by
one, so every check of the limit (upstream's) counts it, and Admin › Servers shows the raised
limit. When it ends the limit goes back down by one, never below 0. **Nothing is deleted**: a
server over its limit keeps its ports, databases and backups and cannot add more until it is under
it (a new backup then works as it does at the limit). An extra port the server added stays, as
upstream keeps allocations when an admin lowers the limit.

The network, databases and backups pages offer one more ("Port limit reached. Add one more port
for 30 days · 2 tokens.") once the limit is reached, for whoever may add one there.

## Keep awake

`settle_limbo` leaves out servers with a running pass (`limbo::candidates`), and so does deleting
servers parked too long (`limbo::purge_due`). Limbo's stop tells the owner (`serverIdleStopped`,
[notifications.md](notifications.md)): how long it was idle, when it will be parked, and what a pass
costs while one is sold, with a link to the server's Settings › Upgrades. The console of a server
limbo stopped (`limbo_state` `stopped`, while it is off) offers a pass, and so does a parked
server's limbo page.

## Short subdomain names

A subdomain's deposit is the larger of the server's tier ([subdomains.md](subdomains.md#limits-and-tokens))
and the short name's: `short_subdomain_three` (15) for 3 characters, `short_subdomain_two` (30)
for 2. The create form's deposit follows the name as it is typed ("Token Deposit • Short name"),
so the price is shown before Create. A rename to a short name that takes more than the subdomain's
deposit is refused ("add it as a new subdomain"), so the bigger deposit is always paid by adding.

## Card colour

One of `blue`, `cyan`, `teal`, `green`, `yellow`, `orange`, `red`, `pink`, `grape`, `violet`
(Mantine's palette at shade 5; `blue` is the panel's accent), in `servers.card_color` and the
client API's `card_color`. The server list's card (`ServerItem.tsx`, `data-card-color`) draws it
as a 2 px line along its top edge, between the rounded corners; no gradient, no glow. Bought once
per server; after that the picker saves a new colour for free.

## The server page

Settings › **Upgrades** (`#upgrades`, after Resources; `UpgradesSection.tsx`), in the Danger Zone's
row layout: the note "Extras for this server, paid with your own tokens. You have N tokens.", then
one full-width row per running purchase (what it does, "Runs until …", a gray "Add 7 days · 3
tokens") and per offer (what it does, "1 of 3 running.", why it cannot be bought now, a gray "Buy ·
2 tokens"), the card colour last ("Change colour" once bought). Every purchase is confirmed first
with its price and the buyer's balance; a balance that does not cover it turns the confirmation
into "Earn tokens" (Account › Tokens). A toast says until when it runs; the sidebar balance and the
server's limits are loaded again. The section shows for `settings.upgrades` while something is
sold or runs.

## Ending and reminders

The `settle_upgrades` task (every minute, second 25):

1. ends what ran out (`ended`): an extra slot's limit goes back down, a boost comes off and the
   server is synced; `server:upgrade.end`.
2. tells whoever bought a keep-awake pass or an extra slot that it ends within 48 hours
   (`upgradeEnding`, once per end date: `reminded_until`), with what adding time costs while it is
   sold and a link to Settings › Upgrades. The buyer, or the owner once the buyer has no access to
   the server. A boost is shorter than that and gets none.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/upgrades` | `settings.upgrades` | `{upgrades}`: `offers: [{kind, tokens, hours, memory, cpu, max, unavailable}]` (`unavailable`: `max`, `node_busy`, `node_unknown`, `unlimited`, `parked` or `null`), `purchases: [{uuid, kind, until, created}]`, `card_color`, `palette`, `balance` (the viewer's) |
| `POST /api/client/servers/{server}/upgrades` | `settings.upgrades` | `{kind, renew?, tokens, color?}` → `{upgrades}`; 409 price changed, short, the most run, no room, parked, a colour bought already; 404 the slot to renew does not run |
| `PUT /api/client/servers/{server}/upgrades/color` | `settings.upgrades` | `{color}` (`null` takes it off) → `{upgrades}`; 409 no colour bought yet |

`kind` is `keep_awake`, `boost`, `extra_allocation`, `extra_database`, `extra_backup` or
`card_color`. Server activity: `server:upgrade.buy`, `server:upgrade.renew` (`kind`, `tokens`,
`until`), `server:upgrade.color`, `server:upgrade.end` (no user). Token history: "Server upgrade".

## Tables

| Table / columns | What it holds |
|---|---|
| `server_upgrades` | One row per purchase: `server_uuid`, `user_uuid` (the buyer), `kind`, `amount` (added to the limit), `tokens` (paid, renewals included), `data` (a boost's memory and CPU, the colour), `until` (`NULL` for good), `reminded_until`, `ended`, `created` |
| `servers.boost_memory`, `boost_cpu` | The running boost (0 without one) |
| `servers.card_color` | The card's colour |

## Settings

Admin › Settings › Economy › Server Upgrades:

| Field | Key | Default |
|---|---|---|
| Keep-awake passes: on, tokens, days | `upgrades::keep_awake_{enabled,tokens,days}` | on, 3, 7 |
| Boosts: on, tokens, hours, memory (MiB), CPU (%) | `upgrades::boost_{enabled,tokens,hours,memory,cpu}` | on, 3, 24, 2048, 100 |
| Extra ports: on, tokens, days, at most | `upgrades::extra_allocation_{enabled,tokens,days,max}` | on, 2, 30, 3 |
| Extra databases | `upgrades::extra_database_{enabled,tokens,days,max}` | on, 2, 30, 3 |
| Extra backup slots | `upgrades::extra_backup_{enabled,tokens,days,max}` | on, 2, 30, 3 |
| Short subdomain names: on, 3 characters, 2 characters | `upgrades::short_subdomain_{enabled,three,two}` | on, 15, 30 |
| Card colours: on, tokens | `upgrades::card_color_{enabled,tokens}` | on, 2 |

## Known limits

- Placement, moves and resizes count a node's paid limits only: a boost is checked against the node
  when it is bought, and a server moved while boosted takes its boost along unchecked.
- A boost's memory reaches a Java server's heap only after a restart; when the boost ends while
  the game uses more than its own limit, the container may be stopped by its node for memory.
- An extra slot raises the server's stored limit, so an admin who lowers the limit while one runs
  sets the limit with it included; when it ends one is taken off what is stored then.

## Without noprofit.space

Nothing changes: tokens are always the panel's own.

## Check

- Admin › Settings › Economy › Server Upgrades: the defaults above; save.
- Limbo on, a user with 20 tokens: the server's Settings › Upgrades lists Keep awake, Boost, Extra
  port, Extra database, Extra backup slot and Card colour, each with "Buy · N tokens". Buy Keep
  awake: the confirmation says 3 tokens and the balance; after it the toast says until when it
  runs, the sidebar shows 17 tokens and the row says "Runs until …" with "Add 7 days · 3 tokens";
  add 7 days: the date moves 7 days. Stop the server and leave it past Stop After: limbo does not
  stop it.
- Boost on a running server: the console's memory card shows 2 GiB more without a restart; on the
  node `docker inspect <uuid> --format '{{.HostConfig.Memory}}'` says so. Admin › Users › the user ›
  Economy shows the 3 tokens in the token history. Set Hours to 1 and buy one on another server:
  within a minute of its end the memory card shows the paid limit again.
- On a node over the pause line (or paused in Admin › Nodes), Boost says the node is busy and its
  button is off.
- Network page at the allocation limit: "Port limit reached" offers a port; buy it: Add works once
  more. Let it end (Days 1, wait): the limit drops, the port stays, Add is off.
- Databases and Backups at their limits: the same offer; after the end the databases and backups
  are all still there.
- Subdomains: type `ab`: the deposit says 30 tokens, `abc` 15, `abcd` the tier's price. Create
  `abc`, delete it: 15 tokens back. Rename a long subdomain to `ab`: refused under the field.
- Card colour: Buy · 2 tokens opens the palette; pick red and buy: the server's card on the server
  list has a red line along its top; Change colour to teal: no tokens spent.
- With a subuser holding `settings.upgrades`: they see the section and pay with their own tokens.
  Without it: no section.
- Tokens off: no section on a server without running upgrades; nothing to buy where a limit is
  reached.
- A keep-awake pass 48 hours before its end: the buyer gets "Keep awake ends soon" with "Add time".
- Limbo stops an idle server: the owner gets "Server stopped while idle" with the keep-awake price;
  the console offers the pass while the server is off.
