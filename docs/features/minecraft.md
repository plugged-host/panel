# Minecraft

Minecraft servers get their own pages in the server sidebar. This page grows with each part:
**Plugins & Mods** is here; the game settings (`server.properties`, Paper and Spigot YAML), players
(whitelist, operators, bans), worlds, Bedrock support (Geyser + Floodgate), modpacks, the version
switch and the log check follow.

The code is in `shared/src/minecraft/` (sources, versions, installed jars), the routes in
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

## Plugins & Mods

`/server/<id>/minecraft/content`, permission `minecraft.content` (browse, install, update, turn
off, delete). Two tabs:

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
(`files/pull`, the upstream pull rate limit) into `/plugins` or `/mods`, retried while Wings is
busy with another pull. Modpacks are installed by the onboarding and the version switch, not here.

Every search, project and version answer is cached (searches 5 minutes, the rest an hour,
`minecraft::<source>::…`); a source that fails answers "the content source did not answer, try
again later" (the cause is logged).

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

## Without noprofit.space

Nothing here uses noprofit.space.

## What to check

- A Paper server: Plugins & Mods appears after Files; a FiveM or other server does not show it.
- Installed: plugins identified (hash, name), a turned-off jar shows Disabled, an old plugin offers
  Update, a plugin without its dependency shows Needs.
- Browse: search, switch source and version, Install and Choose Version; the file appears under
  Installed and in `/plugins`.
- Admin › Settings › Hosting › Minecraft: store a CurseForge key, CurseForge appears as a source.
