# Old Pelican paths

Bookmarks and links to the Pelican panel's pages keep working: each old path sends the browser to
the page that replaced it (`frontend/src/routers/routes/legacyRoutes.ts`, a `<Navigate replace>`
next to the routers' not-found route, so a real page of the same path always wins). Server paths
keep their id: Pelican's short server id is the first eight characters of the UUID, which the panel
accepts the same way.

| Pelican path | Goes to |
|---|---|
| `/profile` | `/account` |
| `/create-server` | `/create` |
| `/store` | `/account/store` |
| `/plus`, `/token-store` (the Pelican panel already sent this one to `/plus`) | `/account/membership` |
| `/tokens` | `/account/tokens` |
| `/welcome` | `/` (the welcome tour runs on the server list) |
| `/server/{id}/console`, `/installing`, `/suspended`, `/start` | `/server/{id}` (the console; installing, suspended and the waiting room show there) |
| `/server/{id}/allocations` | `/server/{id}/network` |
| `/server/{id}/activity-logs` | `/server/{id}/activity` |
| `/server/{id}/users` | `/server/{id}/subusers` |
| `/server/{id}/content`, `/setup`, `/game-settings`, `/worlds`, `/players` | `/server/{id}/minecraft/content`, `/minecraft/setup`, `/minecraft/settings`, `/minecraft/worlds`, `/minecraft/players` |
| `/server/{id}/server-config`, `/frameworks`, `/player-list`, `/artifacts`, `/enhanced` | `/server/{id}/fivem/config`, `/fivem/frameworks`, `/fivem/players`, `/fivem/artifacts`, `/fivem/enhanced` |

Paths that are the same on both panels (`/server/{id}/files`, `/backups`, `/schedules`,
`/databases`, `/startup`, `/settings`, `/activity`, `/queue`, `/limbo`, `/transfer`, `/subusers`)
need nothing. Admin pages (`/admin/...`) are not redirected: their layout differs too much to map.
A Minecraft or FiveM path on a server of the other game ends on the not-found page, as the page
itself would.

## Check

- Open `/token-store` and `/plus`: Membership opens, and Back does not return to the old path.
- Open `/server/<short id>/allocations`: the server's Network page opens; `/server/<short id>/console`
  opens the console.
- Open `/server/<short id>/game-settings` on a Minecraft server: Game Settings opens.
