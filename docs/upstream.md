# Upstream

This fork follows [Calagopus](https://github.com/calagopus/panel) by merging its `main`. Merging is
part of every larger piece of work (plugged.host merges at the start of each phase), so that the fork
never drifts far.

## Merging

```bash
git remote add upstream https://github.com/calagopus/panel.git    # once
git config merge.translations.name 'translation files, key by key'  # once, see Translations
git config merge.translations.driver 'node frontend/merge-translations.mjs %O %A %B %P'

git fetch upstream
git merge upstream/main          # on main; no side branches
```

Then, before committing the merge:

1. Resolve conflicts (the hot spots below). Keep upstream's change and re-apply ours on top, marked.
2. `cd frontend && pnpm install && pnpm build:translations`: `en.json` is generated, never merged by
   hand.
3. Regenerate the migration chain if it forked ([below](#the-migration-chain)).
4. Run every check ([development.md](development.md#checks)), including
   `pnpm coverage:translations de`: fill in German for upstream's new strings.
5. Look at the security alerts afterwards (`gh api 'repos/<owner>/panel/dependabot/alerts?state=open'`)
   and bump what upstream has not.
6. Update the change list at the end of this page when a merge moved one of our changes.

### Conflict hot spots

| File | Why |
|---|---|
| `frontend/src/app.css` | Theme v2 is a large marked section; upstream restyles often |
| `frontend/src/elements/navigation/Sidebar.tsx`, `pages/dashboard/home/ServerItem.tsx` | Rewritten for Theme v2 |
| `frontend/src/elements/data-display/{Table,StatCard}.tsx`, `elements/feedback/*` | Restyled |
| `frontend/src/translations.ts` | Our strings sit among upstream's |
| `frontend/src/routers/routes/*.ts` | Removed and added routes |
| `shared/src/settings/mod.rs`, `backend/src/routes/api/admin/settings.rs` | Our settings sections |
| `shared/src/permissions.rs` | Removed permissions, new groups |
| `backend/src/routes/api/**/mod.rs` | Unmounted routes (commented `.nest` lines) |
| `database/src/schema.ts` + `database/migrations/` | Our tables and the snapshot chain |
| `frontend/pnpm-lock.yaml`, `Cargo.lock` | Take upstream's, then `pnpm install` / `cargo update -w` to re-add ours |

## The migration chain

Each migration's `snapshot.json` has an `id` and the `prevIds` it follows; drizzle-kit diffs
`schema.ts` against the newest snapshot. When upstream adds migrations after ours were written, both
follow the same parent and the chain forks: the next `kit:generate` refuses or produces a wrong diff.

The panel records applied migrations **by snapshot id**, which allows moving ours to the end:

1. Note the snapshot `id` of each of our migrations after the fork point (`git log upstream/main..`
   shows which are ours), then delete their directories.
2. Generate them again, in their original order, with their original names:
   `pnpm kit:generate --name <feature>` (`--custom` for a data migration, then copy its SQL back).
3. In each regenerated `snapshot.json`, set `"id"` back to the old id. A panel that already ran the
   migration then skips it; a new install runs it after upstream's.
4. Check that each regenerated `migration.sql` matches the old one (apart from upstream's changes it
   no longer repeats).

`plugged-host/generate-migration.sh <name> <old-id>` does steps 2 and 3 for one migration.

## Translations

`.gitattributes` sends `frontend/public/translations/*.json` and `shared/mails/variables/*.json` to
the `translations` merge driver (`frontend/merge-translations.mjs`, configured per clone as above).
It merges by key: upstream's new and changed strings come in, ours stay, a key changed on both sides
keeps ours and is printed. Without the driver configured, git falls back to a line merge, which
conflicts on almost every upstream translation update.

Upstream's languages are maintained on its Crowdin project. This fork edits the files directly
(German, and from the languages phase on the other offered languages), so Crowdin updates and ours
meet only through this driver.

## Removed features

Hidden and unmounted, with their tables and models kept, so merges stay small: upstream's migrations
and `query!` SQL still reference those tables, and empty tables send Wings empty lists.

| Feature | How |
|---|---|
| Devices (Docker device passthrough), mounts | Admin, node, server, egg and client routers unmounted; routes removed from `adminRoutes.ts` / `serverRoutes.ts`; their permissions removed (tabs on node, server and egg views disappear with them) |
| Tundra private network ("Connections"), server firewall | Client tunnel and firewall routes and the admin node tunnel tab unmounted, their pages and permissions removed |
| Managed database instances (database agents), backup groups | Admin database-agent routes, the instance mode of server databases and backup groups unmounted; their pages and permissions removed. Classic database hosts stay |
| Extension manager | `/admin/extensions` and its pages removed (the extension system itself stays in the code) |
| Update checker | `state.updates.init` not called; the admin Updates tab removed |
| Telemetry | Off by default (`app.telemetry_enabled`); the admin switch stays |
| OAuth providers, security keys, registration | Off while noprofit.space is connected ([noprofit.md](noprofit.md)); unchanged otherwise |

The permissions are listed in `shared/src/permissions.rs` (`REMOVED_USER_PERMISSIONS`,
`REMOVED_ADMIN_PERMISSIONS`, `REMOVED_SERVER_PERMISSIONS`): they are never offered or granted, not
even to admins. Migration `removed_permissions` strips them from roles, API keys and subusers.
`frontend/src/plugins/usePermissions.ts` mirrors the lists, so every button, tab and sidebar entry
gated by them disappears without editing each page.

## Offering changes upstream

Fixes and small improvements upstream would want too can be offered as pull requests to
`calagopus/panel`. Upstream's [`CONTRIBUTING.md`](../CONTRIBUTING.md) applies there: among other
things, it **does not accept code written with AI tools**, so a change made by or with an agent is
not offered upstream. Whether to offer a change is the maintainer's call.

## Changes to upstream files

`git grep -n "plugged.host"` lists the marked lines; `git diff --stat upstream/main...main` lists
every file. By area:

| Area | Files |
|---|---|
| Theme v2 | `frontend/src/{app.css,theme.ts,main.tsx,App.tsx}`, `frontend/src/lib/accent.ts` (new), `frontend/src/assets/fonts/` (Space Grotesk, JetBrains Mono; Helvetica removed), `frontend/src/elements/{AppIcon,Copyright,DismissibleAnnouncementAlert,ServerStatusIndicator}.tsx`, `elements/buttons/Button.tsx`, `elements/containers/{Account,Admin,AdminSub,Server}ContentContainer.tsx`, `elements/containers/ContentContainerHeader.tsx`, `elements/data-display/{Avatar,Card,StatCard,Table,TitleCard}.tsx`, `elements/feedback/{EmptyState,ErrorState}.tsx`, `elements/layout/Container.tsx`, `elements/modals/Modal.tsx`, `elements/navigation/{Sidebar,SubNavigation}.tsx`, `elements/quickActions/QuickActionsTrigger.tsx`, `pages/dashboard/home/{DashboardHomeAll,ServerItem}.tsx`, `pages/server/console/{ServerConsole.tsx,stats/ServerDetails.tsx,stats/ServerPowerControls.tsx,terminal/Console.tsx}`, `pages/oobe/OobeConfiguration.tsx`, `routers/{Admin,Dashboard,Server,Oobe}Router.tsx` |
| Icons (Tabler drawing the FontAwesome names) | `frontend/src/lib/icons/{FontAwesomeIcon.tsx,tablerIcons.ts}` (new), `frontend/vite-plugins/tabler-icons.ts` (new), `frontend/vite.config.ts` |
| Accent colour | `frontend/src/lib/accent.ts`, `App.tsx`, `pages/admin/settings/metadata/{MetadataContainer,metadataFormValues}.tsx`, `shared/src/settings/metadata.rs`, `backend/src/routes/api/settings.rs`, `frontend/src/lib/schemas/settings.ts` |
| Branding | `frontend/public/` (icons, `site.webmanifest`, `plugged.host.png`; `icon.svg` removed), `shared/src/settings/app.rs` (defaults), `elements/Copyright.tsx` |
| Release | `VERSION` (new), `shared/build.rs`, `shared/src/lib.rs` (`RELEASE`, `RELEASE_VERSION`), `backend/src/commands/version.rs`, `backend/src/routes/api/settings.rs` (`app.release`) |
| noprofit.space | `shared/src/noprofit.rs` (new), `shared/src/settings/noprofit.rs` (new), `shared/src/settings/mod.rs`, `shared/src/models/user/mod.rs`, `shared/Cargo.toml`, `backend/src/routes/api/auth/{mod.rs,noprofit.rs (new),login/mod.rs,password/forgot.rs,register.rs}`, `backend/src/routes/api/client/account/{mod.rs,noprofit.rs (new),email/mod.rs,password.rs,password_login.rs}`, `backend/src/routes/api/remote/sftp/auth.rs`, `backend/src/routes/api/admin/settings.rs`, `backend/src/routes/api/settings.rs`, `backend/src/tasks.rs` (`sync_noprofit_accounts`), `database/src/schema.ts` (`user_noprofit_accounts`), `frontend/src/api/me/account/getNoprofitAccount.ts` (new), `frontend/src/api/admin/settings/updateNoprofitSettings.ts` (new), `frontend/src/lib/schemas/{noprofit.ts (new),settings.ts,admin/settings.ts}`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,noprofit/ (new)}`, `frontend/src/elements/form-engine/types.ts`, `frontend/src/pages/auth/Login.tsx`, `frontend/src/pages/dashboard/account/{DashboardAccount.tsx,NoprofitContainer.tsx (new)}`, `frontend/src/routers/routes/{accountRoutes,adminRoutes}.ts` |
| Start gates | `shared/src/start_gates.rs` (new), `shared/src/lib.rs`, `shared/src/models/server/mod.rs` (`wings_permissions`), `shared/src/models/user/auth.rs` (`is_admin`), `backend/src/routes/api/client/servers/_server_/{power.rs,settings/install/mod.rs,schedules/import.rs,schedules/_schedule_/steps/mod.rs,schedules/_schedule_/steps/_step_/mod.rs}`, `frontend/src/api/server/sendPowerAction.ts`, `frontend/src/plugins/server/{usePowerAction.ts (new),useBulkPowerActions.ts}`, `frontend/src/elements/{ServerStatusIndicator,quickActions/ServerQuickActions}.tsx`, `frontend/src/pages/server/console/{stats/ServerPowerControls.tsx,terminal/features/EulaModalFeature.tsx,terminal/features/JavaVersionModalFeature.tsx}`, `frontend/src/pages/server/settings/modals/SettingsReinstallModal.tsx` |
| Removed features | `backend/src/lib.rs`, `backend/src/routes/api/admin/{mod.rs,system/mod.rs,nodes/_node_/mod.rs,servers/_server_/mod.rs,servers/_server_/databases/mod.rs,locations/_location_/mod.rs,system_backup_policies/_system_backup_policy_/mod.rs,nests/_nest_/eggs/_egg_/mod.rs}`, `backend/src/routes/api/client/servers/_server_/{mod.rs,backups/mod.rs,databases/mod.rs}`, `shared/src/permissions.rs`, `shared/src/extensions/manager.rs`, `shared/src/settings/app.rs`, `frontend/src/plugins/usePermissions.ts`, `frontend/src/routers/routes/{adminRoutes,serverRoutes}.ts`, `frontend/src/routers/AdminRouter.tsx`, `frontend/src/pages/admin/home/AdminHome.tsx`, migration `removed_permissions` |
| Strings | `frontend/src/translations.ts`, `frontend/public/translations/{en,de}.json`, `frontend/coverage-translations.mts` (new), `frontend/merge-translations.mjs` (new), `frontend/package.json` (`coverage:translations`), `.gitattributes` (new) |
| Build | `Cargo.toml` (profile `release-fast`), `Cargo.lock`, `frontend/pnpm-workspace.yaml` (security override), `frontend/pnpm-lock.yaml` |
| Documentation | `README.md` (one line), `docs/`, `AGENTS.md`, `CLAUDE.md`, `.claude/rules/` (all new) |
| plugged.host's own files | `plugged-host/` (new) |
