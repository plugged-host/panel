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
  user (upstream's user suspension) and every server the user owns, kind `noprofit`, no deletion
  time. Without noprofit.space this never happens.

`Server::suspend` leaves a server that is already suspended as it is, so a manual suspension is not
overwritten by an automatic one.

## What the owner sees

The server's pages are blocked as upstream does it; the block says who suspended it and when, the
reason, the deletion date if there is one, and to contact the panel's administrators.

Admins see a **Suspension** card on the server's Overview: by whom, the admin, since when, the
reason and the deletion date. The activity log's `server:update` carries `suspended`,
`suspension_reason` and `suspension_delete_at`.

## Purge

`purge_suspended_servers` runs every 5 minutes on the primary instance and deletes up to 20
suspended servers whose `suspension_delete_at` has passed, oldest first, through the native server
delete (not forced: a node that cannot delete it is tried again next run). Backups stay.

## Without noprofit.space

Everything but the `noprofit` kind works the same.

## Check

- Suspend a test server with a reason and "after 1 day": the Overview card shows both; as the owner,
  the server page shows the reason and the date.
- Unsuspend: the card disappears, the owner gets the server back.
- Suspend with no reason and "never": the owner sees who and when, without a reason line.
