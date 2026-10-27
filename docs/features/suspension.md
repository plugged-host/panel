# Suspension details

Upstream suspends a server with one flag (`servers.suspended`): Wings stops it and its owner is
locked out. The fork records why, since when, by whom and until when, shows that to the owner, and
deletes servers whose deletion time has passed.

## Columns

Next to the native `suspended` on `servers` (`shared/src/models/server/suspension.rs`):

| Column | Holds |
|---|---|
| `suspension_kind` | `manual` (an admin), `abuse` (the abuse checks) or `noprofit` (the owner's noprofit.space account was deleted). Empty reads as `manual` |
| `suspension_reason` | Free text, up to 1000 characters, shown to the owner and subusers |
| `suspended_at` | When it was suspended |
| `suspended_by` | The admin who suspended it (null when the panel did) |
| `suspension_delete_at` | When the purge deletes it, unless it is unsuspended first; empty: never |

Unsuspending clears all of them. Servers suspended before the columns existed have none of them and
show the plain upstream text.

## Who suspends

- **Admins**: the server's Management tab (or the quick action) opens a dialog asking for a reason
  and "Delete the server": never, or after 1, 3, 7 or 30 days. It sends the native
  `PATCH /api/admin/servers/{server}` with `suspended`, `suspension_reason` and
  `suspension_delete_at`; the handler records the admin. A suspended server's reason and deletion
  time can be changed with the same request. Bulk suspend from the server list has no reason.
- **The abuse checks** (Phase 1, [abuse.md](abuse.md)): `Server::suspend` with kind `abuse` and a
  deletion time from the abuse settings.
- **noprofit.space**: when the account sync finds an account deleted there, it suspends the panel
  user (upstream's user suspension) and every server the user owns, kind `noprofit`, deleted after
  Admin › Settings › noprofit.space's *Delete gone accounts after* (7 days by default; 0: no
  deletion time). The sync deletes the user itself once that time has passed
  ([noprofit.md](../noprofit.md#what-changes-while-it-is-connected)); if the account comes back
  first, it lifts the `noprofit` suspensions (and the user's, when the sync made it). Without
  noprofit.space this never happens.

`Server::suspend` leaves a server that is already suspended as it is, so a manual suspension is not
overwritten by an automatic one.

## What the owner sees

The server's pages are blocked as upstream does it; the block says who suspended it and when, the
reason, when it will be deleted if it has a deletion time, and where to get help.

The deletion line is `suspension_delete_at` (the API's `suspension.delete_at`, the time the purge
uses): "This server will be deleted in 3 days (October 30, 2026 at 6:00 PM GMT+1), unless the
suspension is lifted before then." The time left is rounded down (minutes under an hour, hours
under two days, then days; "now" once it is due) and counts down while the page is open; the date
is the viewer's language and time zone (the browser's). Under it: "We don't suspend servers by
mistake, and your server won't be deleted before this date." A suspension without a deletion time
says nothing about deletion.

Where to get help: a **Get help** button to the first link that is set of Admin › Settings › neo's
support link and Admin › Settings › Discord's invite link (`suspension::help_url`, sent as the
suspension's `help_url`), as the Pelican panel's suspended page links its support page or Discord.
With neither set, it says to contact the panel's administrators.

Admins see a **Suspension** card on the server's Overview: by whom, the admin, since when, the
reason and the deletion date. The activity log's `server:update` carries `suspended`,
`suspension_reason` and `suspension_delete_at`.

## Purge

`purge_suspended_servers` runs every 5 minutes on the primary instance and deletes up to 20
suspended servers whose `suspension_delete_at` has passed, oldest first, through the native server
delete (not forced: a node that cannot delete it is tried again next run). Backups stay, except for
kind `noprofit`: its owner's account is gone, so its backups go with it.

This is the only clock a suspended server runs on, so it is never deleted before the time its owner
is shown: limbo's *Delete After (Days)* skips suspended servers ([limbo.md](limbo.md#deleted-after-a-while)),
and a server suspended without a deletion time stays until it is unsuspended. The sync of a deleted
noprofit.space account deletes the user with all its servers once its days have passed, which is
the deletion time it gave the servers it suspended; a server an admin had suspended before keeps
its own time and goes with the user then.

## Without noprofit.space

Everything but the `noprofit` kind works the same.

## Check

- Suspend a test server with a reason and "after 1 day": the Overview card shows both; as the owner,
  the server page shows the reason, "This server will be deleted in 23 hours (…)" with the same
  date and time as the card, and "We don't suspend servers by mistake…" under it. In German and in
  Arabic the time left and the date are in that language.
- Unsuspend: the card disappears, the owner gets the server back.
- Suspend with no reason and "never": the owner sees who and when, without a reason line and
  without any line about deletion.
- Suspend a parked server (limbo) with "never": limbo's *Delete After (Days)* does not delete it.
- With a Discord invite link (or neo's support link) set, the block shows **Get help**, opening that
  link in a new tab; with both empty, the line about contacting the administrators instead.
