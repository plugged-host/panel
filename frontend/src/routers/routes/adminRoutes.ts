import {
  faBriefcase,
  faBuilding,
  faBullhorn,
  faClockRotateLeft,
  faCogs,
  faCrow,
  faDatabase,
  faDownload,
  faEarthAmerica,
  faEgg,
  faFileZipper,
  faFingerprint,
  faFolderOpen,
  faHeartPulse,
  faLayerGroup,
  faScroll,
  faServer,
  faShieldHalved,
  faUsers,
  faWrench,
} from '@fortawesome/free-solid-svg-icons';
import { faComputer } from '@fortawesome/free-solid-svg-icons/faComputer';
import type { AdminRouteDefinition } from 'shared';
import type { LazyString } from '@/lib/lazy.ts';
import AdminAbuse from '@/pages/admin/abuse/AdminAbuse.tsx';
import AdminActivity from '@/pages/admin/activity/AdminActivity.tsx';
import AdminAnnouncements from '@/pages/admin/announcements/AdminAnnouncements.tsx';
import AdminAssets from '@/pages/admin/assets/AdminAssets.tsx';
import AdminBackupConfigurations from '@/pages/admin/backup-configurations/AdminBackupConfigurations.tsx';
import AdminDatabaseHosts from '@/pages/admin/database-hosts/AdminDatabaseHosts.tsx';
import AdminEggConfigurations from '@/pages/admin/egg-configurations/AdminEggConfigurations.tsx';
import AdminEggLibrary from '@/pages/admin/egg-library/AdminEggLibrary.tsx';
import AdminEggRepositories from '@/pages/admin/egg-repositories/AdminEggRepositories.tsx';
import AdminHome from '@/pages/admin/home/AdminHome.tsx';
import AdminLocations from '@/pages/admin/locations/AdminLocations.tsx';
import AdminNests from '@/pages/admin/nests/AdminNests.tsx';
import AdminNodes from '@/pages/admin/nodes/AdminNodes.tsx';
import AdminOAuthProviders from '@/pages/admin/oauth-providers/AdminOAuthProviders.tsx';
import AdminPlans from '@/pages/admin/plans/AdminPlans.tsx'; // plugged.host
import AdminRoles from '@/pages/admin/roles/AdminRoles.tsx';
import AdminServers from '@/pages/admin/servers/AdminServers.tsx';
import AdminSettings from '@/pages/admin/settings/AdminSettings.tsx';
import AdminStatus from '@/pages/admin/status/AdminStatus.tsx';
import AdminSystemBackupPolicies from '@/pages/admin/system-backup-policies/AdminSystemBackupPolicies.tsx';
import AdminUsers from '@/pages/admin/users/AdminUsers.tsx';
import { getTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

export const adminSidebarCategoryOrder: { key: string; label: LazyString }[] = [
  { key: 'system', label: () => getTranslations().t('pages.admin.categories.system', {}) },
  { key: 'infrastructure', label: () => getTranslations().t('pages.admin.categories.infrastructure', {}) },
  { key: 'access', label: () => getTranslations().t('pages.admin.categories.access', {}) },
  { key: 'eggs', label: () => getTranslations().t('pages.admin.categories.eggs', {}) },
  { key: 'databases', label: () => getTranslations().t('pages.admin.categories.databases', {}) },
  { key: 'storage', label: () => getTranslations().t('pages.admin.categories.storage', {}) },
];

// plugged.host: no managed database instances, mounts, devices or Extensions page (docs/upstream.md)
const routes: AdminRouteDefinition[] = [
  {
    name: () => getTranslations().t('pages.admin.home.title', {}),
    icon: faBuilding,
    path: '/*',
    activeMatches: ['/admin/updates', '/admin/health'],
    element: AdminHome,
    exact: true,
  },

  {
    name: () => getTranslations().t('pages.admin.locations.title', {}),
    icon: faEarthAmerica,
    path: '/locations/*',
    element: AdminLocations,
    permission: ['locations.*'],
    category: 'infrastructure',
  },
  {
    name: () => getTranslations().t('pages.admin.nodes.title', {}),
    icon: faServer,
    path: '/nodes/*',
    element: AdminNodes,
    permission: ['nodes.*'],
    category: 'infrastructure',
  },
  {
    name: () => getTranslations().t('pages.admin.servers.title', {}),
    icon: faComputer,
    path: '/servers/*',
    element: AdminServers,
    permission: ['servers.*'],
    category: 'infrastructure',
  },
  // plugged.host: abuse protection (docs/features/abuse.md)
  // plugged.host: the resource presets players create servers with
  {
    name: () => getTranslations().t('pages.admin.plans.title', {}),
    icon: faLayerGroup,
    path: '/plans/*',
    element: AdminPlans,
    permission: ['plans.*'],
    category: 'infrastructure',
  },
  {
    name: () => getTranslations().t('pages.admin.abuse.title', {}),
    icon: faShieldHalved,
    path: '/abuse',
    element: AdminAbuse,
    permission: ['abuse.*'],
    category: 'infrastructure',
  },

  {
    name: () => getTranslations().t('pages.admin.nests.title', {}),
    icon: faCrow,
    path: '/nests/*',
    element: AdminNests,
    permission: ['nests.*'],
    category: 'eggs',
  },
  {
    name: () => getTranslations().t('pages.admin.eggLibrary.title', {}),
    icon: faEgg,
    path: '/egg-library/*',
    element: AdminEggLibrary,
    permission: ['eggs.read'],
    category: 'eggs',
  },
  {
    name: () => getTranslations().t('pages.admin.eggConfigurations.title', {}),
    icon: faCogs,
    path: '/egg-configurations/*',
    element: AdminEggConfigurations,
    permission: ['egg-configurations.*'],
    category: 'eggs',
  },
  {
    name: () => getTranslations().t('pages.admin.eggRepositories.title', {}),
    icon: faDownload,
    path: '/egg-repositories/*',
    element: AdminEggRepositories,
    permission: ['egg-repositories.*'],
    category: 'eggs',
  },

  {
    name: () => getTranslations().t('pages.admin.databaseHosts.title', {}),
    icon: faDatabase,
    path: '/database-hosts/*',
    element: AdminDatabaseHosts,
    permission: ['database-hosts.*'],
    category: 'databases',
  },
  {
    name: () => getTranslations().t('pages.admin.backupConfigurations.title', {}),
    icon: faFileZipper,
    path: '/backup-configurations/*',
    element: AdminBackupConfigurations,
    permission: ['backup-configurations.*'],
    category: 'storage',
  },
  {
    name: () => getTranslations().t('pages.admin.systemBackupPolicies.title', {}),
    icon: faClockRotateLeft,
    path: '/system-backup-policies/*',
    element: AdminSystemBackupPolicies,
    permission: ['system-backup-policies.*'],
    category: 'storage',
  },

  {
    name: () => getTranslations().t('pages.admin.users.title', {}),
    icon: faUsers,
    path: '/users/*',
    element: AdminUsers,
    permission: ['users.*'],
    category: 'access',
  },
  {
    name: () => getTranslations().t('pages.admin.roles.title', {}),
    icon: faScroll,
    path: '/roles/*',
    element: AdminRoles,
    permission: ['roles.*'],
    category: 'access',
  },
  {
    name: () => getTranslations().t('pages.admin.oAuthProviders.title', {}),
    icon: faFingerprint,
    path: '/oauth-providers/*',
    element: AdminOAuthProviders,
    // plugged.host: no OAuth providers while noprofit.space owns sign-in
    filter: () => !useGlobalStore.getState().settings.noprofit?.enabled,
    permission: ['oauth-providers.*'],
    category: 'access',
  },
  {
    name: () => getTranslations().t('pages.admin.activity.title', {}),
    icon: faBriefcase,
    path: '/activity',
    element: AdminActivity,
    permission: ['activity.*'],
    category: 'access',
  },

  {
    name: () => getTranslations().t('pages.admin.settings.title', {}),
    icon: faWrench,
    path: '/settings/*',
    element: AdminSettings,
    permission: ['settings.*'],
    category: 'system',
  },
  {
    name: () => getTranslations().t('pages.admin.announcements.title', {}),
    icon: faBullhorn,
    path: '/announcements/*',
    element: AdminAnnouncements,
    permission: ['announcements.*'],
    category: 'system',
  },
  // plugged.host: the public status page (docs/features/status.md)
  {
    name: () => getTranslations().t('pages.admin.status.title', {}),
    icon: faHeartPulse,
    path: '/status/*',
    element: AdminStatus,
    permission: ['status.*'],
    category: 'system',
  },
  {
    name: () => getTranslations().t('pages.admin.assets.title', {}),
    icon: faFolderOpen,
    path: '/assets',
    element: AdminAssets,
    permission: ['assets.*'],
    category: 'system',
  },
];

export default routes;
