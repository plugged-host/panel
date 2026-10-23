# plugged.host's build and deploy

This folder holds what is only plugged.host's: how its panel is built, deployed and run. What the
fork changes, how to develop on it and how to run your own are in [`docs/`](../docs/README.md);
nothing outside this folder depends on it.

## Scripts

Never build on the panel box: it has neither the disk nor the memory. The scripts copy the working
tree to the build host (`BUILD_HOST`, default the ssh alias `plugged-build`; `BUILD_DIR`, default
`/root/panel`) and run there at low CPU and IO priority, since that machine also serves production.

| Script | What it does |
|---|---|
| `check.sh [ts biome fmt rust test]` | The checks (all by default): `pnpm typecheck`, biome, `cargo fmt --check`, `cargo check`, `cargo test -p shared -p backend` |
| `format.sh` | rustfmt + biome's safe fixes, copies the rewritten files back |
| `translations.sh [--missing]` | Regenerates `en.json`, copies it back, checks `LANGUAGES` (default `de`) for 100% |
| `generate-migration.sh <name> [snapshot-id]` | `kit:generate` for `database/src/schema.ts`, copies the migration back (`CUSTOM=1` for a data migration; a snapshot id re-chains a migration after an upstream merge) |
| `build.sh` | Checks the translations, builds the frontend when its sources changed, then `cargo build --profile release-fast -p panel-rs`, and fetches `target/plugged-host/panel-rs` |
| `deploy.sh [binary]` | Installs it as `/opt/calagopus/releases/<release>`, points `current` at it, restarts `calagopus`, checks `/api/settings` and rolls back if it does not answer; keeps the last 3 releases |

```bash
plugged-host/format.sh && plugged-host/check.sh
git commit … && git push            # the release names the commit
plugged-host/build.sh && plugged-host/deploy.sh
```

Migrations run on start (`DATABASE_MIGRATE=true`).

## The panel box

| What | Where |
|---|---|
| Source | `/var/www/calagopus` (this repository) |
| Binary | `/opt/calagopus/current/panel-rs` (→ `releases/<release>`) |
| Service | `calagopus.service` (this folder), user `calagopus` |
| Config | `/etc/calagopus/panel.env` (`panel.env.example`) |
| Data / logs | `/var/lib/calagopus`, `/var/log/calagopus` |
| Database | PostgreSQL, database and role `calagopus` |
| Cache | the shared Redis, its own DB number + prefix `calagopus` (never DB 0) |
| Web | nginx (`nginx.conf`) → the loopback port in `panel.env` |

noprofit.space is configured in Admin › Settings › noprofit.space (client `plugged-host-calagopus`);
the `NOPROFIT_*` variables in older `panel.env` files only fill it in until the settings are saved
once. On the accounts side: the client in `config/noprofit.php` with its claims arm in
`AuthController::sso`, `POST /api/v1/internal/calagopus/{accounts,verify-password}`, and env
`NOPROFIT_CALAGOPUS_SSO_SECRET`, `NOPROFIT_CALAGOPUS_SECRET`, `NOPROFIT_CALAGOPUS_URL` equal to the
panel's secrets ([`docs/noprofit.md`](../docs/noprofit.md) has the wire format).

Order on a fresh box: database and role, env, unit and binary; start it on the loopback; create the
emergency admin (`panel-rs users create --admin true` with the env loaded); set the application URL in
Admin › Settings; only then enable the nginx site.

## Upstream

`git fetch upstream && git merge upstream/main` on `main`, as [`docs/upstream.md`](../docs/upstream.md)
describes (including the merge driver for translation files, configured once per clone).
