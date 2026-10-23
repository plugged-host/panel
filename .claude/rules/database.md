---
paths:
  - "database/**"
---

# Database schema and migrations

- The schema is `database/src/schema.ts` (Drizzle). Add a table next to its relatives, plural snake
  case like upstream's (`status_incidents`); a property of an existing thing is a column on its
  table (`nodes.deployment_paused`), not a side table. No `ph_` / `plugged_host` names.
- Never hand-write schema SQL. Generate: `cd database && DATABASE_URL=postgresql://x@127.0.0.1:1/x
  pnpm kit:generate --name <feature>` (`--custom` for a data migration, then write its SQL). Name it
  after the feature, no prefix. Read the generated SQL before committing it.
- Never edit a migration that is on `main`; the panel matches applied ones by snapshot id and will
  not rerun it. Write a new migration instead.
- After merging upstream, check the snapshot chain (`prevIds`): if it forked, regenerate ours after
  upstream's and keep their old snapshot ids (docs/upstream.md#the-migration-chain).
- Custom SQL starts with a `-- plugged.host:` comment saying what it does and why.
