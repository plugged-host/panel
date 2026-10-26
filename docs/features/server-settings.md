# Server settings for players

A server's settings look like the Pelican panel's settings page: the title "Settings" with a gray
**Delete Server** in the header, a sticky jump bar, then full-width sections in one column. Pelican
has one long page; here each jump-bar item is a page of its own, and the bar (`ServerSettingsNav`)
links them. Startup and VS Code left the sidebar for the bar, as on the Pelican panel; the
sidebar's Settings row stays active on them (`activeMatches`). Settings shows for anyone holding one
of its pages' permissions.

| Bar item | Page | Permission | Shown when |
|---|---|---|---|
| Server Information | `/settings` | any of Settings' | always: the icon (`settings.change-icon` to change it), name and description (saved on leaving the field, `settings.rename`), UUID, identifier, node, and the limits when the viewer cannot resize |
| SFTP Information | `/settings/sftp` | `files.sftp` | always |
| VS Code | `/devbox` | `devbox.connect` | devboxes are on ([devboxes.md](devboxes.md)) |
| Resources | `/settings/resources` | `settings.resize` | Admin › Settings › Hosting › Games › "Players Resize Servers" is on, or the viewer is an admin |
| Startup | `/startup` | `startup.read` | always (Pelican's "Startup command" section) |
| Version | `/settings/version` | `settings.software` | the server's egg is a game in the catalog with at least two software choices, or a Minecraft egg with a version variable |
| Behaviour | `/settings/behaviour` | `settings.auto-kill`, `.auto-start`, `.timezone` or `.trash` | Calagopus' own auto-start, timezone, auto-kill and trash settings as one section (Pelican has none of them; [below](#behaviour)) |
| Notifications | `/settings/notifications` | `backups.read` | always |
| Danger Zone | `/settings/danger` | `settings.install` or `settings.delete` | Reinstall, and Delete Server when deleting is allowed (below) |

Every page is one section, as every jump-bar item is one section on the Pelican panel: a
`TitleCard` (Filament's section: the heading without an icon, its description, the content, a
footer with the section's one action at the end; `description`, `footer`, `danger` props). Inside,
Filament's spacing: fields 1.5 rem apart (`flex flex-col gap-6`), side by side in
`grid gap-6` columns where Pelican's section has columns, helper text under the field. A section
whose fields save on their own (Server Information, Startup, Notifications) has no button; one with
a Save has it in the footer, filled once per page (Resources, Behaviour), gray where it reinstalls
or deletes (Version, Danger Zone) with the red in the confirmation. A section whose data still loads
shows its heading with a spinner. The label/value rows are `elements/SettingRows.tsx`; the styles
are the `.theme-jump*`, `.theme-section-*`, `.theme-setting-*` and `.theme-field-*` block of
`app.css`, the plan cards `.theme-choice*` (as on Create Server).

## Server Information

Name and description save when they lose focus ("Updated Server Name"), like Pelican's live fields.
The rows show the UUID and the identifier with copy buttons and the hosting node with the start of
its UUID (nodes have no number here). When the viewer cannot resize, the **Limits** grid shows CPU,
memory, disk, and backups, databases and allocations as "used / limit" (∞ for none) from
`GET …/settings/usage`; a count the viewer may not see (no `backups.read`, …) shows the limit only.

## Server icon

`shared/src/models/server/icon.rs`, shaped like upstream's account avatar. Server Information
shows the icon at 120 px in a column of its own (3/12 of the width, above the name on narrow
screens), with **Upload Icon** and, when the server has an icon of its own, **Delete Icon** under
it; both need `settings.change-icon` (Pelican's permission of that name, which the importer now
keeps). Without an icon of its own the server shows its game's: the icon of its egg's game in
Admin › Settings › Hosting › Games, what the Pelican panel calls the egg's icon, with the tooltip
"Using Egg Icon". With neither, no picture (as on the Pelican panel).

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
instead; it starts on when the server's size is no plan. Each field shows what is left on its label
line, red below zero, and **Save resources** sends the values. Without public plans the fields show
directly.

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

`/startup` is upstream's route with a page of ours (`ServerSettingsStartup.tsx`; upstream's
`ServerStartup.tsx` stays unused): the command in a well with a copy button, the startup command
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

`pages/server/settings/BehaviourContainer.tsx`: upstream's Auto-Start, Timezone, Auto-Kill and
Trash cards as one section titled Behaviour, laid out like Pelican's Resources section. Auto-Start
and Timezone side by side (the timezone's helper text is the server's time in it, ticking; empty
means the system's), then the Auto-Kill switch with "Seconds until auto-kill" under it, then "Use
custom trash settings" with Retention and Clear at Disk Usage under it once on (the panel's default
under the switch while off; only when Admin › Settings lets servers change their trash). Each part
shows with its own permission. One **Save** in the footer sends only the parts that changed, each to
its own route (`settings/auto-start`, `timezone`, `auto-kill`, `trash`) with its own toast; it stays
off while nothing changed or a changed value is invalid. Upstream's four cards
(`{Autokill,Autostart,Timezone,Trash}Container.tsx`) are unused.

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

The red-edged section: a note, then Reinstall Server (the reinstall dialog keeps upstream's "delete
all files" switch, and admins' start afterwards) and Delete Server. Both buttons are gray; the red
one is in the confirmation.

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
| `POST /api/client/servers/{server}/settings/software` | `settings.software` | Body `{variant?, version?}` (one at least); 202, the reinstall runs |
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
- As the owner of a server: Settings shows the jump bar with every item; it sticks while scrolling,
  marks the current page, and Danger Zone sits red at its end. The sidebar keeps Settings active on
  Startup and VS Code.
- Server Information: change the name, click elsewhere: "Updated Server Name". The copy icons turn
  green for 2 s.
- Server icon: on a server whose game has an icon (Admin › Settings › Hosting › Games), the game's
  icon shows at 120 px with the tooltip "Using Egg Icon". Upload Icon, pick a picture, Upload Icon:
  "Server icon updated", and it shows on Settings, on the server's card in the server list and in
  the sidebar's server card. Delete Icon: "Server icon deleted", the game's icon is back. A subuser
  without "change-icon" sees the icon but no buttons.
- SFTP Information: copy host and username, log in with the password the row names.
- Resources: the current plan is picked; a plan over the allowance is dimmed. Advanced: raise memory
  past what is left: its hint turns red and Save resources stays off. Lower it and save: the
  console's memory limit follows without a restart.
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
- As a subuser with only `startup.read`: Settings shows Server Information (read-only) and Startup.
