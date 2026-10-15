# plugged.host on Calagopus

This repository is plugged.host's panel: [Calagopus](https://github.com/calagopus/panel) (MIT) with
plugged.host's changes made directly in its source. This folder holds what is only plugged.host's:
the build and deploy scripts, the service and nginx configuration, and this guide.

## What plugged.host changes

| Area | Where |
|---|---|
| **Theme v2** (the Pelican panel's theme: royal-blue accent, obsidian surfaces, Space Grotesk + JetBrains Mono, flat sidebar), dark only | `frontend/src/theme.ts` (Mantine palette), `frontend/src/app.css` (the `--phv2-*` tokens and the "plugged.host Theme v2" section), `elements/navigation/Sidebar.tsx`, `App.tsx` |
| **Accent colour** (Theme v2's 2.2 setting): Admin › Settings › Metadata › Theme Color recolours the panel, a Tailwind preset (the swatches) or any colour, kept readable | `frontend/src/lib/accent.ts` (port of the Pelican theme's `Support\Accent`), `App.tsx`, the metadata settings form; public settings `app.theme_color` |
| **Branding**: name, logo, favicons, manifest, footer | `frontend/public/`, `elements/Copyright.tsx`, `elements/AppIcon.tsx`, defaults in `shared/src/settings/{app,metadata}.rs` |
| **Release** `v{VERSION}.{commit}` (e.g. `v3.0.0.7bc33d4`) | `VERSION`, `shared/build.rs` → `shared::PLUGGED_HOST_RELEASE`; public settings `app.release`; `panel-rs version`. Cargo's own version stays upstream's |
| **noprofit.space** sign-in and account sync (optional) | `shared/src/noprofit.rs`, `backend/src/routes/api/auth/noprofit.rs`, `backend/src/routes/api/client/account/noprofit.rs`, table `user_noprofit_accounts`, login page, account page |
| Build profile `plugged-host` | `Cargo.toml` |

Every change is marked `plugged.host` in a comment, so `git grep -n "plugged.host"` lists them.

## noprofit.space

Optional, like the noprofit plugin on the Pelican panel (2.2):

- **Not configured** (no `NOPROFIT_ACCOUNTS_URL` / `NOPROFIT_SSO_SECRET`): stock Calagopus sign-in
  and registration.
- **Configured**: noprofit.space owns sign-in.
  - The login page goes straight to noprofit.space (client `plugged-host-calagopus`), like the
    Pelican panel's `/login`. A sign-in that comes back with an error stops on it, with *Try again*.
    The first sign-in links the panel user with the account's email, or makes one: admin =
    noprofit.space admin, no password, email verified, username from the display name.
    `NOPROFIT_LINK_BY_EMAIL=false` refuses an existing email instead of linking it.
  - Registration is off. Password sign-in is only for admins without a noprofit.space account:
    the emergency admin, made with `panel-rs users create … --admin true` and signing in via
    *Administrator sign-in* (`/auth/login?local=1`).
  - With `NOPROFIT_API_SECRET` the panel pulls every linked account's snapshot each minute:
    admin flag, email, name, avatar, plan, credits and Discord. A user whose account
    noprofit.space no longer has is suspended.
  - SFTP: those users have no panel password, so with `NOPROFIT_API_SECRET` their SFTP password
    logins are checked by noprofit.space (the account's noprofit.space password; 10 wrong guesses
    per account and minute, shared with the other panels). SSH keys work as on stock Calagopus.
  - `NOPROFIT_ADMINS_ONLY=true` lets only noprofit.space admins in.

**Wire format** (the accounts app's `SignedToken` and `RequestSignature`; tested against its
vectors in `shared/src/noprofit.rs`):

- A token is `base64url(json).hex(hmac_sha256(secret, base64url(json)))`.
- Server-to-server calls carry `X-NP-Timestamp`, `X-NP-Nonce` and `X-NP-Signature`.

**On the accounts side**:

- Client `plugged-host-calagopus` in `config/noprofit.php`, with its claims arm in
  `AuthController::sso`.
- `POST /api/v1/internal/calagopus/accounts` (snapshots) and
  `POST /api/v1/internal/calagopus/verify-password` (SFTP passwords, `{account_id, password}`).
- Env `NOPROFIT_CALAGOPUS_SSO_SECRET`, `NOPROFIT_CALAGOPUS_SECRET` and `NOPROFIT_CALAGOPUS_URL`.
  Each one's value is the same as the panel's matching `NOPROFIT_SSO_SECRET` / `NOPROFIT_API_SECRET`.

## Build and deploy

Never build on the panel box: it has neither the disk nor the memory. `build.sh` copies the tree to
the build host (ssh alias `plugged-build`, a Ryzen 5 PRO 3600), builds the frontend when its sources
changed and the `plugged-host` profile (no LTO, incremental), and fetches the binary. The frontend is
embedded in it.

```bash
git commit … && git push            # the release names the commit
plugged-host/build.sh               # -> target/plugged-host/panel-rs
plugged-host/deploy.sh              # -> /opt/calagopus/releases/<release>, restart, health check, rollback
```

Migrations run on start. New tables and columns go into `database/src/schema.ts`; generate the
migration on the build host with `cd database && pnpm install && DATABASE_URL=postgresql://x pnpm kit:generate --name <name>`,
then commit `database/migrations/<timestamp>_<name>/`. New queries use runtime `sqlx::query*`, not
the `query!` macros: those need `cargo sqlx prepare` against a database.

## Install (non-Docker)

The panel box layout:

| What | Where |
|---|---|
| Source | `/var/www/calagopus` (this repository) |
| Binary | `/opt/calagopus/current/panel-rs` (→ `releases/<release>`) |
| Service | `calagopus.service` (this folder), user `calagopus` |
| Config | `/etc/calagopus/panel.env` (`panel.env.example`) |
| Data / logs | `/var/lib/calagopus`, `/var/log/calagopus` |
| Database | PostgreSQL, database and role `calagopus` |
| Cache | the shared Redis, DB 5 + prefix `calagopus` (never DB 0) |
| Web | nginx (`nginx.conf`) → `127.0.0.1:8730` |

Order on a fresh box:

1. Database, user, env, unit and binary.
2. Start it on the loopback.
3. Create the emergency admin: `panel-rs users create`, with the env loaded.
4. Set the application URL in Admin › Settings.
5. Only then enable the nginx site. With an empty users table, the first visitor would otherwise
   get to register as admin.

## Keeping up with upstream

```bash
git fetch upstream && git merge upstream/main      # on main; never a side branch
```

Conflicts are limited to the files above. Re-run the build, then check the login page, the sidebar
and the account page.
