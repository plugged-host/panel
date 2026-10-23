import {
  faBoxArchive,
  faBriefcase,
  faCog,
  faDatabase,
  faFolderOpen,
  faNetworkWired,
  faPlay,
  faRightLeft,
  faStopwatch,
  faTerminal,
  faUsers,
} from '@fortawesome/free-solid-svg-icons';
import { lazy } from 'react';
import type { ServerRouteDefinition } from 'shared';
import ServerActivity from '@/pages/server/activity/ServerActivity.tsx';
import ServerBackups from '@/pages/server/backups/ServerBackups.tsx';
import ServerSystemBackups from '@/pages/server/backups/system/ServerSystemBackups.tsx';
import DatabaseExplorerView from '@/pages/server/databases/explorer/DatabaseExplorerView.tsx';
import ServerDatabases from '@/pages/server/databases/ServerDatabases.tsx';
import FileSqliteQuery from '@/pages/server/files/editor/FileSqliteQuery.tsx';
import ServerFiles from '@/pages/server/files/ServerFiles.tsx';
import ServerFilesTrash from '@/pages/server/files/trash/ServerFilesTrash.tsx';
import ServerLimbo from '@/pages/server/limbo/ServerLimbo.tsx'; // plugged.host
import ServerNetwork from '@/pages/server/network/ServerNetwork.tsx';
import ServerQueue from '@/pages/server/queue/ServerQueue.tsx'; // plugged.host
import ScheduleView from '@/pages/server/schedules/ScheduleView.tsx';
import ServerSchedules from '@/pages/server/schedules/ServerSchedules.tsx';
import ServerSettings from '@/pages/server/settings/ServerSettings.tsx';
import ServerStartup from '@/pages/server/startup/ServerStartup.tsx';
import ServerSubusers from '@/pages/server/subusers/ServerSubusers.tsx';
import ServerTransfer from '@/pages/server/transfer/ServerTransfer.tsx'; // plugged.host
import { getTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

const ServerConsole = lazy(() => import('@/pages/server/console/ServerConsole.tsx'));
const PopoutConsole = lazy(() => import('@/pages/server/console/PopoutConsole.tsx'));
const ServerFilesEditor = lazy(() => import('@/pages/server/files/editor/FileEditor.tsx'));
const FileRevisionDiff = lazy(() => import('@/pages/server/files/editor/FileRevisionDiff.tsx'));

// plugged.host: no managed database instances, firewall, private network, mounts or devices
// (docs/upstream.md)
const routes: ServerRouteDefinition[] = [
  {
    name: () => getTranslations().t('pages.server.console.title', {}),
    icon: faTerminal,
    path: '/',
    element: ServerConsole,
    exact: true,
    permission: null,
  },
  {
    name: undefined,
    path: '/console/popout',
    element: PopoutConsole,
    permission: null,
  },
  {
    name: () => getTranslations().t('pages.server.files.title', {}),
    icon: faFolderOpen,
    path: '/files',
    element: ServerFiles,
    permission: 'files.read',
  },
  {
    name: undefined,
    path: '/files/trash',
    element: ServerFilesTrash,
    permission: 'files.read',
  },
  {
    name: undefined,
    path: '/files/diff',
    element: FileRevisionDiff,
    permission: 'files.read-content',
  },
  {
    name: undefined,
    path: '/files/sqlite',
    element: FileSqliteQuery,
    permission: 'files.query-raw',
  },
  {
    name: undefined,
    path: '/files/:action',
    element: ServerFilesEditor,
    permission: ['files.read-content', 'files.create'],
  },
  {
    name: () => getTranslations().t('pages.server.databases.title', {}),
    icon: faDatabase,
    path: '/databases',
    element: ServerDatabases,
    permission: 'databases.read',
  },
  {
    name: undefined,
    path: '/databases/:id/explore',
    element: DatabaseExplorerView,
    permission: 'databases.query',
  },
  {
    name: () => getTranslations().t('pages.server.schedules.title', {}),
    icon: faStopwatch,
    path: '/schedules',
    element: ServerSchedules,
    permission: 'schedules.read',
  },
  {
    name: undefined,
    path: '/schedules/:id',
    element: ScheduleView,
    permission: 'schedules.read',
  },
  {
    name: () => getTranslations().t('pages.server.subusers.title', {}),
    icon: faUsers,
    path: '/subusers',
    element: ServerSubusers,
    permission: 'subusers.read',
  },
  {
    name: () => getTranslations().t('pages.server.backups.title', {}),
    icon: faBoxArchive,
    path: '/backups',
    element: ServerBackups,
    permission: 'backups.read',
  },
  {
    name: undefined,
    path: '/backups/system',
    element: ServerSystemBackups,
    permission: 'backups.read',
  },
  {
    name: () => getTranslations().t('pages.server.network.title', {}),
    icon: faNetworkWired,
    path: '/network',
    element: ServerNetwork,
    permission: 'allocations.read',
  },
  {
    name: () => getTranslations().t('pages.server.startup.title', {}),
    icon: faPlay,
    path: '/startup',
    element: ServerStartup,
    permission: 'startup.read',
  },
  {
    name: () => getTranslations().t('pages.server.settings.title', {}),
    icon: faCog,
    path: '/settings',
    element: ServerSettings,
    permission: ['settings.*'],
  },
  // plugged.host: players moving their server (docs/features/transfers.md)
  {
    name: () => getTranslations().t('pages.server.transfer.title', {}),
    icon: faRightLeft,
    path: '/transfer',
    element: ServerTransfer,
    permission: 'settings.transfer',
    filter: () => !!useGlobalStore.getState().settings.games?.transferEnabled,
  },
  // plugged.host: a start waiting for room on a busy node (docs/features/start-queue.md)
  {
    name: undefined,
    path: '/queue',
    element: ServerQueue,
    permission: ['control.start', 'control.restart'],
  },
  // plugged.host: a parked server and its restore (docs/features/limbo.md)
  {
    name: undefined,
    path: '/limbo',
    element: ServerLimbo,
    permission: ['control.start', 'control.restart'],
  },
  {
    name: () => getTranslations().t('pages.server.activity.title', {}),
    icon: faBriefcase,
    path: '/activity',
    element: ServerActivity,
    permission: 'activity.read',
  },
];

export default routes;
