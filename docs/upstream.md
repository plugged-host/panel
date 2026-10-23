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
| Node telemetry and placement fields | `shared/src/node_telemetry.rs` (new), `shared/src/lib.rs`, `shared/src/models/node/{mod.rs,placement.rs (new)}`, `shared/src/models/location.rs`, `backend/src/tasks.rs` (`sample_nodes`, `prune_node_telemetry`), `backend/src/routes/api/admin/nodes/_node_/{mod.rs,telemetry.rs (new)}`, `backend/src/commands/{import/validate.rs,nodes/create.rs}`, `database/src/schema.ts` (`node_telemetry`, `node_telemetry_samples`, node and location columns), migration `node_telemetry`, `frontend/src/api/admin/nodes/getNodeTelemetry.ts` (new), `frontend/src/lib/{queryKeys.ts,schemas/admin/nodes.ts,schemas/admin/locations.ts}`, `frontend/src/pages/admin/nodes/{nodeFormValues.tsx,overview/NodeOverview.tsx,statistics/AdminNodeStatistics.tsx,statistics/NodeTelemetryHistory.tsx (new)}`, `frontend/src/pages/admin/locations/locationFormValues.tsx`, `frontend/src/pages/oobe/{OobeLocation,OobeNode}.tsx` (the new fields' defaults), `frontend/src/elements/charts/ChartBlock.tsx` (icon optional), allocation defaults: `frontend/src/lib/domain/allocation.ts` (new), `frontend/src/pages/admin/nodes/{NodeCreateOrUpdate.tsx,allocations/modals/NodeAllocationsCreateModal.tsx}` |
| Suspension details | `shared/src/models/server/{mod.rs,suspension.rs (new)}`, `shared/src/noprofit.rs` (servers of a deleted account), `backend/src/routes/api/admin/servers/_server_/mod.rs` (`suspended_by`, activity fields), `backend/src/tasks.rs` (`purge_suspended_servers`), `database/src/schema.ts` (`servers.suspension_*`), migration `server_suspension`, `frontend/src/lib/schemas/{server/server.ts,admin/servers.ts}`, `frontend/src/api/admin/servers/updateServer.ts`, `frontend/src/pages/admin/servers/{AdminServerActions.tsx,ServerOverview.tsx,ServerSuspensionCard.tsx (new),management/modals/ServerSuspendModal.tsx (new)}`, `frontend/src/routers/guards/{ServerStateGuard.tsx,SuspensionNotice.tsx (new)}`, `frontend/src/elements/feedback/ScreenBlock.tsx` (`content` may be markup) |
| Delete a user with their servers | `backend/src/routes/api/admin/users/_user_/mod.rs` (`?delete_servers=true&force=`), `frontend/src/api/admin/users/deleteUser.ts`, `frontend/src/pages/admin/users/UserCreateOrUpdate.tsx` |
| Status page | `shared/src/status.rs` (new), `shared/src/settings/{mod.rs,status.rs (new)}`, `shared/src/lib.rs`, `shared/src/permissions.rs` (`status`), `backend/src/routes/{mod.rs,data.rs (new)}`, `backend/src/routes/api/admin/{mod.rs,settings.rs,status/ (new)}`, `backend/src/tasks.rs` (`collect_status`, `prune_status`), `database/src/schema.ts` (`status_*`), migration `status`, `frontend/src/api/admin/{settings/updateStatusSettings.ts,status/} (new)`, `frontend/src/lib/{queryKeys.ts,schemas/admin/settings.ts,schemas/admin/status.ts (new)}`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,status/ (new)}`, `frontend/src/pages/admin/status/ (new)`, `frontend/src/routers/routes/adminRoutes.ts` |
| Abuse protection | `shared/src/abuse.rs`, `shared/src/abuse/{signatures,inspect}.rs` (new), `shared/src/settings/{mod.rs,noprofit.rs (serialize_secret),abuse.rs (new)}`, `shared/src/lib.rs`, `shared/src/permissions.rs` (`abuse`), `shared/src/models/server/suspension.rs` (deletion alert), `backend/src/routes/api/admin/{mod.rs,settings.rs,abuse/ (new)}`, `backend/src/tasks.rs` (`watch_abuse`, `scan_abuse`), `database/src/schema.ts` (`abuse_findings`), migration `abuse`, `frontend/src/api/admin/{abuse/,settings/updateAbuseSettings.ts,settings/removeAbuseAlertWebhook.ts} (new)`, `frontend/src/lib/{queryKeys.ts,schemas/admin/settings.ts,schemas/admin/abuse.ts (new)}`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,abuse/ (new)}`, `frontend/src/pages/admin/abuse/ (new)`, `frontend/src/routers/routes/adminRoutes.ts` |
| Health checks | `backend/src/routes/api/admin/system/health/{mod.rs,tasks.rs (new),telemetry.rs (new)}`, `frontend/src/api/admin/system/health/{getTasksHealth,getTelemetryHealth}.ts (new)`, `frontend/src/lib/queryKeys.ts`, `frontend/src/pages/admin/home/health/{AdminOverviewHealth.tsx,TasksHealthCard.tsx (new),TelemetryHealthCard.tsx (new)}` |
| Plans, games, creating servers | `shared/src/{allowance.rs,server_creation.rs} (new)`, `shared/src/models/{mod.rs,plan.rs (new)}`, `shared/src/settings/{mod.rs,games.rs (new)}`, `shared/src/lib.rs`, `shared/src/permissions.rs` (`plans`, `servers.deploy`), `backend/src/routes/api/admin/{mod.rs,settings.rs,plans/ (new)}`, `backend/src/routes/api/client/servers/{mod.rs,create.rs (new)}`, `backend/src/routes/api/remote/servers/_server_/install.rs` (`eula.txt`), `backend/src/routes/api/settings.rs` (`games`), `database/src/schema.ts` (`plans`), migration `plans`, `frontend/src/api/admin/{plans/,settings/updateGamesSettings.ts} (new)`, `frontend/src/api/server/{getServerCreateOptions,createServer}.ts (new)`, `frontend/src/lib/{queryKeys.ts,tableColumns.ts,schemas/settings.ts,schemas/admin/settings.ts,schemas/admin/plans.ts (new),schemas/server/create.ts (new)}`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/elements/form-engine/types.ts`, `frontend/src/pages/admin/{plans/ (new),settings/AdminSettings.tsx,settings/useSettingsSection.ts,settings/games/ (new)}`, `frontend/src/pages/dashboard/{create/ (new),home/DashboardHomeAll.tsx,home/DashboardHomeGrouped.tsx}`, `frontend/src/routers/{DashboardRouter.tsx,routes/adminRoutes.ts}`, `frontend/src/app.css` (`.theme-choice`), `frontend/src/lib/icons/tablerIcons.ts` |
| Server settings for players | `shared/src/{server_resize.rs,server_software.rs} (new)`, `shared/src/{lib.rs,allowance.rs,permissions.rs (settings.resize, .software, .delete),settings/games.rs}`, `backend/src/routes/api/client/servers/_server_/{mod.rs,delete.rs (new),settings/mod.rs,settings/resize.rs (new),settings/software.rs (new)}`, `backend/src/routes/api/settings.rs` (`games.allow_resizing`, `noprofit.sftp_passwords`), `frontend/src/api/server/{deleteServer.ts,settings/getServerResize.ts,settings/resizeServer.ts,settings/getServerSoftware.ts,settings/switchServerSoftware.ts} (new)`, `frontend/src/lib/{queryKeys.ts,schemas/settings.ts,schemas/server/settings.ts,schemas/admin/settings.ts}`, `frontend/src/plugins/useSftpPassword.ts (new)`, `frontend/src/pages/server/settings/{ServerSettings.tsx,SftpContainer.tsx,ResizeContainer.tsx,SoftwareContainer.tsx,DeleteContainer.tsx (new)}`, `frontend/src/pages/server/files/modals/SftpDetailsModal.tsx` (noprofit.space password), `frontend/src/pages/admin/settings/games/gamesFormValues.tsx`, `frontend/src/stores/slices/admin/settings.ts` |
| Placement and the deployment pause | `shared/src/models/node/{mod.rs (ranking, blocker, `deployment_paused_reason`),placement.rs}`, `shared/src/node_telemetry.rs` (`PRESSURE_SQL`), `shared/src/settings/{mod.rs,placement.rs (new)}`, `backend/src/routes/api/admin/settings.rs`, `backend/src/tasks.rs` (`sample_nodes`), `database/src/schema.ts` (`nodes.deployment_paused_reason`), migration `plans`, `frontend/src/api/admin/settings/updatePlacementSettings.ts (new)`, `frontend/src/lib/schemas/admin/{nodes,settings}.ts`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,placement/ (new)}`, `frontend/src/pages/admin/nodes/overview/NodeOverview.tsx` |
| Moving servers | `shared/src/server_transfers.rs (new)`, `shared/src/{lib.rs,permissions.rs (settings.transfer),server_creation.rs (locations() split out, constants pub),settings/games.rs}`, `backend/src/routes/api/client/servers/_server_/{mod.rs (IGNORED_STATUS_PATHS),transfer/ (new)}`, `backend/src/routes/api/admin/servers/_server_/transfer/cancel.rs` (`mark_cancelling`), `backend/src/routes/api/admin/nodes/_node_/{mod.rs,lighten.rs (new)}`, `backend/src/routes/api/admin/system/{mod.rs,capacity.rs (new)}`, `backend/src/routes/api/settings.rs` (`games.transfer_enabled`), `backend/src/tasks.rs` (`settle_transfers`), `database/src/schema.ts` (`server_transfer_requests`), migration `server_transfer_requests`, `frontend/src/api/server/transfer/ (new)`, `frontend/src/api/admin/{nodes/lightenNode.ts,system/getCapacity.ts} (new)`, `frontend/src/lib/{queryKeys.ts,schemas/settings.ts,schemas/admin/settings.ts,schemas/server/transfer.ts (new),schemas/admin/capacity.ts (new)}`, `frontend/src/pages/server/transfer/ (new)`, `frontend/src/pages/admin/home/{AdminHome.tsx,capacity/ (new)}`, `frontend/src/pages/admin/nodes/servers/{AdminNodeServers.tsx,modals/LightenNodeModal.tsx (new)}`, `frontend/src/pages/admin/settings/games/gamesFormValues.tsx`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/elements/containers/ServerContentContainer.tsx` (players' Cancel), `frontend/src/routers/routes/serverRoutes.ts` |
| Allowance cuts | `shared/src/allowance_cuts.rs (new)`, `shared/src/{lib.rs,allowance.rs,noprofit.rs (Link::store),settings/games.rs (allowance())}`, `backend/src/routes/api/admin/settings.rs` (games allowance lowered), `backend/src/tasks.rs` (`settle_allowance_cuts`), `database/src/schema.ts` (`user_allowance_cuts`), migration `allowance_cuts` |
| Backups page and deleted servers | `shared/src/{deleted_servers.rs,settings/recovery.rs} (new)`, `shared/src/{lib.rs,permissions.rs (backups.read, .download),settings/mod.rs}`, `backend/src/lib.rs` (`deleted_servers::register()`), `backend/src/tasks.rs` (`purge_deleted_servers`), `backend/src/routes/api/client/account/{mod.rs,backups/ (new)}`, `backend/src/routes/api/client/servers/_server_/delete.rs` (system backups held), `backend/src/routes/api/admin/settings.rs`, `database/src/schema.ts` (`deleted_servers`), migration `deleted_servers`, `frontend/src/api/me/backups/ (new)`, `frontend/src/api/admin/settings/updateRecoverySettings.ts (new)`, `frontend/src/lib/{queryKeys.ts,schemas/admin/settings.ts,schemas/user/backups.ts (new)}`, `frontend/src/pages/dashboard/backups/ (new)`, `frontend/src/routers/routes/accountRoutes.ts`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,recovery/ (new)}`, `frontend/src/stores/slices/admin/settings.ts` |
| Welcome tour | `shared/src/{welcome_tour.rs,settings/tour.rs} (new)`, `shared/src/{lib.rs,settings/mod.rs}`, `backend/src/routes/api/client/account/{mod.rs,tour.rs (new)}`, `backend/src/routes/api/admin/settings.rs`, `backend/src/routes/api/settings.rs` (`tour.enabled`), `database/src/schema.ts` (`user_welcome_tours`), migration `welcome_tour`, `frontend/src/elements/WelcomeTour.tsx (new)`, `frontend/src/elements/navigation/Sidebar.tsx` (menu entry), `frontend/src/api/me/account/{getWelcomeTour,updateWelcomeTour}.ts (new)`, `frontend/src/api/admin/settings/updateTourSettings.ts (new)`, `frontend/src/plugins/useWelcomeTour.ts (new)`, `frontend/src/lib/{queryKeys.ts,schemas/settings.ts,schemas/admin/settings.ts,schemas/user/welcomeTour.ts (new)}`, `frontend/src/pages/dashboard/home/WelcomeTourPanel.tsx (new)`, `frontend/src/pages/dashboard/create/CreateServerButton.tsx` (`data-tour`), `frontend/src/pages/server/console/{ServerConsole.tsx (`data-tour`),WelcomeTourServer.tsx (new)}`, `frontend/src/routers/DashboardRouter.tsx`, `frontend/src/pages/admin/settings/{AdminSettings.tsx,useSettingsSection.ts,tour/ (new)}`, `frontend/src/stores/slices/admin/settings.ts`, `frontend/src/app.css` (`.theme-tour*`) |
| Wings fork client | `api-clients/wings-api/src/fork.rs` (new), `api-clients/wings-api/src/lib.rs` and `generator-src/index.ts` (`mod fork`) |
| Strings | `frontend/src/translations.ts`, `frontend/public/translations/{en,de}.json`, `frontend/coverage-translations.mts` (new), `frontend/merge-translations.mjs` (new), `frontend/package.json` (`coverage:translations`), `.gitattributes` (new) |
| Build | `Cargo.toml` (profile `release-fast`), `Cargo.lock`, `frontend/pnpm-workspace.yaml` (security override), `frontend/pnpm-lock.yaml` |
| Documentation | `README.md` (one line), `docs/`, `AGENTS.md`, `CLAUDE.md`, `.claude/rules/` (all new) |
| plugged.host's own files | `plugged-host/` (new) |
