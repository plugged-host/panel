# Contributing

How changes to this fork are made. Upstream's own rules for pull requests to Calagopus are in
[`CONTRIBUTING.md`](../CONTRIBUTING.md); they apply when you offer a change upstream (see
[upstream.md](upstream.md#offering-changes-upstream)).

## Native, not plugin-like

Every feature is built as if Calagopus had always had it:

- It sits where upstream keeps the equivalent ([architecture.md](architecture.md#where-code-lives)):
  its table in the schema next to its relatives, its routes in the existing tree, its settings as a
  section beside upstream's, its pages next to their siblings, its strings in the native key tree.
- A feature is spread over those places. There is no folder, module, table prefix, route prefix,
  settings tab or frontend registry that gathers "plugged.host's code".
- Before writing a feature, read the closest upstream one (backups, schedules, locations) and copy
  its shape: file layout, naming, error handling, activity logging, the components it uses.
- A property of an existing thing is a column on its table (`nodes.deployment_paused`), not a side
  table. New tables are named like upstream's: plural snake case (`status_incidents`).

## Names

No `ph_`, `plugged_host`, `PluggedHost` or `plugged-host` in code names, tables, columns, routes,
permission keys, settings keys or translation keys. Product names a user sees (the brand, "Plus",
the assistant's name) are settings with neutral defaults, so a fork renames them without touching
code ([forking.md](forking.md)).

## The `plugged.host` marker

Every change to an upstream file carries a comment with `plugged.host` on the changed line or the
line above it, and every new file starts with a one-line header comment that has it:

```rust
// plugged.host: noprofit.space settings section (Admin › Settings › Accounts › noprofit.space).
```

```ts
const accent = useGlobalStore((state) => state.settings.app.themeColor); // plugged.host
```

```rust
// plugged.host: the extension manager is removed (docs/upstream.md)
// .nest("/extensions", extensions::router(state))
```

`git grep -n "plugged.host"` then lists every change, which is what keeps upstream merges
manageable. JSON files cannot carry comments; they are listed in [upstream.md](upstream.md).

Keep each edit to an upstream file small and in that file's style. When a change grows, move it into
a new file and call it from one marked line.

## What a change brings along

A change is complete when it has, in the same commit:

- **Strings**: English in `translations.ts` and German in `de.json` (and mail variables when it
  sends mail). `pnpm coverage:translations de` passes.
- **Permissions**: a new action has a permission in `shared/src/permissions.rs`, checked in the
  handler and in the frontend route or button.
- **Settings**: anything an admin may want to change is a setting with an Admin › Settings field,
  never an environment variable. The environment only holds what is needed before the database
  connects (database, Redis, encryption key, bind address).
- **Works without noprofit.space**: every feature has a standalone default
  ([noprofit.md](noprofit.md#optional-by-design)).
- **Tests** for pure logic, next to the code.
- **Docs**: the feature's page in `docs/features/`, plus [architecture.md](architecture.md) when it
  adds a settings section, task or permission group, and [upstream.md](upstream.md) when it touches
  an upstream file.
- **Migrations** generated, never hand-written schema changes ([development.md](development.md#migrations)).

## Style

- Rust: `cargo fmt`, clippy clean, `anyhow` for errors in tasks, `ApiResponse::error(...)` with a
  status for user errors. Error messages are lower case without a trailing period, like upstream's.
- TypeScript: biome (`pnpm biome:fix`), the component kit in `src/elements`, hooks from
  `src/plugins`, icons through the FontAwesome names (they are drawn as Tabler icons, see
  [features/theme.md](features/theme.md)).
- UI: one primary (filled) button per section, destructive actions secondary until their
  confirmation, no decorative icons, gradients, shadows or background shapes. The theme already gives
  pages their look.
- Comments explain why, not what. User-facing text is plain English: short sentences, no jargon.

## Commits

- Imperative subject under ~72 characters (`Add the start queue`), a body that says why and what a
  reviewer should check.
- One feature or fix per commit; a phase may be several commits.
- `main` is the only branch. Commit, then push.
- Never commit secrets, host names or addresses of real machines, or SSH ports. Documentation and
  examples use placeholders (`panel.example.com`, `<password>`, `192.0.2.10`).

## Security reports

Report vulnerabilities privately through GitHub's security advisories on this repository, not in an
issue. If the problem is in Calagopus itself and not in a change of this fork, report it upstream as
its [`SECURITY.md`](../SECURITY.md) describes.
