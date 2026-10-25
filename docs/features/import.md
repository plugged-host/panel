# Importing a Pelican panel

Upstream's `panel-rs import pelican` (and `import pterodactyl`) reads the old panel's database and
writes its users, SSH keys, locations, nodes, nests, eggs, servers, databases, backups, schedules
and subusers into an empty panel, in one transaction. This fork adds what plugged.host's Pelican
panel kept in its plugins and in its own conventions, so a panel moves over with its players'
balances, links and settings:

- `backend/src/commands/import/mapping.rs` (new): the rules, pure and unit-tested (locations from
  node tags, node capacity, the FiveM egg feature, regional languages, the permissions of the plugin
  pages, which schedules may start a server).
- `backend/src/commands/import/extras.rs` (new): reads the plugin tables, plans their rows for the
  imported users and servers, writes them inside the import's transaction, stores the server icons
  afterwards, and leaves devboxes out.
- `backend/src/commands/import/countries.rs` (new): ISO 3166-1 country codes and their English
  names, as the Pelican panel's location catalog named them.
- Upstream's `pelican.rs`, `pterodactyl.rs`, `model.rs`, `validate.rs`, `write.rs` and `mod.rs`
  call them (marked lines).

Every plugin table is optional: a table or column the source panel does not have reads as empty,
so a plain Pelican panel imports as before (with its overallocation and its regional languages
handled). A Pterodactyl import gets the language and schedule rules below and nothing else.

## Running it

The import needs the new panel's environment (its database and cache), the old panel's `.env` and
a connection to the old panel's database. It only reads the old database.

```bash
# 1. a dry run: reads both databases, prints what would be written and every problem, writes nothing
panel-rs import pelican --environment /var/www/pelican/.env --dry-run --on-invalid=fix \
  --report /root/import-report.json

# 2. the import, into an empty panel
panel-rs import pelican --environment /var/www/pelican/.env --on-invalid=fix \
  --report /root/import-report.json
```

- `--on-invalid=fix` applies the suggested fix where there is one and leaves the row out
  otherwise (the report lists both); `skip` leaves out every row with a problem, `abort` reports
  them and writes nothing; in a terminal without the flag it asks per group of problems.
- The summary prints "ready to import:" with a count per table (the plugin data included: "noprofit.space
  links", "token balances", "subdomains", "server icons", …) and `note:` lines for what was changed
  or left out on purpose (devboxes, start steps, regional languages, Discord links without an id).
  Read the notes and the report before the real run.
- Run the dry run against an empty panel (`--force` imports next to existing data and reports the
  collisions, which a dry run then also shows). A scratch database does for that, and a source
  user that may only `SELECT` keeps the old panel safe; the dry run reads the icon folder but
  stores nothing:

  ```bash
  # a scratch database, migrated like a new panel's: the panel's environment, with DATABASE_URL
  # (and DATABASE_URL_PRIMARY, if it is set) pointing at the scratch database
  createdb -O <role> panel_import_dry
  set -a; . /etc/panel/panel.env; set +a
  export DATABASE_URL=postgresql://<role>:<password>@127.0.0.1/panel_import_dry
  unset DATABASE_URL_PRIMARY
  panel-rs database-migrator migrate

  # a copy of the old panel's .env with a read-only database user, next to its icons
  mkdir -p /root/import/storage/app/public/icons
  cp /var/www/pelican/.env /root/import/.env   # then set DB_USERNAME / DB_PASSWORD to the read-only user
  ln -s /var/www/pelican/storage/app/public/icons/server /root/import/storage/app/public/icons/server

  panel-rs import pelican --environment /root/import/.env --dry-run --on-invalid=fix \
    --report /root/import/report.json
  ```

  Compare the counts with the old panel (users, servers, subusers, `token_transactions`, …), read
  every `note:` and the report's skipped rows, then drop the scratch database.
- Server icons are files: they are read from `storage/app/public/icons/server/` next to the `.env`
  passed with `--environment`. When the import runs elsewhere, copy the `.env` and that folder
  together (`<dir>/.env`, `<dir>/storage/app/public/icons/server/`).
- Restart the panel afterwards (the import says so).

## Locations and nodes

The Pelican panel has no locations of its own: Pelican makes one per node, and plugged.host's
location catalog offered one per **country tag** of the nodes (`de`, `nl`, … an ISO 3166-1 code),
with the tier tags putting a location into a plan tier. When at least one node has a country tag,
the import follows that catalog instead of upstream's one location per first tag:

| Node | Becomes |
|---|---|
| A country tag, deployable | A `game` node in the location of its country and tier: named after the country ("Germany"; "Germany Premium" / "Germany Plus" only when the country has nodes of several tiers), `flag` the country code, `tier` `premium` or `plus` from the tier tag, public when one of its nodes is public |
| The limbo, `backup` or `storage` tag, no country tag, or a country outside the deployment tags | An `internal` node in a location players never see (`public` off), named after that internal tag (`limbo`, `backup`, `storage`), else after its first tag, or `Internal` without one |
| A node limbo parks servers on (its tag, or recorded in `server_limbo_states`) | A `limbo` node (below), its location players never see when it holds nothing else |

The tags come from the old panel's `.env` (each with its default when unset):

| Variable | Default | Means |
|---|---|---|
| `UCS_DEPLOYMENT_TAGS` | none (every country tag) | Comma-separated; only nodes with one of these tags take players' servers |
| `UCS_PREMIUM_TAG` | `premium` | The tag of the second membership's nodes (tier `premium`) |
| `UCS_PLUS_TAG` | `plus` | The tag of the first membership's nodes (tier `plus`) |
| `LIMBO_NODE_TAG` | `limbo` | The limbo node's tag (a `limbo` node, in an internal location like `backup` and `storage`) |

Each node also keeps its tags (`nodes.tags`, normalized: lower case, sorted, at most 32 of 1 to 64
letters, digits, `-`, `_`, `.`; others are dropped with a note) and its SRV target
(`nodes.srv_target`, at most 255 characters), see [node-telemetry.md](node-telemetry.md#node-and-location-fields).
Sort orders start at 0 for every location; set them in Admin › Locations.

### Capacity

Pelican lets a node take more than its memory and disk through the overallocation; Calagopus has
none, and `0` means unlimited. The import writes what Pelican would allow:

| Pelican | Calagopus |
|---|---|
| memory or disk 0 or less | 0 (unlimited) |
| overallocation -1 | 0 (unlimited), with a note: the node had a size, but no limit |
| overallocation 0 or more | value × (100 + overallocation) / 100 |

A Pterodactyl node keeps its memory and disk as they are. The placement and the deployment pause
([placement.md](placement.md)) look at the real use, so an unlimited node still stops taking new
servers when it is busy, and the disk guard (the Pelican panel's storage guard) still holds a node
whose disk is full.

## Eggs

An egg tagged `cfx` (the FiveM and RedM eggs) gets the `cfx` feature, which opens the FiveM pages
([fivem.md](fivem.md)). Other tags are not features and stay out.

## Subusers

Pelican's core permissions map as upstream maps them. The plugins' pages had no permissions of
their own; they asked for core ones. A subuser gets the page's Calagopus permission when they had
**every** Pelican permission the page needed, so nobody can do more than before:

| Calagopus | Needs all of |
|---|---|
| `minecraft.content` | `file.create`, `file.update`, `file.delete` |
| `minecraft.settings`, `minecraft.players` | `file.read`, `file.read-content`, `file.update` |
| `minecraft.worlds` | `file.read`, `file.read-content`, `file.create`, `file.update`, `file.delete` |
| `minecraft.bedrock` | `file.create`, `control.restart` |
| `fivem.config` | `file.read`, `file.read-content`, `file.update` |
| `fivem.players` | nothing (the page had no gate) |
| `fivem.artifacts` | `startup.read`, `startup.update` |
| `fivem.frameworks` | `file.read`, `file.read-content`, `file.create`, `file.delete`, `control.restart`, `database.create` |
| `subdomains.read`, `.create`, `.update`, `.delete` | `allocation.read`, `.create`, `.update`, `.delete` (the subdomains plugin used them) |
| `settings.software` | `settings.reinstall` (switching was a reinstall) |

The plugins' own keys map directly: `transfer.move` → `settings.transfer`, `devbox.connect` and
`neo.access` as they are. Resizing and deleting a server were the owner's alone and are not given.
Only a Pelican import derives these (a Pterodactyl panel had no such pages).

## Schedules

A schedule runs on Wings, past the start gates ([start-gates.md](start-gates.md)), so the panel
refuses a schedule step that starts a server unless the server's owner is an admin. The import
does the same: a `start` step on a server whose owner is not an admin is left out, together with
the sleep before it, with a note. Restart, stop, command and backup steps stay. The schedule itself
stays, so its owner can see what is left and add the start back as a restart.

## Devboxes

A devbox on the Pelican panel was a hidden server (`plugged_host_devboxes`) with a mount of its game
server's files. The panel makes devboxes anew on their first start ([devboxes.md](devboxes.md)), so
the import leaves out:

- the devbox servers and everything on them (schedules, databases, variables, backups, subusers,
  subdomains, icons); their allocations stay as free allocations of the node,
- the devbox mounts and their links,
- the devbox egg and its nest, unless a server that stays uses them (or another egg copies them),
- the system account that owned the devboxes, unless it is an admin or owns or shares anything else.

The note "devboxes were left out" counts each. Their files stay on the nodes, where nothing uses
them any more; remove them there.

## Users

- A regional language the panel does not ship goes to the language it does (`pt_BR` → `pt`); any
  other unshipped language becomes the panel's default, as upstream does.
- The console font size (the Pelican `customization` JSON's `console_font_size`, 6 to 64) is kept
  in `user_settings` as `console::font_size` when it is not the default 14.

## Plugin data

| Pelican table | Calagopus | How |
|---|---|---|
| `users.noprofit_account_id` | `user_noprofit_accounts` | The link, with the imported email and username; an account linked to two users goes to the first, with a note |
| `user_tokens` | `users.tokens`, `tokens_earned`, `tokens_daily_granted` | Balance, lifetime earned, last daily grant |
| `token_transactions` | `user_token_transactions` | Every row; types renamed: `admin_adjustment` → `admin`, `limbo_restore` → `limbo`, `limbo_restore_refund` → `limbo_refund`, `subdomain_create` → `subdomain`; the metadata JSON as `data` |
| `user_credits` | `users.credits`, `extra_cpu`, `extra_memory`, `extra_disk` | The balance and the Resource Store purchases |
| `credit_transactions` | `user_credit_transactions` | Every row; `admin_adjustment` → `admin` |
| `user_memberships` | `user_memberships` | Plus since, PlusPlus until, purchases, last settled |
| `discord_links` | `discord_members` | Discord id, name, avatar, in the server, boosting (level 1 or 2 from `boost_count`) and since when, invite debt, last check; never the OAuth tokens. A link without a valid id is left out, a Discord account linked twice is imported once |
| `discord_links.boosts_rewarded` | `discord_boost_payouts` | One payout per rewarded level, with the credits the Pelican panel paid (`DISCORD_CREDITS_PER_BOOST_1`, the difference to `_2`), so a level is never paid twice |
| `discord_invited_members` | `discord_invites` | Inviter, member, code, join time, eligible, maturity (`matured_at`, else join + `DISCORD_INVITE_MATURITY_HOURS`), rewarded with `DISCORD_CREDITS_PER_INVITE`, left, revoked, counts |
| `cloudflare_domains` | `domains` | Name (lower case), zone id |
| `subdomains` | `server_subdomains` | Name, record type (`A`, `AAAA`, `SRV`), the Cloudflare record ids by key (`a`, `srv`, …; a single id becomes `a`), the deposit paid |
| `servers.subdomain_limit` | `servers.subdomain_limit` | Only a limit above 0 (Pelican's 0 meant the default) |
| `server_limbo_states` | `servers.limbo_state`, `server_limbo` | Servers parked on a limbo node stay there, `parked` (since `limbo_at`, home node and ports kept, limbo's suspension lifted); idle stops become `stopped` (since `stopped_at`); the rest are ordinary servers. The nodes become `limbo` nodes. [limbo.md](limbo.md#parked-on-a-node) |
| server icon files | `servers.icon` | Read from `storage/app/public/icons/server/<uuid>.<png,jpg,webp>`, made into the panel's WebP icon and stored after the import committed; a failing icon is a warning, the server keeps the egg's icon |

Rows of users or servers that were not imported are left out with a note. A panel imported into with
`--force` keeps the rows it already has (the inserts skip conflicts).

The Cloudflare API token, the Discord bot and the economy prices are settings, not data: set them in
Admin › Settings ([subdomains.md](subdomains.md#settings), [discord.md](discord.md),
[economy.md](economy.md)).

### With and without noprofit.space

With noprofit.space connected ([noprofit.md](../noprofit.md)), the account sync takes over the
imported links within a minute: a linked account that no longer exists on noprofit.space is marked
deleted and its user suspended, as for any other link. Without it, the links stay unused. Discord
members without an OAuth link of their own are forgotten by the standalone Discord check
([discord.md](discord.md)); their invites and payouts stay.

## Not imported

These have no Calagopus counterpart yet, or bringing them over is a product decision:

- rux's and neo's state (cases, warnings, conversations), deleted servers kept for their owners,
  the status page's history and incidents, sponsored banners.
- The user's time zone, servers per page, API keys, activity logs and per-server user settings.

## Check

Do a dry run first (above), read its notes and the report, then after the import:

- Admin › Locations: one location per country (flag and tier shown), the limbo, backup and storage
  locations not public.
- Admin › Nodes: role, tags, SRV target; a node with overallocation -1 shows unlimited memory and
  disk; the limbo node(s) with role Limbo.
- A server the Pelican panel had parked: the limbo page (not suspended), Restore offers its old
  node ([limbo.md](limbo.md#parked-on-a-node)).
- Admin › Nests: the FiveM egg has the `cfx` feature; a FiveM server shows its FiveM pages.
- A subuser of a Minecraft server: the Minecraft and subdomain pages they could use before.
- A server with a scheduled start: the start step is gone, the schedule is there.
- Account › Tokens and Account › Store: balances and histories; Account › Membership: the
  membership.
- A server with a subdomain and an icon: both shown, the subdomain resolves.
- Account › Discord: linked, in the server, boosts; Admin › Users: the Discord column.
- With noprofit.space: Account shows the linked account; Admin › Users has no new suspensions
  beyond accounts gone from noprofit.space.
