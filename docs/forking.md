# Running your own fork

This panel can run under any name. Most of the brand is settings; a few files carry plugged.host's
defaults and are yours to replace. Everything optional (noprofit.space, ads, Discord) is off until
it is configured.

## The brand

| What | Where |
|---|---|
| Name | Admin › Settings › General › Application › Name (`app.name`). The default for new installs is in `shared/src/settings/app.rs` |
| Icon and banner | Admin › Settings › General › Application (`app.icon`, `app.banner`, and light variants). Defaults: `frontend/public/plugged.host.png` |
| Favicons, app manifest | `frontend/public/{favicon.ico,favicon-*.png,apple-touch-icon.png,android-chrome-*.png,site.webmanifest}` |
| Accent colour | Admin › Settings › General › Metadata › Theme Color: any colour; the panel derives its shades and keeps text readable (`frontend/src/lib/accent.ts`). Default in `shared/src/settings/metadata.rs` |
| Surfaces, fonts, radii | The tokens at the top of the Theme v2 section of `frontend/src/app.css` (`--theme-*`) and the palette in `frontend/src/theme.ts` ([features/theme.md](features/theme.md)) |
| Footer | `frontend/src/elements/Copyright.tsx`: your name and link, the release, "Powered by Calagopus" (keep the credit) |
| Product names a user sees | Settings with neutral defaults, in the section of the feature that shows them (for example the paid tiers' and the assistant's names, from the economy and AI phases on) |
| Mail templates | `shared/mails/*.html` use `settings.app.name`; the wording is in `shared/mails/variables/<lang>.json` |

Search for leftovers before you ship: `git grep -n "plugged.host" -- frontend/public shared/src/settings`.

## Version

`VERSION` at the root holds your version (`3.0.0`); the release is `v{VERSION}`.
`PANEL_RELEASE=...` at build time overrides it. Cargo's own version stays upstream's, so `panel-rs version` shows both
([features/release.md](features/release.md)).

## Languages

The panel offers the languages in `LANGUAGES` (`frontend/vite-plugins/translations.ts`): English,
German and nine others. Every other language is not built and the API refuses it. To offer a
different set, change that list (each entry with its own name, `rtl` for a right-to-left script,
`admin` when admin strings must be translated too) and give the languages you add a translation;
`pnpm coverage:translations` then checks exactly your list. Upstream's languages you turn on come
from Calagopus's Crowdin and may be incomplete. Taking a language off the list needs a migration
that moves its accounts to the default language. Details: [features/languages.md](features/languages.md).

## Optional services

| Service | Off by default | How to turn it on |
|---|---|---|
| noprofit.space | Yes: the panel signs people in on its own | [noprofit.md](noprofit.md) |
| Telemetry to calagopus.com | Yes | Admin › Settings › General › Application |
| Ads, Discord bots, the AI assistant (later phases) | Yes | Their settings sections |
| Asking players for reviews on a review site | Yes | Admin › Settings › Accounts › Reviews, your own review page ([features/reviews.md](features/reviews.md)) |

Nothing in the panel needs plugged.host's machines or accounts.

## Building and deploying

Any machine with the tools from [development.md](development.md#what-you-need) builds it:

```bash
cd frontend && pnpm install && pnpm build && cd ..
SQLX_OFFLINE=true cargo build --profile release-fast -p panel-rs     # target/release-fast/panel-rs
```

`plugged-host/` shows how plugged.host runs it, as an example to adapt, not to use as is:

| File | What it is |
|---|---|
| `build.sh` | Copies the tree to a build machine over ssh (`BUILD_HOST`, `BUILD_DIR`), checks the translations, builds the frontend when it changed and the binary, fetches it |
| `deploy.sh` | Installs a binary as `/opt/calagopus/releases/<release>`, switches `current`, restarts the unit, checks it answers and rolls back if not; optionally on a second box too (`PEER_SSH` in `/etc/calagopus/deploy.env`) |
| `calagopus.service` | The systemd unit |
| `panel.env.example` | The environment file (database, Redis with its own DB number and prefix, encryption key, bind address) |
| `nginx.conf` | The site in front of it |
| `check.sh`, `format.sh`, `translations.sh`, `generate-migration.sh` | The checks, formatters, coverage and drizzle-kit, run on the build machine |

Order on a fresh server: database and role, environment, binary and unit, start it on the loopback,
create the first admin (`panel-rs users create --admin true` with the environment loaded), set the
application URL in Admin › Settings › General, and only then open the site to the world. With an
empty users table the first visitor would otherwise get to set it up.

Upstream's Docker files (`docker/`, `compose*.yml`) build upstream's images; a fork that wants
containers builds its own image from its tree.

## Keeping your fork current

Merge this repository (or Calagopus directly) the way [upstream.md](upstream.md) describes. Keep
your brand changes in the files above, so merges touch them rarely.
