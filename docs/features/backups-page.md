# Backups page and deleted servers

Account › Backups lists the newest backup of each of the user's servers and of the servers they
deleted in the last days, with a download for each (`shared/src/deleted_servers.rs`). It replaces
Pelican's backup portal: the copies are the panel's own backups, taken by a system backup policy.

## Setting up the copies

The copies are upstream's **system backups**: no code of ours takes them. An admin sets them up:

1. Admin › Backup Configurations: a configuration whose storage is off the nodes, so a copy
   survives its node: S3 (`https://s3.example.com`, a bucket, a key) or restic with an SFTP
   repository on a storage box (`sftp:backup@192.0.2.10:/srv/restic`). Make it the default of
   every location (or node).
2. Admin › System Backup Policies: a policy on that configuration that runs every 30 minutes,
   with a retention that keeps a day of copies (for example the newest 48), on its locations or
   nodes.

Players see these copies on the server's System Backups page too (read-only).

## The page

Laid out like the Pelican panel's backups portal: the heading "Server backups", then **Your
servers**, one full-width row per server the user owns (not suspended), newest server first. A row
shows the server's name, `egg · node · #id` (the ID copies itself on click), and the state of its
newest finished, successful backup, system or not:

| State | Status line | Actions |
|---|---|---|
| A backup exists | green dot, "Backup ready · size · 12 minutes ago" | Download, Manage (`/server/{id}`), Backups (`/server/{id}/backups`) |
| No backup yet | grey dot, "Waiting for the first copy from the node" | Manage, Backups |

With no live server, the section is a card: "No active servers found on this account."

**Deleted servers** follows only while the user has one held: the servers they (or an admin, or
the suspension purge) deleted, most recently deleted first, with "Deleted 2 days ago · Kept until
31 Oct · size" and Download as the only action. Its meta line shows the node that took the copy;
`deleted_servers` keeps no egg, so a deleted server's line has no egg.

Download asks for a link like the server's own backup download: a presigned S3 URL, or a link to
the node that took the backup (that node must be up). The button then reads "Starting…", then
"Downloading, see your browser", and ignores clicks for 30 seconds so a second click does not start
a second download. The account's activity log gets `account:backup.download`. Subusers do not see
other people's servers here: the page is the owner's.

`/account/backups?server=<id>` highlights that server's row (the selected-row look) and scrolls to
it. The server's Backups page links there with its **Off-site Backups** header action (a secondary
button before Create, shown to the server's owner).

API-key permissions: `backups.read` (the list) and `backups.download`.

| Route | Does |
|---|---|
| `GET /api/client/account/backups` | `{backups: [{server_uuid, server_uuid_short, server_name, egg_name, node_name, deleted, kept_until, backup: {uuid, name, bytes, created}}]}`; `egg_name` is null for a deleted server |
| `GET /api/client/account/backups/{backup}/download` | `{url}`; 404 unless the backup is one the page offers to this user |

## Deleted servers

Every server deletion (by its owner, by an admin, by the suspension purge) runs a delete handler
that records the server in `deleted_servers` with its successful system backups, in the deleting
transaction. Backups outlive their server (`server_backups.server_uuid` becomes null), so nothing
else changes about them. A server with no system backup is not recorded.

The `purge_deleted_servers` task (every 10 minutes) then:

1. removes a newly recorded server's older system backups at once, keeping the newest;
2. removes the newest one when the hold ends (`hold_until`) and closes the record (`purged`).

Exceptions:

- An owner deleting their server keeps its system backups for the hold; their own backups go
  with it as before ([server-settings.md](server-settings.md#delete)).
- An admin who ticks "delete backups" removes everything at once; otherwise upstream keeps the
  non-system backups detached (Admin › Nodes › Backups), and the hold still ends for the system ones.
- A server that was suspended when it was deleted is recorded (so its copies are removed in
  time) but not offered for download.

## Settings

Admin › Settings › Hosting › Recovery: **Keep Deleted Servers' Backups (Days)**
(`recovery::deleted_hold_days`, default 7, 0 to 90). 0 records nothing, and an owner's delete
removes the system backups too.

## Without noprofit.space

Nothing differs.

## Check

- Set up a backup configuration and a system backup policy as above; wait for the first copy.
- Account › Backups: "Your servers" lists every server you own, newest first, each with its game,
  node and `#id` (a click copies it); Download reads "Starting…", then "Downloading, see your
  browser", and the download starts. Manage and Backups open the server.
- A new server shows "Waiting for the first copy from the node" until its first backup.
- A server's Backups page › Off-site Backups opens Account › Backups with that server's row
  outlined and scrolled into view.
- Delete a server from its Settings page: it moves to "Deleted servers" with "Deleted …" and "Kept
  until …" (7 days on); its download still works. Within 10 minutes its older system backups are
  gone (Admin › Nodes › Backups), the newest stays.
- Admin › Settings › Hosting › Recovery: set 0; delete another server: it does not appear on the
  page.
- An API key without `backups.read` gets 403 on the list.
