# Development

Everything here runs on one Linux or macOS machine. The Rust build is heavy: plan for 8 GB of memory
and 20 GB of disk for `target/`.

## What you need

| Tool | Version |
|---|---|
| Rust | stable, at least `rust-version` in `Cargo.toml` (`rustup update stable`) |
| Node.js | 26 (24 works) |
| pnpm | 11 (`corepack enable`) |
| PostgreSQL | 15 or newer |
| Redis or Valkey | any recent |
| mold | optional, a faster linker (`RUSTFLAGS='-C link-arg=-fuse-ld=mold'`) |

## First run

```bash
git clone https://github.com/plugged-host/panel.git && cd panel
git remote add upstream https://github.com/calagopus/panel.git   # for merges, see upstream.md

# A database and its owner
sudo -u postgres createuser -P panel          # pick a password
sudo -u postgres createdb -O panel panel

# The environment
cp .env.example .env
#   DATABASE_URL="postgresql://panel:<password>@localhost:5432/panel"
#   REDIS_URL="redis://localhost"   (use a free DB number, e.g. redis://localhost/3, if Redis is shared)
#   APP_ENCRYPTION_KEY=<openssl rand -hex 32>   generated once, never changed: it encrypts stored secrets
#   DATABASE_MIGRATE=true

# The frontend is embedded in the binary, so build it once before the backend
cd frontend && pnpm install && pnpm build && cd ..

# The backend: migrates the database, then serves on PORT (8000)
SQLX_OFFLINE=true cargo run
```

Open `http://localhost:8000`. The first visit runs the setup (OOBE), which creates the first admin.
To make one from the command line instead:

```bash
SQLX_OFFLINE=true cargo run -- users create --username admin --email admin@example.com --admin true
```

`SQLX_OFFLINE=true` makes upstream's `query!` macros use `.sqlx/` instead of a database at compile
time. New code does not use those macros (see [Database code](#database-code)).

## Working on the frontend

```bash
cd frontend && pnpm dev       # Vite on :5173, /api and websockets proxied to the backend on :8000
```

`BACKEND_PORT` changes the backend port the dev server proxies to. Backend changes need a restart of
`cargo run`; the frontend reloads by itself.

## Builds

| Profile | Use |
|---|---|
| `cargo build` (dev) | Fast to build, slow to run |
| `cargo build --profile release-fast -p panel-rs` | Optimised without LTO, incremental: minutes. What plugged.host deploys |
| `cargo build --release -p panel-rs` | Upstream's release profile (fat LTO, size-optimised): close to an hour |

The binary is `target/<profile>/panel-rs`. It contains the frontend from the last `pnpm build`, so
rebuild the frontend first when it changed.

## Checks

Run what upstream's CI runs before you push:

```bash
cargo fmt --all --check
SQLX_OFFLINE=true cargo clippy
SQLX_OFFLINE=true cargo test -p shared -p backend
cd frontend
pnpm typecheck
pnpm biome:validate                # pnpm biome:fix applies the safe fixes
pnpm build:translations && pnpm coverage:translations   # every offered language
pnpm test:translations             # the merge driver's and the language rules' tests
```

`git config core.hooksPath .hooks` runs biome on every commit. plugged.host runs the same checks on a
build machine with `plugged-host/check.sh` (and formats with `plugged-host/format.sh`).

## Migrations

1. Change `database/src/schema.ts`: a new table next to its relatives, or a column on an existing
   table.
2. Generate the migration (drizzle-kit only diffs the schema against the newest snapshot; the
   `DATABASE_URL` is required by the config but never connected to):

   ```bash
   cd database && pnpm install
   DATABASE_URL=postgresql://generate@127.0.0.1:1/generate pnpm kit:generate --name <feature>
   ```

   For a data change (an `UPDATE`, a backfill), add `--custom` and write the SQL into the empty
   `migration.sql` it creates.
3. Read the generated `migration.sql`, then commit `database/migrations/<timestamp>_<feature>/`
   with the code that uses it. The panel applies it on its next start (`DATABASE_MIGRATE=true`).

Rules:

- Name the migration after the feature, without a prefix (`status`, `server_suspensions`).
- Never edit a migration that is on `main`: the panel matches applied migrations by snapshot id and
  will not run a changed one again. Write a new one.
- After merging upstream, generate once more and fix the chain if upstream's migrations and ours
  forked ([upstream.md](upstream.md#the-migration-chain)).

## Database code

- Runtime queries only: `sqlx::query`, `sqlx::query_as`, `sqlx::query_scalar` with `.bind(...)`.
  The `query!` macros need a live database at compile time and a refreshed `.sqlx/`.
- SQL built with `format!` must be wrapped: `sqlx::query(sqlx::AssertSqlSafe(format!(...)))`. Only
  ever format in identifiers or fragments from constants; values are always bound.
- Writes on `state.database.write()`, reads on `state.database.read()`.
- Models implement upstream's traits (`ByUuid`, `CreatableModel`, `DeletableModel`,
  `IntoApiObject`, `IntoAdminApiObject`); copy a small model such as `shared/src/models/location.rs`.

## Strings

- English goes into `frontend/src/translations.ts` under the page's key (`pages.server.queue.*`),
  never into a component. Use it with `const { t } = useTranslations(); t('pages.server.queue.title', {})`.
- `pnpm build:translations` regenerates `public/translations/en.json`; commit it.
- **German is kept complete.** Add the German string at the same key in
  `public/translations/de.json` in the same commit, keeping the file's order (a new key after its
  English neighbour). Mails: `shared/mails/variables/de.json`. German uses the informal "du", like
  upstream's.
- **The other offered languages** (ar, ru, tr, id, pt, vi, fr, uk, uz) get every string that is not
  admin-only (a key with an `admin` segment), at the same key in `public/translations/<code>.json`
  and in the mails, before the next deploy; use the language's glossary.
- `pnpm coverage:translations` checks every offered language (`pnpm coverage:translations de --missing`
  one, with the missing keys) and fails below 100%; plugged.host's build refuses to ship while it
  fails. Details: [features/languages.md](features/languages.md).
- Upstream's languages the panel does not offer stay as Crowdin has them and are never built.
- Layout CSS is logical (`ms-*`, `padding-inline-start`, `text-align: start`), so Arabic mirrors it;
  technical values (paths, addresses, code) get `dir='ltr'`.

## Tests

Unit tests sit in a `#[cfg(test)] mod tests` at the bottom of the module they test, and run with
`cargo test -p shared -p backend`. Test pure logic there (gate order, ranking, pricing, signature
matching, permission filters); there is no database in the test run, so keep queries out of the
tested functions.

There is no browser test suite: describe what to click through in the commit or the feature's page
(each [features/](features/) page ends with its check list).
