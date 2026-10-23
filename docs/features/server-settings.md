# Server settings for players

Four cards on a server's Settings page, next to upstream's Rename and Reinstall: SFTP access,
resources, software and delete. Each one has its own permission, so an owner (who has every
permission) sees them all and a subuser only what they were given.

| Card | Permission | Shown when |
|---|---|---|
| SFTP | `files.sftp` | always |
| Resources | `settings.resize` | Admin › Settings › Games › "Players Resize Servers" is on, or the viewer is an admin |
| Software | `settings.software` | the server's egg is a game in the catalog with at least two software choices |
| Delete Server | `settings.delete` | Admin › Settings › Games › "Players Delete Servers" is on, or the viewer is an admin |

## SFTP

Host and port, the username (`<user>.<server>`), and which password SFTP takes: the noprofit.space
password when noprofit.space checks SFTP passwords (its account API is set up), else the panel
password, else an SSH key from the account. A click copies host or username; "Launch SFTP" opens
the `sftp://` link in an SFTP client. The same password line is used by upstream's SFTP details on
the Files page (`frontend/src/plugins/useSftpPassword.ts`). The public settings carry
`noprofit.sftp_passwords` for it.

## Resources

`shared/src/server_resize.rs`. CPU (percent of one thread), memory and disk, each up to the server's
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

`shared/src/server_software.rs`. The game's software choices from Admin › Settings › Games. The current
one is read back from the game's variable (an unknown value counts as the first choice). Switching:

1. writes the choice to the variable,
2. takes the choice's startup command and image (or the egg's first),
3. reinstalls the server without wiping it, and without starting it afterwards (the start goes
   through the start gates).

The activity log gets `server:settings.software`.

## Delete

`DELETE /api/client/servers/{server}`. The player types the server's name to confirm. The server is
deleted like an admin deletes it (not forced: when Wings refuses, nothing is deleted), then its
backups, except its system backups while deleted servers are held: the newest one stays
downloadable from the owner's Backups page ([backups-page.md](backups-page.md)). A suspended or
installing server cannot be
deleted from here. The account's activity log gets `server:delete` (the server's own log goes with
it).

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/settings/resize` | `settings.resize` | `{resize: {enabled, current, max}}`; `max` is `null` per resource without a limit |
| `POST /api/client/servers/{server}/settings/resize` | `settings.resize` | Body `{cpu, memory, disk}`; 403 when off, 409 when the allowance or the node has no room, 429 while another change of the owner runs |
| `GET /api/client/servers/{server}/settings/software` | `settings.software` | `{software: {variants, current}}`, `null` without choices |
| `POST /api/client/servers/{server}/settings/software` | `settings.software` | Body `{variant}`; 202, the reinstall runs |
| `DELETE /api/client/servers/{server}` | `settings.delete` | 403 when off |

## Without noprofit.space

The allowance comes from the games settings and SFTP takes the panel password. Nothing else
changes.

## Check

- Admin › Settings › Games: turn "Players Resize Servers" and "Players Delete Servers" on.
- As the owner of a server: Settings shows SFTP, Resources, Software (for a game with two or more
  choices) and Delete Server.
- SFTP: copy host and username, log in with the password the card names.
- Resources: raise memory past "Up to": Save stays off. Lower it and save: the console's memory limit
  follows without a restart. With noprofit.space, an account without a plan can only lower values.
- Software: pick another choice, "Switch Software", confirm: the console shows the reinstall; the
  files are still there afterwards and the new software runs after a start.
- Delete: the button in the dialog stays off until the name is typed; the server disappears from the
  list.
- As a subuser without the new permissions: none of the three cards (Resources, Software, Delete) show.
