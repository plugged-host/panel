# FiveM

FiveM and RedM servers get their own pages in the server sidebar, in the Pelican panel's order:
**Frameworks** (txAdmin's recipes, installed in one click), **Players** (who is on the server now),
**Server Configuration** (the `server.cfg` txAdmin runs, as a form), **Artifacts** (the FXServer
build and how it updates) and **GTAV Enhanced** (FiveM for GTAV Enhanced and the switch to it). The
pages look like the Pelican panel's (Theme v2): full-width rows on raised cards, Filament's aside
sections on Server Configuration (`elements/data-display/AsideSection.tsx`), and their own classes
in `frontend/src/app.css` (`.theme-aside*`, `.theme-fivem-*`).

The code is in `shared/src/fivem/` (`server_cfg.rs` reads and edits `server.cfg` in place,
`config.rs` the form over it, `txadmin.rs` where txAdmin keeps its files, `frameworks.rs` the recipe
list and the install, `recipe.rs` the recipe runner over Wings, `players.rs`, `artifacts.rs` what
Cfx offers, `runtime.rs` the server's build and edition), the routes in
`backend/src/routes/api/client/servers/_server_/fivem/`, the pages in
`frontend/src/pages/server/fivem/`. Files are read and written through Wings with the Minecraft
pages' helpers (`shared/src/minecraft/files.rs`).

## Which servers

A server is a FiveM server when its egg has the `cfx` feature. Pelican eggs mark FiveM with a
`cfx` *tag*, which Calagopus does not keep on import, so add the feature to an imported egg, or
import [`docs/eggs/fivem.json`](../eggs/fivem.json), which has it. Other servers neither show the
pages (`frontend/src/lib/domain/games.ts` hides `/fivem/*` routes) nor answer the routes (404 "this
server does not run fivem").

RedM is the same egg: a server is a RedM server when its `server.cfg` sets `gamename rdr3`. RedM
gets RedM's game builds in the form and no Enhanced edition.

## The image

The panel never downloads an FXServer artifact. The image does: its `fivem-runtime` runs at every
start (and in the install script) and keeps `alpine/` on the build the server's variables ask for:

| Variable | Values | Set by |
|---|---|---|
| `FIVEM_VERSION` | `recommended`, `latest` (follow Cfx's channel: every start installs its current build when it changed) or a build number (stays on it) | the Artifacts page |
| `FIVEM_EDITION` | `legacy` or `enhanced` | the GTAV Enhanced page |

After installing, it writes `alpine/.plugged-host.json` (`{"edition", "build", "installed_at"}`),
which is how the Artifacts page knows what is installed. The image's source is
`plugged-host-fivem/image/` in [plugged-host/plugins](https://github.com/plugged-host/plugins), and
`docs/eggs/fivem.json` runs on it. A server whose egg lacks `FIVEM_VERSION` shows the Artifacts page
with the update switch disabled and no Install buttons, as the Pelican panel does; one that lacks
`FIVEM_EDITION` cannot move to Enhanced.

Changing the build or the edition sets the variable, moves the server onto the image when it is
not on it (`runtime::adopt`: Wings recreates the container with it at the next start) and restarts
the server when it runs and the user may restart it (`control.restart`). Otherwise the change
applies at the next start. Following a channel never restarts: it is a promise about future
starts.

## Settings

Admin › Settings › Hosting › FiveM (`shared/src/settings/games.rs`, the `games` section):

| Setting | Default | Means |
|---|---|---|
| Image | empty | The image FiveM servers run on (one word, like `ghcr.io/example/fivem:tag`). Empty leaves servers on their egg's image and never moves them |
| FiveM for GTAV Enhanced | off | Whether players may switch to the Enhanced edition (the GTAV Enhanced page then warns that it is early access; off, it says Enhanced is not available yet and offers no switch). Its txAdmin needs `io_uring`, which Docker's default seccomp profile forbids: turn it on once the nodes allow it. Switching back to Legacy is always possible |

## Server Configuration

`/server/<id>/fivem/config`, permission `fivem.config` (the form shows the license key and the
database password, so reading needs it too). GET and PUT `/api/client/servers/<id>/fivem/config`.

The file is the `server.cfg` txAdmin starts: txAdmin's `txData/default/config.json`
(`server.dataPath` + `server.cfgPath`), else `server.cfg` in the server's root. Without one ("txAdmin
has not set this server up yet") the page says so.

The form covers:

- **Identity**: `sv_hostname`, `sets sv_projectName`, `sets sv_projectDesc`, `sv_maxclients`
  (1–2048), `sets locale`, `load_server_icon`, `sets tags`;
- **Network and build**: the `endpoint_add_tcp`/`_udp` lines (shown, managed by the panel's
  allocations), OneSync (managed by txAdmin, which comments out any `onesync` line), `set
  sv_enforceGameBuild` (GTA V's builds with their DLC name, RedM's for RedM);
- **Database**: `set mysql_connection_string`, as a URI (`mysql://user:password@host:port/name`) or
  `key=value` (`server=…;database=…;userid=…;password=…`), with a picker that fills it from the
  server's own MySQL databases (Databases page, needs `databases.read`);
- **Keys**: `sv_licenseKey`, `set steam_webApiKey`, `sv_tebexSecret`;
- **Resources** (`ensure` lines; a disabled one is commented out, not removed), **Group
  memberships** (`add_principal`) and **Access control** (`add_ace … allow|deny`), as rows, folded
  after 25.

Each group is an aside section like the Pelican page's (heading and description beside a card of
fields, stacked on phones), with the field hints under the fields; the one filled button is
**Save changes** under the form.

Saving rewrites only what changed: a setting's line keeps its place, its comment and its quoting
style; a new one goes before the first line that loads resources (`ensure`, `start`, `exec`, …),
else at the end; emptying a setting, or removing a row, removes its line. Every other line stays
byte for byte.
Names in rows may use letters, numbers and `. _ - : @ [ ]` (resources no `:`); a row with anything
else is refused with its list and row number. The form loads with a hash of the file (the first 16
bytes of its SHA-256), and a save whose hash no longer matches (txAdmin, the file editor or another
tab changed it) is refused with 409: reload and change it again. A save that changes nothing says
"Nothing changed" instead of saved. The page header has **Edit as text** (the whole file in an editor
in a modal, saved through Wings; needs `files.read-content` and `files.update`) and a restart button
(needs `control.restart`; a start gate that holds the start opens its page, as the console's power
buttons do).

FXServer reads the file when it starts, so the toast says to restart. The activity log records
`server:fivem.config.update` with the names of the changed settings and lists, never their values.

## Frameworks

`/server/<id>/fivem/frameworks`, permission `fivem.frameworks` (installing restarts the server, and
can create a database). GET `/api/client/servers/<id>/fivem/frameworks` (the recipes, the current
folder, the install), POST the same with `{"recipe": "<id>", "server_name", "license", "database"?:
{"mode": "new"} | {"mode": "existing", "uuid"}}` (202), DELETE `…/frameworks/install` to clear a
finished install's outcome.

**The list** is txAdmin's own recipe index (`tabarra/txAdmin-recipes` `indexv4.json`, what txAdmin's
setup shows under "Popular recipes"): the engine-3 entries tagged for the server's game (`fivem`, or
`redm` when `server.cfg` says `gamename rdr3`), each recipe's YAML fetched and parsed (with
`serde-saphyr`) for its name, version, author and whether it needs a database. Index and recipes are
cached for 10 minutes plus the last good copy for a day. A recipe is named by the start of the
SHA-256 of its index URL and looked up in the index again when installing, so nothing a browser
sends becomes a URL the panel fetches.

**Installing** asks what txAdmin's deployer asks: a server name, the Cfx license key (filled in from
the current `server.cfg` when the user may read it, `fivem.config`) and, for a recipe with
`connect_database` or `query_database` tasks, a database: a new one from the server's database slots
(needs `databases.create`, a free slot and a MySQL host on the node) or one of the server's MySQL
databases (`databases.read`). One install per server at a time. It runs as a background task:

1. **prepare**: a new folder `txData/<Name>_<XXXXXX>.base` (txAdmin's naming), the database (a new
   one is logged as `server:database.create`), and txAdmin's variables: `serverName`, `svLicense`,
   `maxClients` (48), `serverEndpoints` (the primary allocation's port, marked "do not modify"),
   `addPrincipalsMaster` (txAdmin's master admin's linked identities, from `txData/admins.json`),
   the `recipe*` ones and, with a database, `dbHost`, `dbPort`, `dbUsername`, `dbPassword`, `dbName`
   and `dbConnectionString` (`mysql://…?charset=utf8mb4`, the host as players' servers reach it);
2. **tasks**: the recipe's tasks in order, while the server keeps running: `download_github`,
   `download_file`, `unzip`, `move_path`, `copy_path`, `remove_path`, `ensure_dir`, `write_file`,
   `replace_string` (template, `all_vars`, literal), `load_vars`, `connect_database`,
   `query_database` and `waste_time`. Paths stay inside the new folder. Wings does the work
   (downloads with its pull, extracts, renames, copies); the panel only lists folders and edits small
   text files, and runs the SQL as the database's own user. Each task has the recipe's timeout,
   60 seconds to 30 minutes;
3. **config**: `server.cfg`'s `{{variables}}` filled;
4. **restart**: txAdmin's config is read (it names the previous folder), the server stops (killed
   after a minute), txAdmin is pointed at the new folder (with the recipe's `$onesync`), the server
   moves onto the FiveM image when it is set, and it starts again.

The page (titled **Framework installer**) lists the recipes as rows with their logo, version,
author and description, and an Install button each (disabled with the reason as a tooltip when
something is missing). The page polls the install every 3 seconds and shows a card with a progress
bar and each step; a finished one says where the new
and the previous files are, and when txAdmin has no admin yet, how to register one. The activity log
records `server:fivem.framework.start` and, when done, `server:fivem.framework` with the folder.

**Nothing is lost:** the folder txAdmin ran before is not touched. A failed install removes the new
folder and the database it created, and txAdmin keeps running the previous folder. An install that
has not moved for 40 minutes (the panel restarted under it) counts as failed.

## Players

`/server/<id>/fivem/players`, permission `fivem.players`. GET `/api/client/servers/<id>/fivem/players`.

The panel asks FXServer's own HTTP endpoints on the game port, `/dynamic.json` (hostname, player
count, slots) and `/players.json` (id, name, ping, identifiers), 4 seconds each, at the primary
allocation's alias, else its IP, else the node's host for an allocation bound to every address. They
are what the server browser reads, public by design. Since 2023 FXServer leaves identifiers out
unless `sv_exposePlayerIdentifiersInHttpEndpoint` is set, so they are shown when there. Colour codes
(`^0`–`^9`) are stripped. The page refreshes every 12 seconds (and on **Refresh**); it shows
online/offline with the count, a search by name, server id (`#12`) or identifier, and a row per
player with its ping (amber from 150 ms). A server that does not answer is "not answering".

## Artifacts

`/server/<id>/fivem/artifacts`, permission `fivem.artifacts` to change (reading the page needs it
too). GET `/api/client/servers/<id>/fivem/artifacts`; PUT the same with
`{"mode": "recommended" | "latest" | "pin" | "build", "build"?: n}`.

What Cfx offers comes from the same two sources `fivem-runtime` reads: the changelog API
(recommended, latest, and each build's end of support) and the download page's `__NEXT_DATA__`
(every Linux build Cfx still serves, and the Enhanced build). Both are cached for 10 minutes, and
the last good answer for a day on top, so Cfx having a bad minute shows the last list instead of
an empty page. Only Cfx's own download hosts are accepted.

The page shows a card with what is installed (from the marker) and since when, the build the next
start installs when it differs, and the update switch (Follow Recommended, Follow Latest, Stay on this
build), then a search by build number and the builds as rows with Recommended / Latest / Installed /
Out of support badges and their support end. **Install** on a build (a gray button, confirmed) pins
`FIVEM_VERSION` to it and restarts the server (when it runs and the user may restart). A build Cfx
does not list is refused. "Stay on this build" needs a known installed build. While a server runs
Enhanced, the switch and the builds are hidden (Enhanced always runs Cfx's current Enhanced build)
and a note links to the GTAV Enhanced page. The activity log records `server:fivem.version`.

## GTAV Enhanced

`/server/<id>/fivem/enhanced`, permission `fivem.artifacts` (as Artifacts, whose data it reads: GET
`/api/client/servers/<id>/fivem/artifacts`). PUT `…/fivem/artifacts/edition` with
`{"edition": "legacy" | "enhanced"}`.

Listed on every FiveM server, like the Pelican panel's page: with the Enhanced setting off (it ships
off) it says Enhanced is not available yet and still explains what will change; on, it warns that
Enhanced is early access at Cfx.re. A card shows the edition the server runs (and Enhanced's build),
a pending switch that has not reached the server yet, and the page's one filled button, the switch,
with a confirmation. Moving to Enhanced needs the setting and the egg's `FIVEM_EDITION`; going back to
Legacy is always offered. Below, what changes for players, scripts, assets, networking and txAdmin.
A RedM server gets a note that RedM has no Enhanced edition. Switching sets `FIVEM_EDITION` and
restarts like a build change; the activity log records `server:fivem.edition`.

## Differences from Pelican

- No relay for GitHub archives: Pelican's Wings needed a `Content-Length`, so the panel relayed
  GitHub's archives. wings-rs downloads without one.
- The install is one background task instead of queued 50-second chunks, and its outcome shows on
  the page and as a toast instead of in the notification bell.
- The sidebar keeps the Pelican panel's order, but has no "More" group: Artifacts and GTAV Enhanced
  are plain entries after Server Configuration.
- Server Configuration is editable with the one `fivem.config` permission (Pelican shows it
  read-only to subusers without the file update permission).
- With the Enhanced setting on but an egg without `FIVEM_EDITION`, GTAV Enhanced says "not available
  yet" (Pelican shows its early-access warning and no switch).
- The image is a setting (Admin › Settings › Hosting › FiveM) instead of `.env`, and servers move
  onto it when their build or edition changes instead of with a command (`plugged-host:fivem-setup
  --servers`). New servers get it from the egg (`docs/eggs/fivem.json`).

## Checks

- A FiveM server (egg with `cfx`) shows Frameworks, Players, Server Configuration, Artifacts and
  GTAV Enhanced, in that order; a Minecraft server shows none of them, and
  `/api/client/servers/<id>/fivem/config` answers 404 there.
- Server Configuration: change the hostname and one resource, save, open `server.cfg` in the file
  editor: only those lines changed. Edit the file in the editor, then save the form from an
  older tab: refused, reload asked for. Save again without changing anything: "Nothing changed".
  **Edit as text** opens the file in a modal; saving it updates the form.
- The database picker fills host, port, user, name and password from a MySQL database.
- Players: join the server; the player shows within 12 seconds, and the search finds it by name and
  by `#id`. Stop it: "not answering".
- Artifacts: with the image set, switch to Follow Latest (no restart), install an older build on a
  running server (restarts, the marker shows the build after the start).
- GTAV Enhanced: with FiveM for GTAV Enhanced off in Admin › Settings, the page says it is not
  available yet and offers no switch; a server on Enhanced still offers switching back. On, the
  page warns about early access and switching asks for confirmation, then Artifacts shows the
  Enhanced note with its link back.
- Frameworks: install ESX Legacy with a new database on a running server: the steps count up, the
  server restarts at the end on `txData/ESXLegacy_….base`, the old folder is still there, the
  database shows on the Databases page. Install one with a bad license key on a server with no
  database slot: refused before anything starts. Make a task fail (remove the node's network
  mid-download): the new folder and database are gone, txAdmin runs the old folder.
