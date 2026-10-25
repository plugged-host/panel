# Architecture

The panel is one binary, `panel-rs`: an Axum HTTP server with the React frontend embedded in it,
PostgreSQL for data and Redis (or Valkey) for cache, sessions and rate limits. Game servers run on
nodes under **Wings** (plugged.host runs [its fork](wings.md)), which the panel drives over HTTP.

## Workspace

| Path | What it is |
|---|---|
| `bins/base` | `panel-rs`: registers the embedded frontend, the extension list, starts `backend::handle_startup()` and serves |
| `bins/all-in-one`, `bins/heavy-supervisor` | Upstream's Docker variants (panel + Wings in one image; the image that rebuilds extensions). plugged.host does not use them |
| `backend/` | HTTP routes (`src/routes/`), background tasks (`src/tasks.rs`), CLI commands (`src/commands/`), startup (`src/lib.rs`) |
| `shared/` | Everything the routes, tasks and extensions share: `AppState`, models, settings, permissions, mail, captcha, storage, cache, JWT, noprofit.space |
| `database/` | The schema in TypeScript (`src/schema.ts`, Drizzle) and the SQL migrations generated from it (`migrations/`) |
| `database-migrator/` | Embeds and applies `database/migrations` (also as `cargo run -p database-migrator -- migrate`) |
| `embedded-assets/` | Embeds `frontend/dist` into the binary |
| `api-clients/wings-api` | The Wings API client, generated from Wings' OpenAPI spec |
| `api-clients/db-agent-api`, `http-client` | Upstream's database-agent client (unused here, see [upstream.md](upstream.md#removed-features)) and the shared HTTP client |
| `rule-validator/` | Laravel's validation rules in Rust, for egg variables |
| `schema-extension/` | The macros that let extensions add fields to API objects |
| `backend-extensions/`, `frontend/extensions/` | Upstream's extension system. plugged.host adds no extensions: features go into the source |
| `frontend/` | React 19 + Mantine 9 + Tailwind 4, built with Vite |
| `plugged-host/` | plugged.host's own build, deploy and service files. Nothing else in the tree depends on it |
| `docs/`, `AGENTS.md`, `CLAUDE.md`, `.claude/rules/` | This documentation |

## Startup

`backend::handle_startup()` (`backend/src/lib.rs`):

1. Parses the environment (`shared/src/env.rs`, see `.env.example`). A CLI subcommand (`panel-rs
   users create`, `panel-rs version`, …) runs here and exits.
2. Connects the cache (Redis + an in-memory layer) and the database (one pool, or with
   `DATABASE_URL_PRIMARY` a write pool there and a read pool on `DATABASE_URL`).
3. With `DATABASE_MIGRATE=true`, applies every embedded migration whose snapshot id is not in the
   migrations table yet, oldest first.
4. Loads the settings, builds `AppState` (`shared::State` = `Arc<AppState>`): settings, database,
   cache, storage, mail, captcha, JWT, NTP, the HTTP client, extensions, background tasks.
5. Builds the router (`routes::router`) and lets extensions add theirs.
6. Registers the background tasks (`tasks::define_background_tasks`). They run only on the instance
   with `APP_PRIMARY=true`, so several panel instances can share one database.

## A request

```
nginx / proxy ─▶ panel-rs (Axum)
                 ├─ /api/settings, /api/languages, /api/announcements …   public
                 ├─ /api/auth/*                                             sign-in, OAuth, noprofit.space
                 ├─ /api/client/*            ── client::auth (session or API key)
                 │   └─ /servers/{server}/*  ── _server_::auth (owner, subuser or admin)
                 ├─ /api/admin/*             ── client::auth + admin::auth (admin or role permission)
                 ├─ /api/remote/*            ── remote::auth (a node's token, used by Wings)
                 └─ anything else            ── the embedded frontend (index.html for app routes)
```

- Every route folder has a `mod.rs` with `pub fn router(state) -> OpenApiRouter<State>` that nests
  its children; each endpoint is a `mod get|post|patch|delete { pub async fn route(...) }` with a
  `#[utoipa::path]`, so it is in the OpenAPI spec (`/api` shows it). Read
  `backend/src/routes/api/admin/locations/` for a compact example.
- A resource folder `_thing_/` has an `auth` middleware that loads the thing by UUID into the request
  extensions, checks the read permission and returns 404 when it is missing.
- Handlers check permissions themselves (`permissions.has_admin_permission("locations.delete")?`,
  `permissions.has_server_permission("control.start")?`), log activity
  (`GetAdminActivityLogger`, `GetServerActivityLogger`, `GetUserActivityLogger`) and answer with
  `ApiResponse`.
- Validation uses `garde` on the payload structs; errors become 400 responses with the field paths.

## Where code lives

Features are spread over the usual places, like upstream's own (backups or schedules are good models
to copy), never gathered in a folder of their own.

| Kind | Place |
|---|---|
| Table | `database/src/schema.ts`, next to its relatives, plural snake case (`status_incidents`). A property of an existing thing is a column on its table (`nodes.deployment_paused`), not a side table |
| Migration | `database/migrations/<timestamp>_<feature>/`, generated (see [development.md](development.md#migrations)) |
| Model | `shared/src/models/<name>.rs` (or `<name>/mod.rs`), registered in `shared/src/models/mod.rs`, with its event handlers |
| Shared logic | A file at `shared/src/` root, like `captcha.rs` or `mail.rs` |
| A game's tools | `shared/src/<game>/` (`minecraft/`), routes under `client/servers/_server_/<game>/`, pages under `pages/server/<game>/`; `frontend/src/lib/domain/games.ts` shows them only on that game's servers (by egg feature) |
| Settings section | `shared/src/settings/<section>.rs` + its field and (de)serializer in `settings/mod.rs` |
| Admin settings API | `backend/src/routes/api/admin/settings.rs` (one payload struct per section, or the section file's own with `apply()`) |
| Public settings | `backend/src/routes/api/settings.rs` (what the frontend may read before sign-in) |
| Routes | The existing tree: `client/servers/_server_/<feature>/`, `client/account/<feature>/`, `admin/<feature>/`, `admin/nodes/_node_/<feature>/` … mounted by the parent's `mod.rs` |
| Background task | `backend/src/tasks.rs`, `define_background_tasks`, next to upstream's |
| Permissions | `shared/src/permissions.rs` |
| Unit tests | `#[cfg(test)] mod tests` inside the module they test |
| Frontend page | `frontend/src/pages/{server,dashboard,admin}/<feature>/` |
| API client | `frontend/src/api/{server,me,admin}/<feature>/` |
| Route + sidebar entry | `frontend/src/routers/routes/{serverRoutes,accountRoutes,adminRoutes}.ts` |
| Extra tab on a view | That view's own tab list (NodeView, ServerView, UserView) |
| Strings | `frontend/src/translations.ts` (English) under the native key tree, `frontend/public/translations/<lang>.json` for the rest |
| Mails | `shared/mails/*.html` + `shared/mails/variables/<lang>.json` |

## Database

- PostgreSQL only. The Drizzle schema in `database/src/schema.ts` is the source of truth; migrations
  are generated SQL plus a snapshot (`snapshot.json`). The panel records applied migrations by
  snapshot id.
- Queries are written with `sqlx`. New code uses the runtime API (`sqlx::query`, `query_as`,
  `query_scalar`), never the `query!` macros: those need `cargo sqlx prepare` against a live
  database and an updated `.sqlx/`. sqlx 0.9 wants `sqlx::AssertSqlSafe(...)` around SQL built with
  `format!`. Upstream's existing `query!` calls stay as they are (`.sqlx/` covers them).
- `state.database.read()` (the read pool when there is one) and `state.database.write()`.

## Settings

- Stored as rows `key = value` in the `settings` table, keys `section::field`. One struct per
  section in `shared/src/settings/` with a `SettingsSerializeExt` and a `SettingsDeserializeExt`;
  secrets go through `write_raw_encrypted_setting` / `database.decrypt_base64` (encrypted with
  `APP_ENCRYPTION_KEY`) and are returned censored to the admin API.
- Read: `state.settings.get().await?` (a read guard, drop it before awaiting other things) or
  `state.settings.get_as(|s| s.app.name.clone())`. Write: `state.settings.get_mut().await?`, change,
  `.save().await?`. Each instance caches the settings for up to a minute, so another instance sees
  a save within that time.
- Sections: upstream's `app`, `metadata`, `webauthn`, `server`, `user`, `activity`, `ratelimits`
  (plus the storage, mail and captcha choices), and plugged.host's `noprofit`, `status`, `abuse`,
  `economy`, `queue`, `limbo`, `ads`, `games` (with the Minecraft and FiveM tabs), `placement`, `tour`,
  `recovery`, `subdomains`, `devboxes`, `discord`, `rux`, `neo` and `neo_discord`.
- Each section is a tab (`frontend/src/pages/admin/settings/<section>/`) on one of the Admin ›
  Settings pages, saved by `PUT /api/admin/settings` with that section's payload. The pages are
  groups by topic, each its own entry under "Settings" in the admin sidebar (`settingsGroups` in
  `AdminSettings.tsx`): General (application, metadata, storage, status page, updates), Mail,
  Accounts (users, noprofit.space, Discord, rux, passkeys, welcome tour), Hosting (servers, games, Minecraft,
  placement, start queue, limbo, recovery), Security (captcha, rate limits, abuse, activity), Economy (store, tokens,
  memberships: one section in three tabs), Ads (placement, Adsterra, Monetag, waiting room, ad
  blockers, in-game: one section in six tabs) and AI (neo, neo on Discord). A new section joins the group its
  topic belongs to; a big section is split into tabs that each save their own fields. Every
  setting is editable there; the environment holds only what must exist before the database does.

## Background tasks

`define_background_tasks` registers two kinds:

- `add_cron_task(name, cron, f)`: runs on a cron schedule with seconds (`0 */5 * * * *` = every five
  minutes).
- `add_task(name, f)`: a loop the task paces itself (sleeps inside).

Both run only on the primary instance. A task returns `Result`; errors are logged and the task
continues on its next run. Long work belongs in its own task, not in a request.

A long-lived connection (the Discord bots' gateway) is a cron task that starts and stops its own
tokio tasks as the settings ask (`discord::gateway::supervise`), so it too runs on the primary
instance only, and keeps its state in the cache for the other instances to show.

## Events

Models have event emitters (`shared/src/models/mod.rs`): `register_create_handler`,
`register_after_create_handler`, `register_update_handler`, `register_delete_handler` … Create, update
and delete handlers get the model's transaction, so a feature can react to a server being created or
deleted without editing the code that does it. Register them where the feature's model is.

## Permissions

`shared/src/permissions.rs` defines three maps: user (API key scopes), admin (`nodes.read`,
`servers.create`, …) and server (`control.start`, `files.read`, …). Roles carry admin and server
permissions; subusers and API keys carry server permissions. Admins have every permission.

- New permissions go into an existing group when they fit (`control.*`, `settings.*`); a new area
  gets a group of its own named like upstream's (`status`, `abuse`).
- Permissions of removed features are listed in `REMOVED_*_PERMISSIONS`: they are never offered or
  granted, and the frontend (`frontend/src/plugins/usePermissions.ts`) hides whatever they gate.
- Wings receives the user's server permissions in its websocket token (`websocket.rs`); Wings checks
  `control.*`, `websocket.*`, `files.*` there.

## Frontend

| Folder | What it holds |
|---|---|
| `src/routers/` | `AdminRouter`, `DashboardRouter` (account), `ServerRouter`, `AuthenticationRouter`, `OobeRouter`; route lists in `routes/` with sidebar entries, permissions and a `filter` for conditional entries |
| `src/pages/` | The pages, by area |
| `src/api/` | One file per API call (axios), returning typed data |
| `src/stores/` | zustand stores (`global` = public settings and languages, `admin`, `server`, `user`) |
| `src/elements/` | The component kit: containers, tables, forms (`form-engine`), modals, navigation |
| `src/plugins/` | Hooks: `usePermissions`, `useSearchablePaginatedTable`, `useResourceForm`, `useModalForm` … |
| `src/lib/` | Schemas (zod), serialization, icons, the accent colour |
| `src/app.css`, `src/theme.ts` | Theme v2 (see [features/theme.md](features/theme.md)) |
| `src/translations.ts` | The English strings, the source of `public/translations/en.json` |

Data from the API is camelCase in the frontend (`serializeForApi` / the transforms in
`lib/serialization`). Pages are built from the kit and its hooks; plugged.host's look comes from
`app.css`, so pages need no styling of their own.

## Strings

English lives in `frontend/src/translations.ts`; `pnpm build:translations` writes
`public/translations/en.json`. Every other language is a JSON file next to it, loaded at runtime
(`TranslationProvider`). Keys follow the native tree (`pages.server.backups.toast.locked`); a
feature's strings sit under its page's key, without a namespace of their own. Mails take their
strings from `shared/mails/variables/<lang>.json`. German is kept complete: see
[development.md](development.md#strings).
