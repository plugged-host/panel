# FiveM

FiveM and RedM servers get their own pages in the server sidebar: **Server Configuration** (the
`server.cfg` txAdmin runs, as a form), **Players** (who is on the server now) and **Artifacts** (the
FXServer build, how it updates, and FiveM for GTAV Enhanced).

The code is in `shared/src/fivem/` (`server_cfg.rs` reads and edits `server.cfg` in place,
`config.rs` the form over it, `txadmin.rs` where txAdmin keeps its files, `players.rs`,
`artifacts.rs` what Cfx offers, `runtime.rs` the server's build and edition), the routes in
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
| `FIVEM_EDITION` | `legacy` or `enhanced` | the Artifacts page (Enhanced section) |

After installing, it writes `alpine/.plugged-host.json` (`{"edition", "build", "installed_at"}`),
which is how the Artifacts page knows what is installed. The image's source is
`plugged-host-fivem/image/` in [plugged-host/plugins](https://github.com/plugged-host/plugins), and
`docs/eggs/fivem.json` runs on it. A server whose variables the egg lacks shows the page without the
choices ("its build cannot be changed here").

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
| FiveM for GTAV Enhanced | off | Whether players may switch to the Enhanced edition. Its txAdmin needs `io_uring`, which Docker's default seccomp profile forbids: turn it on once the nodes allow it. Switching back to Legacy is always possible |

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

Saving rewrites only what changed: a setting's line keeps its place, its comment and its quoting
style; a new one goes before the first line that loads resources (`ensure`, `start`, `exec`, …),
else at the end; emptying a setting, or removing a row, removes its line. Every other line stays
byte for byte.
Names in rows may use letters, numbers and `. _ - : @ [ ]` (resources no `:`); a row with anything
else is refused with its list and row number. The form loads with a hash of the file (the first 16
bytes of its SHA-256), and a save whose hash no longer matches (txAdmin, the file editor or another
tab changed it) is refused with 409: reload and change it again. **Edit as Text** opens the file in
the file editor (needs `files.read-content` and `files.update`).

FXServer reads the file when it starts, so the toast says to restart. The activity log records
`server:fivem.config.update` with the names of the changed settings and lists, never their values.

## Players

`/server/<id>/fivem/players`, permission `fivem.players`. GET `/api/client/servers/<id>/fivem/players`.

The panel asks FXServer's own HTTP endpoints on the game port, `/dynamic.json` (hostname, player
count, slots) and `/players.json` (id, name, ping, identifiers), 4 seconds each, at the primary
allocation's alias, else its IP, else the node's host for an allocation bound to every address. They
are what the server browser reads, public by design. Since 2023 FXServer leaves identifiers out
unless `sv_exposePlayerIdentifiersInHttpEndpoint` is set, so they are shown when there. Colour codes
(`^0`–`^9`) are stripped. The page refreshes every 12 seconds; a server that does not answer is
"not answering".

## Artifacts

`/server/<id>/fivem/artifacts`, permission `fivem.artifacts` to change (reading the page needs it
too). GET `/api/client/servers/<id>/fivem/artifacts`; PUT the same with
`{"mode": "recommended" | "latest" | "pin" | "build", "build"?: n}`; PUT `…/artifacts/edition` with
`{"edition": "legacy" | "enhanced"}`.

What Cfx offers comes from the same two sources `fivem-runtime` reads: the changelog API
(recommended, latest, and each build's end of support) and the download page's `__NEXT_DATA__`
(every Linux build Cfx still serves, and the Enhanced build). Both are cached for 10 minutes, and
the last good answer for a day on top, so Cfx having a bad minute shows the last list instead of
an empty page. Only Cfx's own download hosts are accepted.

The page shows what is installed (from the marker), the build the next start installs when it
differs, the update mode (follow Recommended, follow Latest, stay on the installed build) and the
builds with Recommended / Latest / Installed badges and their support end. **Install** on a build
pins `FIVEM_VERSION` to it and restarts the server (when it runs and the user may restart). A build
Cfx does not list is refused. "Stay on this build" needs a known installed build.

**FiveM for GTAV Enhanced** is a section of the same page (Pelican had it as its own page): what
changes for players, scripts, assets, networking and txAdmin, and the switch, with a confirmation.
Switching sets `FIVEM_EDITION` and restarts like a build change. While a server runs Enhanced, the
builds list is hidden: Enhanced always runs Cfx's current Enhanced build. The activity log records
`server:fivem.version` and `server:fivem.edition`.

## Differences from Pelican

- No relay for GitHub archives: Pelican's Wings needed a `Content-Length`, so the panel relayed
  GitHub's archives. wings-rs downloads without one.
- Enhanced is a section of Artifacts, not its own page.
- The image is a setting (Admin › Settings › Hosting › FiveM) instead of `.env`, and servers move
  onto it when their build or edition changes instead of with a command (`plugged-host:fivem-setup
  --servers`). New servers get it from the egg (`docs/eggs/fivem.json`).

## Checks

- A FiveM server (egg with `cfx`) shows Server Configuration, Players and Artifacts; a Minecraft
  server shows none of them, and `/api/client/servers/<id>/fivem/config` answers 404 there.
- Server Configuration: change the hostname and one resource, save, open `server.cfg` in the file
  editor: only those lines changed. Edit the file in the editor, then save the form from an
  older tab: refused, reload asked for.
- The database picker fills host, port, user, name and password from a MySQL database.
- Players: join the server; the player shows within 12 seconds. Stop it: "not answering".
- Artifacts: with the image set, switch to Follow Latest (no restart), install an older build on a
  running server (restarts, the marker shows the build after the start).
- With FiveM for GTAV Enhanced off in Admin › Settings, the Enhanced switch is not offered; a
  server on Enhanced still offers switching back.
