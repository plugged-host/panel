# Notifications

What the panel tells a person after the fact, kept until they read or delete it: a move that
finished, a failover, a modpack installed while they were away. It is the Pelican panel's bell
(Filament's database notifications, which Theme v2 opened from a row in the sidebar)
(`shared/src/notifications.rs`, `shared/src/models/user_notification.rs`,
`backend/src/routes/api/client/account/notifications/`, `frontend/src/elements/notifications/`).

## The inbox

**Notifications** is a row of the sidebar under the search, on the dashboard, on a server and in the
admin area, with the number of unread notifications at its end (99+ above 99). The count is asked
for every 30 seconds, like Filament's bell.

The row opens a panel at the end of the window: the newest notification first, 25 at a time and
**Show more** for older ones. An unread one has an accent bar at its start edge (screen readers hear
"Unread notification"). Each shows its title, how long ago it came, its text and lines of detail (a
new address, why a move failed, an error), and:

- **Open server** (**View backups** for a backup), when it is about a server that still exists:
  marks it read and opens the server (its Backups page).
- **Mark as read**, while it is unread.
- the **×** at its top, which deletes it.

The header has **Mark all as read** (while any is unread) and **Clear**, which deletes them all
without asking, as Filament's does. The list is asked for again every 30 seconds while the panel is
open. Reading or deleting notifications is not written to the activity log.

## What sends one

| Kind | Who gets it | When |
|---|---|---|
| `transferCompleted` | The owner and whoever asked for the move; for a move an admin started (Lighten load, a membership running out) the owner only | A [move](transfers.md) finished: the new location and address |
| `transferFailed` | The owner and whoever asked; nobody for a move an admin started | A move gave up: the server stayed, with the reason when the panel knows it |
| `admin.transferFailed` | Every admin, and every user whose role has `servers.transfer` | Any move gave up, with the reason or the error |
| `allowanceCut` | The account | An [allowance cut](allowance-cuts.md) took CPU, memory or disk off its servers: each server with what it gave |
| `membershipEnded` | The account | A membership ran out and servers on locations it opened are moving ([economy](economy.md)) |
| `failoverDone` | The owner | A [failover](failover.md) or a drill restored the server on another node: the new address and the time of the backup used |
| `failoverFailed` | The owner, at most once a day per server, and only when the server had already left its node (before that nothing changed for them) | A failover failed |
| `modpackInstalled`, `modpackFailed` | Who started it (on Minecraft › Modpacks or through neo) | A modpack install ended ([Minecraft](minecraft.md)) |
| `frameworkInstalled`, `frameworkFailed` | Who started it | A framework install ended ([FiveM](fivem.md)) |
| `worldImported`, `worldImportFailed` | Who started it | An uploaded world archive became the loaded world, or could not ([Minecraft](minecraft.md)) |
| `partnerApproved`, `partnerRejected`, `partnerRevoked` | The applicant | An admin decided on their [partner](partners.md) application or ended the partnership (with the credits a month, or the reason) |
| `partnerPaid` | The partner | A month's partner credits were paid |
| `partnerCheckFailed`, `partnerSuspended` | The partner | The monthly check failed: the month is held until a date; past it the partnership is suspended |
| `partnerPaused`, `partnerResumed`, `partnerCreditsChanged` | The partner | An admin paused (with the reason) or resumed the payments, or changed the credits a month; a passing check that lifts a suspension sends `partnerResumed` too |
| `admin.partnerApplication`, `admin.partnerSuspended` | Every admin, and every user whose role has `partners.manage` | A new partner application; a partnership suspended after its grace period |
| `serverInstalled`, `serverInstallFailed` | The owner | An install or a reinstall ended (upstream's `InstallCompleted`; also the reinstall of a modpack install or a software switch, as on the Pelican panel, which had a second text for a reinstall). A failover's or limbo's restore sends none |
| `subuserAdded` | The person added | They were made a subuser of a server |
| `subuserRemoved` | The person removed | They are not a subuser of the server any more (no **Open server**) |
| `backupCompleted`, `backupFailed` | Whoever asked for that backup's mail on the server's Settings › Notifications ([server settings](server-settings.md)) | A backup made by hand or by a schedule ended |

The last four rows are the Pelican panel's own (upstream Pelican's listeners), which it sent both
as mails and to the bell; here too the mails stay as they were. Not sent: cancelled moves, and what a
person watches happen on the page they started it from (Geyser, a world reset or download).

## Text in the reader's language

A notification is stored as a `kind` and the `data` its text is drawn with (a server's name, a
location, an address, amounts), not as text: the panel draws it in whatever language the person
reads it in, from `elements.notifications.kinds.<kind>`. Kinds under `admin.` are for admins and,
like the other admin pages, in English and German only. A kind the panel does not know (sent by a
newer backend) shows as "Notification" with its server's name. Error texts (a modpack, a framework,
a world, a move for admins) are shown as the backend wrote them, at most 300 characters.

### Sending one from code

```rust
crate::notifications::send(state, user_uuid, Some(server.uuid), crate::notifications::MY_KIND, json!({ "server": server.name })).await;
crate::notifications::send_admins(state, "servers.transfer", Some(server.uuid), "admin.myKind", json!({ ... })).await;
```

Sending never fails the caller; a notification that cannot be written is logged. A new kind needs
its constant in `notifications.rs`, its strings in `translations.ts`, `de.json` and the nine other
offered languages (an `admin.` kind: English and German), and its case in
`frontend/src/elements/notifications/NotificationRow.tsx`.

## Routes

No permission: a person only ever reaches their own notifications.

| Route | Does |
|---|---|
| `GET /api/client/account/notifications?page=&per_page=` | `{notifications, unread}`: a page of `{uuid, kind, data, server, read, created}`, newest first; `server` is `{uuid, uuid_short}` while the server exists, else `null` |
| `GET /api/client/account/notifications/unread` | `{unread}` |
| `POST /api/client/account/notifications/read` | Marks every unread one read, `{updated}` |
| `POST /api/client/account/notifications/{notification}/read` | Marks one read; 404 when it is not the person's |
| `DELETE /api/client/account/notifications/{notification}` | Deletes one; 404 when it is not the person's |
| `DELETE /api/client/account/notifications` | Deletes every one, `{deleted}` |

## Tables

| Table / columns | What it holds |
|---|---|
| `user_notifications` | `uuid`, `user_uuid` (deleted with the account), `server_uuid` (set to null when the server is deleted: the text stays, the link goes), `kind`, `data` (JSON), `read` (when it was read), `created` |

Migration: `notifications`.

Indexes: `(user_uuid, created)` for the list, `user_uuid` where unread for the count, `server_uuid`,
`created` for pruning.

## Pruning

The Pelican panel kept notifications for ever. Here the `delete_old_activity` task (every 30
minutes) deletes read notifications older than 90 days and any notification older than 365 days,
so an inbox nobody opens does not grow without end.

## Without noprofit.space

Nothing changes: every kind is the panel's own. With noprofit.space connected, a smaller
noprofit.space plan is one more cause of an allowance cut, and so of its notification.

## Check

- Move a server as a player: when it is done, the bell counts one more and the panel shows "Server
  moved" with the new location and address. Let a move fail (no node with room): "Move failed" for
  the player, "Move failed: <server>" for an admin with the reason.
- Lighten a node in Admin › Nodes: the owners hear only of the completed moves.
- Install a modpack and leave the page; a FiveM framework; upload a world: each ends with its
  notification, success or failure, for the person who started it.
- Lower an account's allowance below what its servers use: "Your servers have fewer resources now"
  lists each server with what it gave.
- Drill a server in Admin › Nodes › Failover: its owner gets "Server moved to another node".
- Create a server and reinstall it: "Server installation completed" twice, for the owner. Add a
  subuser: they get "Added to server" with **Open server**; remove them: "Removed from server",
  without it. Switch on backup mails in Settings › Notifications and make a backup: "Backup
  completed" with **View backups**.
- **Open server**, **Mark as read**, the **×**, **Mark all as read** and **Clear** each change the
  count at once; more than 25 notifications show **Show more**.
- In Arabic the panel opens on the left and the unread bar is on the right edge of each row.
- Delete a server: its notifications stay, without **Open server**. Delete an account: its
  notifications are gone (`user_notifications_user_uuid_users_uuid_fkey`, cascade).
