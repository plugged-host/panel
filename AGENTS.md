# AGENTS.md

plugged.host's game-server panel: a fork of [Calagopus](https://github.com/calagopus/panel)
(Rust/Axum backend, React/Mantine frontend, PostgreSQL, Redis) with plugged.host's features written
directly into the source. Read [`docs/README.md`](docs/README.md) first; the details are in
[`docs/`](docs/).

## Hard rules

1. **Native placement.** Put a feature where upstream keeps its equivalent and copy the closest
   upstream feature's shape (backups, schedules, locations). No plugin-like folders, registries,
   route prefixes, table prefixes or settings super-tabs. Map: [`docs/architecture.md`](docs/architecture.md#where-code-lives).
2. **Neutral names.** No `ph_`, `plugged_host`, `PluggedHost` or `plugged-host` in code, tables,
   columns, routes, permission keys, settings keys or translation keys. Names a user sees are
   settings with neutral defaults.
3. **Mark upstream edits.** A changed upstream line carries a `plugged.host` comment; a new file
   starts with a one-line `plugged.host:` header. Keep upstream edits small; move bulk into new files.
4. **Strings in English and German, together.** English in `frontend/src/translations.ts`, German at
   the same key in `frontend/public/translations/de.json` (keep its order), mails in
   `shared/mails/variables/`. The nine other offered languages need every non-admin string before
   the next deploy ([languages](docs/features/languages.md)). `pnpm build:translations && pnpm
   coverage:translations` must pass.
5. **Database.** Change `database/src/schema.ts` and generate the migration (`kit:generate --name
   <feature>`); never hand-write schema SQL or edit a migration that is on `main`. Runtime `sqlx`
   only (`sqlx::query*`, never `query!`), `sqlx::AssertSqlSafe` around formatted SQL, values always bound.
6. **Settings, not environment.** Anything an admin may change is a field in a settings section with
   an Admin › Settings tab; secrets are encrypted settings. Env is for what is needed before the
   database connects.
7. **noprofit.space is optional.** Every feature works without it (a standalone default) and asks
   `settings.noprofit.connected()` / `.config()` for the noprofit version.
8. **Permissions** for every new action, in `shared/src/permissions.rs`, checked in the handler and
   in the frontend. Features removed from upstream stay removed ([`docs/upstream.md`](docs/upstream.md#removed-features)).
9. **A change brings along** its strings, permissions, settings fields, unit tests for pure logic
   (`#[cfg(test)]` in the same module) and its docs page (`docs/features/<feature>.md`, plus
   `docs/upstream.md` for upstream files touched).
10. **Public repository.** No secrets, real host names, IP addresses or SSH ports in code, docs,
    commits or examples. Placeholders only (`panel.example.com`, `192.0.2.10`, `<secret>`).
11. **Upstream does not accept AI-written code.** Never open pull requests to `calagopus/panel`.
12. **One branch.** Work on `main`; no feature branches.

## Checks before committing

```bash
cargo fmt --all --check
SQLX_OFFLINE=true cargo clippy
SQLX_OFFLINE=true cargo test -p shared -p backend
cd frontend && pnpm typecheck && pnpm biome:validate
pnpm build:translations && pnpm coverage:translations && pnpm test:translations
```

The Rust build needs about 8 GB of memory. On a small machine, run the checks on a bigger one
(plugged.host uses `plugged-host/check.sh` and `plugged-host/format.sh`, which do this over ssh).

## Where to look

| Task | Read |
|---|---|
| Find where something goes | `docs/architecture.md` |
| Set up, run, test, migrate, add strings | `docs/development.md` |
| Rules for a change, commit messages | `docs/contributing.md` |
| Merge Calagopus, the change list | `docs/upstream.md` |
| A feature's behaviour | `docs/features/<feature>.md` |
| noprofit.space | `docs/noprofit.md` |
| Something Wings must do | `docs/wings.md` (the fork is `plugged-host/wings`) |
