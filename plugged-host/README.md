# plugged.host's build and deploy

This folder holds what is only plugged.host's: how its panel is built, deployed and run. What the
fork changes, how to develop on it and how to run your own are in [`docs/`](../docs/README.md)
(one page per feature in [`docs/features/`](../docs/features/)); nothing outside this folder
depends on it.

## Where it runs

| | Prod: panel.plugged.host | Dev |
|---|---|---|
| Boxes | the two prod boxes, behind one proxy | the webserver box only (also one of the prod boxes) |
| Units | `calagopus` (web, both boxes), `calagopus-tasks` (background tasks and the Discord bots, on one box at a time: the HA lease holder) | `calagopus-dev` (web and its own tasks) |
| Binary | `/opt/calagopus/current/panel-rs` → `releases/<release>-<UTC time>` (the last 3 kept) | `/opt/calagopus-dev/current/panel-rs` (the last 2 kept) |
| Config | `/etc/calagopus/panel.env` (the same on both boxes), `tasks.env` (what differs for the tasks unit), `deploy.env` (`PEER_SSH`, the other box) | `/etc/calagopus-dev/panel.env` |
| Listens | `127.0.0.1:8730` (web), `127.0.0.1:8731` (tasks) | `127.0.0.1:8732` |
| Database | PostgreSQL `calagopus_v3`, the HA cluster, through a local haproxy | the box's PostgreSQL, database `calagopus` |
| Redis | the HA Redis, through Sentinel | the box's Redis, its own DB number (never DB 0), prefix `calagopus` |
| Files | `/var/lib/calagopus`, uploads in `/var/www/v3-storage` (synced between the boxes), logs in `/var/log/calagopus` | `/var/lib/calagopus`, logs in `/var/log/calagopus-dev` |
| Migrations | once per deploy, by `deploy.sh` (`DATABASE_MIGRATE=false`) | on start (`DATABASE_MIGRATE=true`) |

Secrets live in those env files only (`root:calagopus 640`); [`panel.env.example`](panel.env.example)
lists the keys with placeholders. Everything else (noprofit.space, Discord, neo, ads, …) is a
setting in Admin › Settings, stored in the database: no env edit, no deploy. The prod units, the
HA services and the proxy are set up by plugged.host's private ops docs (`plugged-host/docs-priv`).

## Scripts

Never build on the webserver box: it has neither the disk nor the memory. The scripts copy the
working tree to the build host (`BUILD_HOST`, default the ssh alias `plugged-build`; `BUILD_DIR`,
default `/root/panel`) and run there at low CPU and IO priority, since that box is also a prod box.
It needs Rust, Node 26, pnpm 11 and mold.

| Script | What it does |
|---|---|
| `check.sh [ts biome fmt rust test js]` | The checks (all by default): `pnpm typecheck`, biome, `cargo fmt --check`, `cargo check`, `cargo test -p shared -p backend`, `pnpm test:translations` |
| `format.sh` | rustfmt + biome's safe fixes, copies the rewritten files back |
| `translations.sh [--missing]` | Regenerates `en.json`, copies it back, checks `LANGUAGES` (default every offered language: German every string, the others all but admin-only ones) for 100% |
| `generate-migration.sh <name> [snapshot-id]` | `kit:generate` for `database/src/schema.ts`, copies the migration back (`CUSTOM=1` for a data migration; a snapshot id re-chains a migration after an upstream merge) |
| `build.sh` | Checks the translations, builds the frontend when its sources changed (`FRONTEND=1` forces it), then `cargo build --profile release-fast -p panel-rs` (no LTO), and fetches `target/plugged-host/panel-rs` |
| `deploy.sh [--prod] [binary]` | Installs a binary (default `target/plugged-host/panel-rs`) as `<base>/releases/<release>-<UTC time>`, points `current` at it, restarts the unit, checks `/api/settings` and rolls back if it does not answer. Without `--prod`: the dev panel. With `--prod`: see below |

## Build and deploy

```bash
cd /var/www/plugged.host/panel
plugged-host/format.sh && plugged-host/check.sh
git commit … && git push            # what runs is in git
plugged-host/build.sh               # -> target/plugged-host/panel-rs
plugged-host/deploy.sh              # dev
```

**Prod is for emergency fixes only**, and every prod release is a new version: the release is
`v{VERSION}` with no commit in it ([`docs/features/release.md`](../docs/features/release.md)).

1. Bump `VERSION` (`3.0.3` → `3.0.4`), commit, push, build, try it on dev.
2. `plugged-host/deploy.sh --prod`, from the webserver box. It installs the release on this box and
   on `PEER_SSH`, runs the migrations once, restarts `calagopus` here, then on the other box (each
   checked, both rolled back if either does not answer), then restarts `calagopus-tasks` wherever it
   runs. It refuses a release prod already ran (`SAME_RELEASE=1` overrides).
3. `./tag-release.sh 3.0.4` tags `v3.0.4` and pushes the tag. The Release workflow builds the
   binaries and images and leaves a draft GitHub release: write its notes (what changed, which Wings
   release goes with it) and publish it. Tag [the Wings fork](https://github.com/plugged-host/wings)
   too when it changed.
4. If users notice the change, announce it on Discord with `ph-announce` (plugged.host's ops tools).

## Roll back

`deploy.sh` rolls back by itself when the new release does not answer. To go back by hand, deploy a
kept release again:

```bash
ls -1dt /opt/calagopus/releases/*/
SAME_RELEASE=1 plugged-host/deploy.sh --prod /opt/calagopus/releases/<release>-<UTC time>/panel-rs
plugged-host/deploy.sh /opt/calagopus-dev/releases/<release>-<UTC time>/panel-rs   # dev
```

Migrations are not rolled back: the older binary runs on the newer schema.

## Maintenance page

nginx shows [`maintenance.html`](maintenance.html) while the panel answers 502, 503 or 504 (a
restart, a crash). It reloads itself every 15 seconds and links the status page. Each box's nginx
serves it from `/var/www/plugged.host/panel/plugged-host/`; the other prod box has only a copy of
this file at that path, so copy it there when it changes.

Planned maintenance is an incident on the panel's own status page (Admin › Status, or an override
setting every component to `maintenance`; [`docs/features/status.md`](../docs/features/status.md)).
status.plugged.host keeps showing it while the panel is down.

## A fresh box

[`calagopus.service`](calagopus.service) and [`nginx.conf`](nginx.conf) are the single-box setup.
The order matters:

1. PostgreSQL: a role and a database. Redis: its own DB number (never DB 0) and prefix.
2. A system user `calagopus`, `/var/lib/calagopus`, `/var/log/calagopus`, and
   `/etc/calagopus/panel.env` from `panel.env.example`. `APP_ENCRYPTION_KEY` is generated once and
   never changed: it encrypts stored secrets.
3. The unit and a release (`deploy.sh --prod` on a box with one panel), started on the loopback only.
4. Before the site is public: the emergency admin, with the env loaded
   (`panel-rs users create --admin true`), and the application URL in Admin › Settings. With no
   users, the first visitor would be offered to register as admin.
5. Only then the nginx site.

noprofit.space is optional: Admin › Settings › Accounts › noprofit.space (client
`plugged-host-calagopus`), wire format and accounts side in [`docs/noprofit.md`](../docs/noprofit.md).

## Upstream

`git fetch upstream && git merge upstream/main` on `main` (never a side branch), as
[`docs/upstream.md`](../docs/upstream.md) describes: the merge driver for translation files
(configured once per clone), the conflict hot spots, the migration chain.
