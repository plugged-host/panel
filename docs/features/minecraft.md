# Minecraft

Minecraft servers get their own pages in the server sidebar: **Plugins & Mods** (with **Modpacks**
and **Bedrock**), **Game Settings**, **Players** and **Worlds**, plus the **log check** above the
console and the Minecraft version in Settings › Software
([server-settings.md](server-settings.md#software)). A player's new Minecraft server starts with
the [setup](#setup).

The code is in `shared/src/minecraft/` (sources, versions, installed jars, `config.rs`,
`players.rs`, `worlds.rs`, `bedrock.rs`, `modpacks.rs`, `onboarding.rs`, `log_check.rs`, `files.rs` for reading and
writing the server's files through Wings; `shared/src/redact.rs` masks secrets), the routes in
`backend/src/routes/api/client/servers/_server_/minecraft/`, the pages in
`frontend/src/pages/server/minecraft/`.

## Which servers

A server is a Minecraft server when its egg has the `eula` feature (every Minecraft Java egg has
it). Other servers neither show the pages (`frontend/src/lib/domain/games.ts` hides `/minecraft/*`
routes from the sidebar and the router) nor answer the routes (404 "this server does not run
minecraft").

What the server runs is read from:

- **The loader**: the software the server is set to in the games catalog (the egg's software
  variable, [create-server.md](create-server.md)), else the egg's name. Paper, Purpur, Folia,
  Spigot, Bukkit, Pufferfish and Leaf load **plugins** (Paper here); Fabric, Quilt, Forge and
  NeoForge load **mods**. A vanilla server has no loader and nothing to install into.
- **The Minecraft version**: the first of the variables `MINECRAFT_VERSION`, `MC_VERSION`,
  `VANILLA_VERSION` the server has. `latest` is the newest release the loader has builds for:
  PaperMC's Fill API (and Purpur's), Fabric's meta (Quilt uses it too), Forge's promotions and
  NeoForge's Maven metadata, releases only, cached for an hour (`minecraft::versions`).

## Settings

Admin › Settings › Hosting › Minecraft (`shared/src/settings/games.rs`, the `games` section):

| Setting | Default | Means |
|---|---|---|
| CurseForge API Key | none | Offers CurseForge as a source and matches jars against it. Encrypted; the admin API shows a placeholder. Keys come from console.curseforge.com |
| SpigotMC | on | Offers SpigotMC (through the Spiget API) for plugins on Paper servers |
| User Agent | empty | What the panel calls itself to Modrinth, CurseForge, Spiget and the loaders' APIs. Empty: `<app name>-panel/<release> (+<app URL>)`, which is what Modrinth asks for |
| Log Check | on | Checks Minecraft servers' latest log on mclo.gs and shows the problems above the console ([Log check](#log-check)) |

## Plugins & Mods

`/server/<id>/minecraft/content`, permission `minecraft.content` (browse, install, update, turn
off, delete). Three tabs, Installed, Browse and [Modpacks](#modpacks) (only on a server that can run a
mod loader), and the [Bedrock](#bedrock-players) button in the header:

**Installed** lists the jars in `/plugins` or `/mods` (a `.jar.disabled` is a turned-off one) with
what each one is:

1. Wings' SHA-1 of the file, looked up on Modrinth (`Exact match`);
2. Wings' CurseForge fingerprint, looked up on CurseForge when a key is set (`Exact match`);
3. the jar's own descriptor (`plugin.yml`, `paper-plugin.yml`, `fabric.mod.json`,
   `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, the manifest), read
   through Wings without downloading the jar, then the name searched on Modrinth and SpigotMC
   (`Matched by name`), else the descriptor alone (`Read from the jar`).

Wings computes the hashes and reads inside the jar, so the panel never downloads a jar. Reading
inside a `.jar` needs the [Wings fork](../wings.md); on stock wings-rs the descriptor step is
skipped. Up to 8 unknown jars are identified per request and the result is kept for 30 days per
file name and size (`minecraft::jar::<server>::<hash>`), so the page asks again until
"Identifying N more…" is gone.

A row shows the name, the file, where it was found, the version and the size; badges for
**Disabled**, **Update: x** (Modrinth's newest build of that file for the server's loader and
version) and **Needs y** (a plugin's `depend` or a mod's required dependency that no installed jar
provides). Its menu: Update to x (downloads the new build, keeps `.disabled`, deletes the old one),
Enable/Disable (renames to or from `.jar.disabled`), Open on the source, Delete (with "Also delete
its folder" for a plugin's data folder). A change applies on the next restart.

**Browse** searches Modrinth (always), CurseForge (with a key) and SpigotMC (Paper only, when on):
plugins for Paper, mods for the others, for the server's Minecraft version or any version, 20 per
page. **Install** takes the newest release that fits the loader and version (the newest build when
there is no release); **Choose Version** lists every build, or every build at all when none lists
the server's version ("loose"). SpigotMC only downloads the latest build, and premium or externally
hosted resources cannot be installed (**External**). The file is pulled by Wings
(`files/pull`, the upstream pull rate limit) into `/plugins` or `/mods`, retried for a few seconds
while Wings already runs as many pulls of the server as it allows (`limits.server_concurrent_pulls`,
3 by default; Wings answers 417 "too many concurrent pulls"). Modpacks have their own tab.

Every search, project and version answer is cached (searches 5 minutes, the rest an hour,
`minecraft::<source>::…`); a source that fails answers "the content source did not answer, try
again later" (the cause is logged).

## Modpacks

`/server/<id>/minecraft/content/modpacks`, shown when the server can run a mod loader: its game's
software choices include Fabric, Quilt, Forge or NeoForge (a Paper server whose game also offers
Fabric qualifies), or, without choices, it runs one of them. Searching needs `minecraft.content`;
installing also needs `settings.software`, since a pack brings its own loader and version and the
server is reinstalled for it.

The tab searches Modrinth's and (with a key) CurseForge's modpacks for those loaders, any Minecraft
version. **Install** asks for the pack's version (it shows the Minecraft version and loader it is
for) and whether to **remove the mods installed now** (on by default: the `mods` folder goes to the
trash when the server has one, else it is deleted). The server has to be stopped. The request
checks the pack, stores the install as started and answers 202; a background task then:

1. downloads the pack into the panel (Modrinth: the `.mrpack` from `cdn.modrinth.com`; CurseForge:
   the pack zip through its download URL), at most 256 MB;
2. reads `modrinth.index.json` (the `minecraft` and loader dependencies, the files with
   `env.server` other than `unsupported`, their first download on a host the format allows:
   `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com`) or CurseForge's
   `manifest.json` (the primary mod loader, the required files by project and file id);
3. when the server's software choice or Minecraft version differ from the pack's, switches them like
   Settings › Software does (writing `BUILD_NUMBER` back to `latest`) and waits up to 30 minutes for
   the reinstall; the loader's own build is the newest the egg installs for that version (no egg
   variable pins an exact Forge or Fabric loader build);
4. removes the old mods when asked;
5. has Wings download every file into its folder, three at a time, each waited for. A file that
   fails (a dead link, a CurseForge author who forbids downloads by other apps) is listed as
   skipped instead of failing the pack; CurseForge files that are not `.jar` (resource and shader
   packs, which only clients load) are left out;
6. unpacks the overrides: `overrides/` then `server-overrides/` (which wins) of a `.mrpack`, the
   manifest's `overrides` folder of a CurseForge pack, zipped again by the panel and unpacked by
   Wings at the server root. More than 256 MB of overrides are left out and the page says to upload
   them in Files.

Paths in a pack never leave the server's folder (absolute paths, `..` and drive letters are
dropped). The state (step, files done of total, skipped files, error) is kept in the cache, for 2
hours while running and an hour after; the tab polls it every 3 seconds, shows the step with a
progress bar and toasts when it ends. Activity: `server:minecraft.modpack.install` when it ends.

## Bedrock players

The **Bedrock** button on Plugins & Mods (permission `minecraft.bedrock`), on Paper and its forks,
Fabric and NeoForge (Geyser has no build for Quilt or Forge). The dialog shows the installed Geyser
jar and the address Bedrock players join, and **Install Geyser** / **Update Geyser**
(`shared/src/minecraft/bedrock.rs`):

1. **The port.** The server's allocation noted `Bedrock (Geyser)`, else (a Geyser set up by hand) a
   non-primary allocation on the port its config listens on; else the panel takes the node's lowest
   free port on the primary allocation's IP (ports something on the node already listens on are
   skipped) and adds it to the server with that note, under the server's allocation lock. It does
   not count against the server's allocation limit. Wings maps every allocation for TCP and UDP, so
   Bedrock's UDP listener is reachable. No free port: 409 "this node has no free port left for
   bedrock".
2. **The jars.** Old Geyser and Floodgate jars are deleted; Geyser is pulled from GeyserMC's build
   API (`download.geysermc.org/v2/projects/geyser/versions/latest/builds/latest/downloads/<platform>`,
   `spigot` for the Paper family) as `Geyser-<Platform>.jar`, Floodgate from GeyserMC for the Paper
   family and from Modrinth (project `floodgate`, for the server's version) for Fabric and NeoForge.
   Without a Floodgate build, Bedrock players sign in with a Java account (`auth-type: online`) and
   the dialog says so.
3. **The config.** An existing `plugins/Geyser-Spigot/config.yml` (`config/Geyser-Fabric/…`,
   `config/Geyser-NeoForge/…`) gets `bedrock.port`, `remote.port` (or `java.port` in newer configs)
   and `auth-type` set, comments kept; without one the panel writes a short config with those and
   `address: auto`, and Geyser adds its defaults on start.
4. A running server is restarted after 5 seconds (the downloads run in Wings' background).

Activity: `server:minecraft.bedrock.install`.

## Setup

A Minecraft server a player creates ([create-server.md](create-server.md)) is not started after its
install. It gets a `server_onboardings` row instead, and its owner is sent to **Set Up Your
Server** (`/minecraft/setup`) from every page of the server, also while it installs (the page is
the one a server that installs may show besides the console). Servers an admin creates have no row
and never see it. The page asks, as choice tiles like the create page:

- **What to run** (when the server can take a modpack): a server, with its software (the game's
  choices) and Minecraft version, or a modpack from the [Modpacks](#modpacks) sources and its
  version;
- **Bedrock players** (when the chosen software, or the pack's loader, runs Geyser): Java only, or
  Java and Bedrock;
- **Accounts**: bought accounts (`online-mode=true`) or any name;
- **Who can join**: anyone, or only the players added (the whitelist);
- **Players**: the whitelist (when private) and operators, up to 20 names each.

**Set Up and Start** stores the answers on the row (`completed`, once: a second submit is 409), and
`minecraft::onboarding::apply` runs after the request: it waits for the install to end (30 minutes
at most), installs the modpack or switches the software and version (a reinstall it waits for
too), writes `online-mode`, turns the whitelist on or off and adds the names (looked up at Mojang in
online mode; names without an account are listed as skipped), installs Geyser when asked, and
starts the server. A part that fails is listed and the rest still applied; an install that failed
stops it ("reinstall it from Settings"). The page then shows each step (polled every 3 seconds,
kept in the cache 2 hours while running and an hour after) and **Go to the Console** when it ends.
Activity: `server:minecraft.onboarding.complete` when it ends.

### The egg

[`docs/eggs/minecraft-java.json`](../eggs/minecraft-java.json) is the Minecraft: Java Edition egg the
pages are made for: one egg for Purpur (Paper below 1.14.1), Fabric, Forge and NeoForge, chosen by
`MC_SOFTWARE`, with `MINECRAFT_VERSION` and `BUILD_NUMBER`, the `eula` feature and a startup command
per kind (jar, old Java, mod loader). Import it in Admin › Nests (a nest's Eggs tab, Import), then add it to
the games catalog (Admin › Settings › Hosting › Games) with `MC_SOFTWARE` as the software variable
and the choices `purpur`, `fabric`, `forge`, `neoforge`, each with its startup command. Other
Minecraft eggs work too; this one is the one the setup and Modpacks switch between softwares in.

## Game Settings

`/server/<id>/minecraft/settings`, permission `minecraft.settings` (read and change; the files hold
secrets such as Paper's Velocity secret, so reading needs it too). One tab per file that is on the
server:

| Tab | File | Shown on |
|---|---|---|
| server.properties | `server.properties` | every Minecraft server |
| bukkit.yml | `bukkit.yml` | plugin servers (Paper and the like) |
| spigot.yml | `spigot.yml` | plugin servers |
| paper-global.yml | `config/paper-global.yml`, else the older `paper.yml` | plugin servers |
| purpur.yml | `purpur.yml` | plugin servers |

A file the server has not written yet has no tab; with none at all the page says to start the
server once. Files are read up to 512 KB.

**server.properties**: every key except the ones the panel manages (`server-ip`, `server-port`,
`query.port`, `rcon.port`, `rcon.password`), as a switch (`true`/`false` keys and the known
toggles), a number (the known numeric keys, or a value that is a whole number), a choice (the
gamemode, difficulty, level type and the like, when the current value is one of the choices) or
text. Numbers and choices are checked before anything is written. Values are read and written
with Java's escapes, as the game writes them (`level-type=minecraft\:normal`, `\u00A7` in an
older `motd`), so the form shows `minecraft:normal` and `§`.

**The YAML files**: every single-line value (true/false, null, numbers, text) by its dotted path,
grouped under its top-level key. Lists, block values and values spread over several lines are not
shown; **Edit as Text** opens the file in the file editor for those (with `files.read-content` and
`files.update`). A value keeps its type: a number stays a number, text that would read as another
type is quoted.

**Save** sends only what changed (up to 1000 values). The panel reads the file again, changes those
lines and nothing else (comments, order, blank lines and keys it does not show stay as they are)
and writes it only when something differs; activity `server:minecraft.settings.update` with the
file and the count. The change applies when the server restarts.

## Players

`/server/<id>/minecraft/players`, permission `minecraft.players`. One list of everyone on the
whitelist (`whitelist.json`), an operator (`ops.json`, with the level) or banned
(`banned-players.json`, with the reason), sorted by name, plus:

- **Only Whitelisted Players**: `white-list` and `enforce-whitelist` in `server.properties` (both
  set together), and `whitelist on`/`off` on the console while the server runs;
- a note when the server is in offline mode (anyone can join under any name) and when it is
  stopped (changes apply at the next start).

**Add Player** takes a name (1 to 16 letters, digits and `_`) and adds it to the whitelist and,
when ticked, as an operator. A row's menu adds to or removes from the whitelist, makes or removes
an operator (making one asks first), bans with an optional reason (up to 200 characters) or pardons.

How a change is made, like the game itself does it:

1. A server in online mode needs a real account: the name is looked up on Mojang
   (`api.mojang.com`, cached a day as `minecraft::profile::<name>`, a miss 5 minutes) and stored
   with Mojang's spelling and UUID; an unknown name is refused. In offline mode the UUID is the one
   the server computes itself (`OfflinePlayer:<name>`, version 3).
2. While the server runs and the caller also has `control.console`, the command is sent
   (`whitelist add`, `op`, `ban <name> <reason>`, `pardon`, …) and the server writes its own file.
3. Otherwise the panel edits the JSON file the way the server writes it (an operator at
   `op-permission-level`, a ban "forever" by "Server"); the server reads it at its next start.

A change that is already in place (adding someone who is on the list) does nothing. Every change is
in the activity log as `server:minecraft.players.{whitelist,unwhitelist,op,deop,ban,pardon}` with
the name, the reason and whether it went through the console; the switch as
`server:minecraft.players.enforce`.

## Worlds

`/server/<id>/minecraft/worlds`, permission `minecraft.worlds`. A world is a folder in the server
root with a `level.dat`; the `<world>_nether` and `<world>_the_end` folders Paper-family servers
keep beside it belong to it. The list shows the loaded world (`level-name` in `server.properties`)
first, also before its first start has generated it, then the other worlds by name, with the
folders and when they last changed. Folders that are never worlds (`plugins`, `mods`, `config`,
`logs` …) and hidden ones are not looked into, and at most 12 folders are.

A row's menu:

- **Download**: one archive (zip) of the world and its dimension folders, streamed by Wings through
  the file manager's download (`files.read-content`). A running server is told `save-all flush`
  first when the caller also has `control.console`, so the archive has the last minutes of play.
- **Load This World**: sets `level-name`; the server loads it from its next start (works while it
  runs).
- **Open in Files**: the world's folder in the file manager.
- **Delete** (not the loaded world): its folders go to the server's trash when it has one, else
  they are deleted.

Two page actions need the server stopped:

- **New World**: name, seed (empty is random), world type, structures, generator settings (for
  superflat and single biome) and data packs, written to `server.properties`; the server generates
  the world at its next start. **Keep the current world** (on by default) renames the loaded world's
  folders to `<world>-<date>` when the new one takes its name, or leaves it as it is under another
  name; off, it is deleted (trash first when there is one). Either way it stays loadable from the
  list, which is what Pelican's "keep a zip copy" became.
- **Upload World** (also `files.create`): empties `.world-upload`, uploads the archive there with the
  file manager's uploader (in parts, resumable, in the uploads card) and, when the upload is done,
  starts the import. The panel unpacks it (Wings, in the foreground of a background task), finds the
  folder with `level.dat` (the top or up to two folders down, `__MACOSX` skipped), keeps or deletes
  the loaded world as above, moves the new one into its place (with `_nether`/`_the_end` siblings)
  and empties `.world-upload`. The page shows "Unpacking …" and polls until it ends; a failure stays
  on the page for an hour. Archives: `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.tar.xz`, `.tar.zst`. The
  import is started by the browser, so the tab has to stay open until the upload is done.

Activity: `server:minecraft.worlds.{create,activate,delete,import}`.

## Log check

Above the console of a Minecraft server, for whoever may read the console and files
(`control.console` and `files.read-content`; `shared/src/minecraft/log_check.rs`), like Aternos does
it. The page asks every minute; the panel then reads `logs/latest.log` through Wings (at most 4 MB),
strips colour codes, masks secrets (`shared/src/redact.rs`: `key=value`, `key: value` and JSON
entries whose key looks secret such as passwords, RCON, tokens, API keys and webhooks; JWTs; Discord
bot tokens; the token half of Discord webhook URLs; `sk-`, `gsk_`, `ghp_`, `xox?-` keys) and uploads
it to mclo.gs (`api.mclo.gs/1/log`), which strips IP addresses itself. mclo.gs's analysis
(`/1/log/<id>?insights=1`) gives the problems and their first suggested fix.

One result per server, kept 24 hours (`minecraft::log_check::<server>`) and shared by everyone
watching. A log is uploaded only when it changed (MD5), a running server's at most every 2 minutes,
a stopped server's right away (the log with the crash); one upload at a time per server. A clean log
shows one dimmed line with a link; problems show as a card with up to three problems and their fixes
and the link to the whole log on mclo.gs; error lines without a known problem say so. The "Ask neo"
button comes with Phase 5's AI port. Admin › Settings › Hosting › Minecraft › Log Check turns it off
(the console then shows nothing).

## Routes

Under `/api/client/servers/{server}/minecraft/content`, all with `minecraft.content`:

| Route | Does |
|---|---|
| `GET /` | The loader, its software value, the Minecraft version (as set and as resolved), the sources and kinds offered, the directory |
| `GET /search?source&kind&query&game_version&page` | One page of projects (`game_version` empty = any version, absent = the server's) |
| `GET /versions?source&project&kind` | The builds that fit, newest first, and whether the list is loose |
| `POST /install` `{source, project, version?}` | Pulls the file; 202 with the file and directory; activity `server:minecraft.content.install` |
| `GET /installed` | The jars and how many are still being identified |
| `DELETE /installed` `{file, with_data}` | Deletes the jar (and the plugin's folder); activity `server:minecraft.content.delete` |
| `POST /installed/toggle` `{file, enabled}` | Renames to or from `.disabled` |
| `POST /installed/update` `{file}` | Replaces the jar with Modrinth's newest build; 202 |

Under `/api/client/servers/{server}/minecraft/settings`, with `minecraft.settings`:

| Route | Does |
|---|---|
| `GET /` | The files on the server, each with its fields (path, kind, value, choices) |
| `PUT /` `{file, edits: [{path, value}]}` | Changes those values; answers how many changed |

Under `/api/client/servers/{server}/minecraft/players`, with `minecraft.players` (the console is
used only with `control.console` too):

| Route | Does |
|---|---|
| `GET /` | The players, whether the whitelist is on, online mode, the operator level, whether the server runs |
| `POST /whitelist`, `DELETE /whitelist` `{name}` | Adds to or removes from the whitelist; answers whether it applied live |
| `POST /operators`, `DELETE /operators` `{name}` | Makes or removes an operator |
| `POST /bans` `{name, reason?}`, `DELETE /bans` `{name}` | Bans or pardons |
| `PUT /enforce` `{enabled}` | Turns the whitelist on or off |

Under `/api/client/servers/{server}/minecraft/worlds`, with `minecraft.worlds`:

| Route | Does |
|---|---|
| `GET /` | The worlds, the generation settings, the world types, whether the server runs, whether deleting uses the trash, the last import's state |
| `POST /` `{name, seed, level_type, structures, generator, datapacks, keep}` | A new world in place of the loaded one (server stopped); answers what was kept or deleted |
| `DELETE /` `{world}` | Deletes a world that is not the loaded one |
| `PUT /active` `{world}` | Loads that world from the next start |
| `POST /save` | `save-all flush` on a running server (also `control.console`) |
| `POST /upload` | Empties the upload folder (also `files.create`, server stopped) |
| `POST /import` `{archive, keep}` | Starts the import of the uploaded archive; 202 (also `files.create`) |

Under `/api/client/servers/{server}/minecraft/modpacks` (`minecraft.content`):

| Route | Does |
|---|---|
| `GET /` | The loaders a pack may be for, the sources, whether the server runs, the current install |
| `GET /search?source&query&page` | One page of modpacks for those loaders |
| `GET /versions?source&project` | The pack's versions, newest first |
| `POST /install` `{source, project, version, clean}` | Starts the install (also `settings.software`, server stopped); 202 with the install |

`/api/client/servers/{server}/minecraft/bedrock` (`minecraft.bedrock`): `GET` whether Geyser runs on
the software, the jar, the port and address; `POST` installs or updates (`{updated, floodgate, port,
address, restarted}`).

`/api/client/servers/{server}/minecraft/onboarding`: `GET` (`minecraft.settings`) whether the setup
waits, when it was answered, the progress, and which software choices and loaders run Geyser (404
for a server without one); `POST` the answers `{variant, version, modpack: {source, project,
version}, bedrock, online_mode, private, players, operators}` (`minecraft.settings` +
`minecraft.players`, plus `settings.software` to change the software, `minecraft.content` for a
modpack, `minecraft.bedrock` for Bedrock); 202, 409 when already answered.

`GET /api/client/servers/{server}/minecraft/log-check` (`control.console` + `files.read-content`):
`{enabled, result}`, the result being `{id, url, errors, problems: [{message, solution}], checked}`;
502 when the log could not be checked.

## Without noprofit.space

Nothing here uses noprofit.space.

## What to check

- A Paper server: Plugins & Mods appears after Files; a FiveM or other server does not show it.
- Installed: plugins identified (hash, name), a turned-off jar shows Disabled, an old plugin offers
  Update, a plugin without its dependency shows Needs.
- Browse: search, switch source and version, Install and Choose Version; the file appears under
  Installed and in `/plugins`.
- Admin › Settings › Hosting › Minecraft: store a CurseForge key, CurseForge appears as a source.
- Game Settings on a stopped Paper server: tabs for server.properties and the YAML files that
  exist; change a switch, a number and a choice, Save, and the file in the editor keeps its
  comments; a wrong number is refused; Edit as Text opens the file.
- Players on a running server: add a player (whitelist + operator), ban with a reason, pardon,
  remove from the whitelist; the console shows the commands and the lists update. The same on a
  stopped server writes the JSON files ("applies when the server starts").
- The whitelist switch on and off; `server.properties` shows `white-list` and `enforce-whitelist`.
- An unknown name on an online-mode server is refused; any name works in offline mode.
- A subuser without `minecraft.settings` or `minecraft.players` does not see those pages.
- Game Settings: `level-type` shows as a choice (Normal etc.) on a server the game wrote
  `minecraft\:normal` into; a MOTD with `§` colours saves and the server shows the colours.
- Worlds on a stopped server: New World with a seed and Keep on, the old world is listed as
  `world-<date>`; Load This World on it and start, the old world is back.
- Upload World: a zipped world (with and without a wrapping folder) becomes the loaded world; the
  uploads card shows the progress, then "Unpacking …", then the list updates.
- Download on a running server: the zip has the world, `world_nether` and `world_the_end`.
- Delete a world that is not loaded: it shows up in the file manager's trash.
- Modpacks on a stopped Fabric server: install a Modrinth pack for another Minecraft version; the
  page shows "Reinstalling the server…", then the mods count up, then "is installed"; the server
  starts with the pack's mods and configs; Settings › Software shows the pack's version.
- The same with a CurseForge pack (with a key) on a Forge server; a pack whose author blocks
  downloads lists the skipped mods.
- Modpacks on a running server: Install is disabled and the page says to stop it.
- Bedrock on a Paper server: Install Geyser; Network shows a new port noted "Bedrock (Geyser)";
  `plugins/` has Geyser and Floodgate; a Bedrock client joins on the shown address. Update Geyser
  keeps the same port.
- Bedrock on a Fabric server: Geyser-Fabric and Floodgate from Modrinth in `mods/`.
- The log check: start a server with a broken mod; above the console a card lists the problem and
  its fix, with a link to mclo.gs where passwords in the log show as `••••`. A clean start shows the
  one dimmed line. Turning Log Check off in Admin › Settings hides it.
- Create a Minecraft server as a player: the server page goes to Set Up Your Server while it
  installs; the console and other pages lead back to it. Pick Fabric and an older version, Java and
  Bedrock, any name, private with two names and an operator: after Set Up and Start the page shows
  waiting, reinstalling, settings, Bedrock, starting, then done; `server.properties` has
  `online-mode=false`, `white-list=true`, the lists hold the names, Geyser is in `mods/` and the
  server runs. A made-up name in online mode is listed as not added.
- The same with a modpack: the pack installs, then the settings are written and the server starts.
- A server an admin creates starts after its install as before and has no setup.
