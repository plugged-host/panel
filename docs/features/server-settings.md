# Server settings for players

A server's settings are one page, **Settings**, laid out like v3's own pages (Transfer, Limbo, the
account's Discord and Partners pages), not like the Pelican panel's settings page: the title, then
one column of sections, each a heading on the page with one box under it. Settings shows in the
sidebar for anyone holding one of its sections' permissions (`settings.*`, `files.sftp` or
`backups.read`). **Startup** and **VS Code** are pages of their own with their own sidebar rows,
like every other server page.

| Section | Anchor | Permission | Shown when |
|---|---|---|---|
| Server Information | `#general` | any of Settings' | always: the icon (`settings.change-icon` to change it), name and description (saved on leaving the field, `settings.rename`), UUID, identifier and node |
| SFTP Information | `#sftp` | `files.sftp` | always |
| Resources | `#resources` | `settings.resize` | Admin › Settings › Hosting › Games › "Players Resize Servers" is on, or the viewer is an admin |
| Limits | `#resources` | any of Settings' | in Resources' place for whoever cannot resize ([below](#server-information)) |
| Version | `#version` | `settings.software` | the server's egg is a game in the catalog with at least two software choices, or a Minecraft egg with a version variable |
| Behaviour | `#behaviour` | `settings.auto-kill`, `.auto-start`, `.timezone` or `.trash` | Calagopus' own auto-start, timezone, auto-kill and trash settings as one form ([below](#behaviour)); the trash only where Admin › Settings lets servers change it |
| Notifications | `#notifications` | `backups.read` | always |
| Danger Zone | `#danger` | `settings.install` or `settings.delete` | Reinstall Server, and Delete Server when deleting is allowed (below) |

The page is `pages/server/settings/ServerSettings.tsx` (upstream's file; its container, registry and
extension slots stay), with one file per section (`InformationSection`, `SftpSection`,
`ResourcesSection`, `VersionSection`, `BehaviourSection`, `NotificationsSection`, `DangerSection`),
each in `SettingsSection.tsx`. Which sections a person gets is `useSettingsSections.ts`. The page
waits for the first answers that decide or fill its sections (`GET …/settings/resize`,
`…/settings/software`, `…/settings/notifications`) before it shows them, so nothing moves when they
arrive.

### Layout

Every part comes from a page that already has it:

- **Header**: the title alone. There are no page actions; Delete Server is in Danger Zone.
- **Section** (`SettingsSection.tsx`): the heading (`.theme-eco-heading`) and, where the section has
  one, a line under it (`.theme-eco-muted`), then the section's one box 0.75 rem below; sections
  1.5 rem apart. As Transfer's "Choose a location" and the Discord page's sections.
- **A box of fields** is `.theme-eco-card` (a fill without a border, children 1.25 rem apart):
  Server Information, Resources, Version, Behaviour, Notifications. Fields side by side in
  `grid gap-4 md:grid-cols-2` (CPU, memory and disk in three from `lg`), helper text under the input
  as on the Startup page (so fields side by side keep their inputs in line), switches with their
  label and description.
- **A box of rows** is `.theme-eco-list` with `elements/SettingRows.tsx` (label, value, copy or link
  icons at the end, hairlines between), as on the VS Code page: SFTP Information, Limits. Server
  Information ends with its UUID, identifier and node rows inside its card.
- **Rows with an action** are `.theme-rows` / `.theme-row` (title and text, the button at the end,
  under the text on a phone), as the Store's perks: Danger Zone.
- **Saving**: what stands alone saves at once (name and description when they lose focus, the
  Notifications switches when they flip, each with a toast); a form of several values has one filled
  button under its fields, start-aligned ("Save resources", Behaviour's "Save"). At most one filled
  button per section.
- **Destructive**: "Change and reinstall", "Reinstall" and "Delete Server" are gray; the red is in
  their confirmation dialogs.
- **Anchors**: each section has its anchor as `id`. `/settings#<anchor>` opens the page at that
  section (a `scroll-margin` keeps it clear of the phone's menu button), and the old addresses of
  the sections, from when each was a page of its own (`/settings/sftp`, `/settings/resources`,
  `/settings/version`, `/settings/behaviour`, `/settings/notifications`, `/settings/danger`),
  redirect there (`ServerSettingsRedirect.tsx`). There is no jump bar.
- **Phones**: one column; the field grids become one column and Danger Zone's buttons go under
  their text.

## Server Information

The card starts with the icon ([below](#server-icon)), then Server Name and Server Description side
by side, each saved when it loses focus ("Updated Server Name"), like Pelican's live fields. Under
them, rows with the UUID and the identifier (copy buttons) and the hosting node with the start of its
UUID (nodes have no number here). When the viewer cannot resize, the **Limits** section takes
Resources' place: rows with CPU, memory, disk, and backups, databases and allocations as
"used / limit" (∞ for none) from `GET …/settings/usage`; a count the viewer may not see (no
`backups.read`, …) shows the limit only.

## Server icon

`shared/src/models/server/icon.rs`, shaped like upstream's account avatar. Server Information
starts with the icon at 64 px, with the gray buttons **Upload Icon** and, when the server has an icon
of its own, **Delete Icon** beside it; both need `settings.change-icon` (Pelican's permission of that
name, which the importer now keeps). Without an icon of its own the server shows its game's: the
icon of its egg's game in Admin › Settings › Hosting › Games, what the Pelican panel calls the egg's
icon, with "Using Egg Icon" next to it. With neither, no picture (as on the Pelican panel), and
without the permission no buttons.

Upload Icon opens a dialog with a file field and upstream's avatar editor (pick the square); the
upload is checked like an avatar (PNG, JPEG, WebP or GIF, 64 to 4096 px a side, the request at
most 2 MB), cut to its centred square, stored as a 256 px lossless WebP at
`avatars/servers/<server>/<random>.webp` in the panel's storage (served as immutable, so every
upload gets a new name; the old file is removed) and its path kept in `servers.icon` (migration
`server_icons`). Deleting it needs no confirmation, as on the Pelican panel. Deleting the server
removes its icon file. Toasts "Server icon updated" / "Server icon deleted"; the activity log gets
`server:settings.update-icon` / `server:settings.delete-icon`.

The client API's server has `icon` (a link, or `null`) and `egg_icon` (the game's link, or
`null`). Where the Pelican panel shows a server's icon, so does this panel: the server list cards
(Theme v2's 2.5 rem icon before the name) and the server sidebar's server card (Filament's tenant
menu shows the server's avatar there). Pelican's "URL" tab (import an icon from a link) is left
out: the panel would fetch any address a player gives it.

## SFTP

Rows for the connection (`host:port`), the username (`<user>.<server>`) and the password: "Your
Account Password" when noprofit.space checks SFTP passwords (its account API is set up) or the panel
password works, else an SSH key from the account. Copy buttons for host and username; "Launch in
SFTP Client" opens the `sftp://` link. The same password line is used by upstream's SFTP details on
the Files page (`frontend/src/plugins/useSftpPassword.ts`). The public settings carry
`noprofit.sftp_passwords` for it.

## Resources

`shared/src/server_resize.rs`. The public plans (Admin › Plans) as radio cards (Pelican's
CardSelect, four a row from `lg`, as on Create Server: name, the note, the size), the current one
picked and marked "Current plan"; a plan larger than the server plus what the owner has left is "Exceeds your
remaining allowance" and cannot be picked (`plan_fits`). **Advanced** sets the values by hand
instead; it starts on when the server's size is no plan. Each field shows what is left under it
(a value over that says so there), and **Save resources** under the fields sends the
values; it stays off while nothing changed or a value is invalid. Without public plans the fields
show directly.

CPU (percent of one thread), memory and disk, each up to the server's
own size plus what its owner's allowance has left ([create-server.md](create-server.md#allowance));
the owner's allowance counts, also when a subuser resizes.

- Shrinking always works, even for an account over its limit or without a plan, so it can make room.
- Every value is at least 1; 0 (no limit) only where an admin gave the server no limit, and then only
  kept, never taken.
- Growing also needs room on the server's node: allocated memory (with overhead) and disk against the
  node's limits, like a deploy. When the node is full, the answer says to move the server first.
- The owner's account lock (`allowance::{user}`) is held for the check and the write, shared with
  creating servers.
- The new limits go to Wings right away (`sync`), which applies them to the running container.

The server's activity log gets `server:settings.resize` with the old and new sizes.

## Software

`shared/src/server_software.rs`. The game's software choices from Admin › Settings › Hosting ›
Games; a choice's optional description (same place) shows under the field. The current one is read back from the game's variable (an unknown value counts as the first
choice). Switching:

1. writes the choice to the variable,
2. takes the choice's startup command and image (or the egg's first),
3. reinstalls the server without wiping it, and without starting it afterwards (the start goes
   through the start gates).

On a Minecraft server whose egg has a version variable (`MINECRAFT_VERSION`, `MC_VERSION`,
`VANILLA_VERSION`) the section also has **Minecraft version**: Newest release (`latest`) or one of the
versions the chosen software offers (the loader's list from [minecraft.md](minecraft.md#which-servers),
Mojang's releases for Vanilla; `GET …/software/versions?variant=`). The version is checked against
that list when it loads, written to the variable together with `BUILD_NUMBER` back to `latest` (a
build pinned for the old version does not exist for the new one), and the server is reinstalled the
same way. Software and version can change in one go. A modpack install uses the same switch
([minecraft.md](minecraft.md#modpacks)).

The activity log gets `server:settings.software` with the variant and the version.

## Startup

`/startup` is upstream's route and sidebar row (`startup.read`) with a page of ours
(`ServerSettingsStartup.tsx`; upstream's `ServerStartup.tsx` stays unused), titled "Startup" with
Pelican's "Startup command" section: the command in a well with a copy button, the startup command
(the egg's list, "Custom" where the egg allows commands of its own, which opens the command for
editing) and the Docker image side by side, then **Server Variables**. Each field saves on its own
when it loses focus, switches and lists when they change ("Updated: <variable>"), like Pelican's
live startup variables; there is no Save button.

**Enable Preview** / **Disable Preview**, the link at the end of the Startup Command field's label
line (Pelican's `PreviewStartupAction`), switches the well to the command with its values filled in
and back; the copy button copies what the well shows. The panel fills it in
(`shared/src/startup_preview.rs`, `GET …/startup/preview`, `startup.read`) as the Pelican panel's
`StartupCommandService` does: `{{SERVER_MEMORY}}`, `{{SERVER_IP}}` and `{{SERVER_PORT}}` from the
server and its primary allocation (127.0.0.1 and 0 without one), then each `{{VARIABLE}}` of the
egg with the server's value or the default (a variable of that name wins over the three, as in
Wings' environment). Variables the egg hides from players, and secret ones, show as `[hidden]`, so
the preview never shows a value the variables list does not; unknown `{{…}}` stay as written. The
preview is fetched again when the command, a variable, the memory or the allocation changes.

## Behaviour

`pages/server/settings/BehaviourSection.tsx`: upstream's Auto-Start, Timezone, Auto-Kill and Trash
cards as one form titled Behaviour. Auto-Start and Timezone side by side (the timezone's helper text
is the server's time in it, ticking; empty means the system's), then the Auto-Kill switch with
"Seconds until auto-kill" under it, then "Use custom trash settings" with Retention and Clear at
Disk Usage under it once on (the panel's default under the switch while off; only when Admin ›
Settings lets servers change their trash). Each part shows with its own permission. One **Save**
under the fields sends only the parts that changed, each to its own route (`settings/auto-start`,
`timezone`, `auto-kill`, `trash`) with its own toast; it stays off while nothing changed or a changed
value is invalid. Upstream's four cards (`{Autokill,Autostart,Timezone,Trash}Container.tsx`) are
unused.

## Notifications

`shared/src/backup_notifications.rs`. Two switches per person and server: **Manual Backup
Notifications** (backups made on the Backups page) and **Scheduled Backup Notifications** (a
schedule's: its `server:backup.create` activity has a `schedule_uuid`). When a backup finishes, the
owner and the subusers with `backups.read` who switched that kind on get the `backup_completed`
mail (Admin › Email Templates), successful or failed, with a link to the Backups page, and the
same in the panel's [notifications](notifications.md) (as the Pelican panel's bell). System
backups send none. Switches save at once ("Notification Settings Updated"); both off deletes the
row. The activity log gets `server:settings.notifications`.

The switches are rows of `server_notification_settings` (migration `backup_notifications`).

## Danger Zone

The last section: its note under the heading, then two rows, Reinstall Server (the reinstall dialog
keeps upstream's "delete all files" switch, and admins' start afterwards) and Delete Server, each
with what it does and a gray button at its end. The red is in the confirmations; the section has no
red edge or tint of its own.

## Delete

`DELETE /api/client/servers/{server}`. The player types the server's name to confirm. The server is
deleted like an admin deletes it (not forced: when Wings refuses, nothing is deleted; but for a
stuck or failed install, [below](#stuck-installs)), then its backups, except its system backups
while deleted servers are held: the newest one stays downloadable from the owner's Backups page
([backups-page.md](backups-page.md)). A suspended or installing server cannot be deleted from
here; one whose install is stuck or failed can, from its banner or block screen. The account's
activity log gets `server:delete` (the server's own log goes with it). The route and neo's `delete_server` ([neo.md](neo.md#tools)) share
`shared::deleted_servers::delete_server`.

## Stuck installs

As on the Pelican panel's installing page: an install that has run for 30 minutes is probably
stuck (Wings refused the server, or the node lost it), and the console's installing banner says so
and offers two ways out (`frontend/src/elements/InstallStuck.tsx`):

- **Reinstall** (`settings.install`) confirms ("Start the install over?"), then
  `POST /settings/install/retry` asks Wings to drop whatever install it still runs (a refusal there
  is fine), sets the server back to installed and installs again as a reinstall does, keeping the
  files. When Wings refuses that too, the server stays installing (so it can still be deleted) and
  the error shows as a toast. The route answers 409 while the install is not stuck yet, and is
  reachable while the server installs. Activity: `server:settings.install-retry`.
- **Delete Server** (as in [Delete](#delete): `settings.delete` and deleting allowed) opens the
  usual dialog. A server whose install failed offers the same button on its block screen, next to
  upstream's acknowledge button. Such a server (stuck or failed) is deleted without waiting for its
  node to agree, as the Pelican panel forced it: Wings often does not know it at all, or its node
  is gone (`stuck_installs::delete_without_node`, also for neo's `delete_server`). Files Wings
  may still keep for it stay on the node.

When the install began is not stored (no schema change): `installingSince` on the server (only
while installing) is the newest of its creation and `servers.last_activity`, which every activity
a person causes stamps (a reinstall or a software switch logs one), as the Pelican panel used
`updated_at` (`shared/src/stuck_installs.rs`). An install that starts while the page is open counts
from when the page saw it start.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/settings/resize` | `settings.resize` | `{resize: {enabled, current, max, plans}}`; `max` is `null` per resource without a limit; `plans` are the public plans with `fits` and `current` |
| `POST /api/client/servers/{server}/settings/resize` | `settings.resize` | Body `{cpu, memory, disk}`; 403 when off, 409 when the allowance or the node has no room, 429 while another change of the owner runs |
| `GET /api/client/servers/{server}/settings/software` | `settings.software` | `{software: {variants, current, version}}`, `null` without choices or version |
| `GET /api/client/servers/{server}/settings/software/versions?variant` | `settings.software` | The Minecraft versions of that choice (the server's own without one), newest first |
| `POST /api/client/servers/{server}/settings/software` | `settings.software` | Body `{variant?, version?}` (one at least); 202, the reinstall runs; 409 while a Minecraft setup or a modpack holds the server's software lock ([minecraft.md](minecraft.md#modpacks)) |
| `DELETE /api/client/servers/{server}` | `settings.delete` | 403 when off |
| `POST /api/client/servers/{server}/settings/install/retry` | `settings.install` | Starts a stuck install over; 202, 409 while it is not stuck (30 minutes) |
| `GET /api/client/servers/{server}/settings/usage` | any | `{usage: {backups, databases, allocations}}`, each `null` without its list's read permission |
| `PUT /api/client/servers/{server}/settings/icon` | `settings.change-icon` | Body: the image (PNG, JPEG, WebP or GIF); `{icon}` (its link) |
| `DELETE /api/client/servers/{server}/settings/icon` | `settings.change-icon` | 400 when the server has no icon of its own |
| `GET /api/client/servers/{server}/startup/preview` | `startup.read` | `{command}`: the startup command with its values filled in, `[hidden]` for what the viewer may not read |
| `GET /api/client/servers/{server}/settings/notifications` | `backups.read` | `{notifications: {manual_backups, scheduled_backups}}` of the signed-in person |
| `PUT /api/client/servers/{server}/settings/notifications` | `backups.read` | Body `{manual_backups, scheduled_backups}` |

## Without noprofit.space

The allowance comes from the games settings and SFTP takes the panel password. Nothing else
changes.

## Check

- Admin › Settings › Hosting › Games: turn "Players Resize Servers" and "Players Delete Servers" on.
- As the owner of a server: Settings is one page with every section under each other, no sub-pages
  and no bar: each section's heading sits on the page with its box under it, like Transfer's
  sections, and Danger Zone is the last section. The header has the title only. Startup and VS Code
  (devboxes on) are rows of their own in the sidebar, each active on its own page.
- Open `/server/<id>/settings/resources` (and `sftp`, `version`, `behaviour`, `notifications`,
  `danger`): it lands on `/settings#resources` with that section at the top, also on a fresh load
  and on a phone (clear of the menu button). `/settings#danger` opens at the end of the page.
- Server Information: change the name, click elsewhere: "Updated Server Name". The copy icons turn
  green for 2 s.
- Server icon: on a server whose game has an icon (Admin › Settings › Hosting › Games), the game's
  icon shows at 64 px with "Using Egg Icon" beside it. Upload Icon, pick a picture, Upload Icon:
  "Server icon updated", and it shows on Settings, on the server's card in the server list and in
  the sidebar's server card. Delete Icon: "Server icon deleted", the game's icon is back. A subuser
  without "change-icon" sees the icon but no buttons.
- SFTP Information: copy host and username, log in with the password the row names.
- Resources: the current plan is picked; a plan over the allowance is dimmed. Advanced: raise memory
  past what is left: the field says so and Save resources stays off. Lower it and save: the
  console's memory limit follows without a restart. As a subuser without `settings.resize`, Limits
  shows in its place.
- Startup: change a variable and leave the field: "Updated: <name>". Enable Preview: the well
  shows the command with the memory, port and variable values in it, `[hidden]` for a secret
  variable; change a variable and the preview follows; Disable Preview shows the command again.
- Behaviour: change the timezone (the time under it follows) and the auto-start, Save: two toasts,
  and Save turns off again. Clear the seconds of auto-kill: the field says why and Save stays off.
  As a subuser with only `settings.timezone`, only the timezone shows.
- Version: pick another choice, "Change and reinstall", confirm: the console shows the reinstall;
  the files are still there afterwards and the new software runs after a start.
- Minecraft version on a Paper server: pick an older version, confirm; the egg variable shows it,
  `BUILD_NUMBER` is `latest`, and the server starts on that version.
- Notifications (after the migration): switch manual backups on, make a backup: the mail arrives.
- Danger Zone / Delete Server: the button in the dialog stays off until the name is typed; the server
  disappears from the list.
- Stuck install: on a server installing for over 30 minutes (stop its node's Wings during an
  install, then wait), the console's banner says the install is probably stuck, with Reinstall and
  Delete Server. Reinstall, confirm: "The install has started over." and the note goes away; with
  Wings still down, the error shows and the server stays installing. Delete Server deletes it,
  with Wings still down too. A
  subuser with neither `settings.install` nor `settings.delete` sees only the banner. A server whose
  install failed shows Delete Server on its block screen.
- As a subuser with only `startup.read`: the sidebar has Startup and no Settings. With only
  `backups.read`: Settings shows Server Information (read-only, no icon buttons), Limits and
  Notifications.
