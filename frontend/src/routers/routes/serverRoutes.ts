import {
  faBoxArchive,
  faBriefcase,
  faCode, // plugged.host
  faCog,
  faCubesStacked, // plugged.host
  faDatabase,
  faEarthAmericas, // plugged.host
  faFileCode, // plugged.host
  faFolderOpen,
  faGlobe, // plugged.host
  faLayerGroup, // plugged.host
  faNetworkWired,
  faPlay,
  faPuzzlePiece, // plugged.host
  faRightLeft,
  faSliders, // plugged.host
  faStopwatch,
  faTerminal,
  faUserGroup, // plugged.host
  faUsers,
} from '@fortawesome/free-solid-svg-icons';
import { lazy } from 'react';
import type { ServerRouteDefinition } from 'shared';
import ServerActivity from '@/pages/server/activity/ServerActivity.tsx';
import ServerBackups from '@/pages/server/backups/ServerBackups.tsx';
import ServerSystemBackups from '@/pages/server/backups/system/ServerSystemBackups.tsx';
import DatabaseExplorerView from '@/pages/server/databases/explorer/DatabaseExplorerView.tsx';
import ServerDatabases from '@/pages/server/databases/ServerDatabases.tsx';
import ServerDevbox from '@/pages/server/devbox/ServerDevbox.tsx'; // plugged.host
import FileSqliteQuery from '@/pages/server/files/editor/FileSqliteQuery.tsx';
import ServerFiles from '@/pages/server/files/ServerFiles.tsx';
import ServerFilesTrash from '@/pages/server/files/trash/ServerFilesTrash.tsx';
import ServerFivemArtifacts from '@/pages/server/fivem/artifacts/ServerFivemArtifacts.tsx'; // plugged.host
import ServerFivemConfig from '@/pages/server/fivem/config/ServerFivemConfig.tsx'; // plugged.host
import ServerFivemFrameworks from '@/pages/server/fivem/frameworks/ServerFivemFrameworks.tsx'; // plugged.host
import ServerFivemPlayers from '@/pages/server/fivem/players/ServerFivemPlayers.tsx'; // plugged.host
import ServerLimbo from '@/pages/server/limbo/ServerLimbo.tsx'; // plugged.host
import ServerMinecraftBrowse from '@/pages/server/minecraft/content/ServerMinecraftBrowse.tsx'; // plugged.host
import ServerMinecraftContent from '@/pages/server/minecraft/content/ServerMinecraftContent.tsx'; // plugged.host
import ServerMinecraftModpacks from '@/pages/server/minecraft/content/ServerMinecraftModpacks.tsx'; // plugged.host
import ServerMinecraftPlayers from '@/pages/server/minecraft/players/ServerMinecraftPlayers.tsx'; // plugged.host
import ServerMinecraftSettings from '@/pages/server/minecraft/settings/ServerMinecraftSettings.tsx'; // plugged.host
import ServerMinecraftSetup from '@/pages/server/minecraft/setup/ServerMinecraftSetup.tsx'; // plugged.host
import ServerMinecraftWorlds from '@/pages/server/minecraft/worlds/ServerMinecraftWorlds.tsx'; // plugged.host
import ServerNetwork from '@/pages/server/network/ServerNetwork.tsx';
import ServerQueue from '@/pages/server/queue/ServerQueue.tsx'; // plugged.host
import ScheduleView from '@/pages/server/schedules/ScheduleView.tsx';
import ServerSchedules from '@/pages/server/schedules/ServerSchedules.tsx';
import ServerSettings from '@/pages/server/settings/ServerSettings.tsx';
import ServerStartup from '@/pages/server/startup/ServerStartup.tsx';
import ServerSubdomains from '@/pages/server/subdomains/ServerSubdomains.tsx'; // plugged.host
import ServerSubusers from '@/pages/server/subusers/ServerSubusers.tsx';
import ServerTransfer from '@/pages/server/transfer/ServerTransfer.tsx'; // plugged.host
import ServerWaitingRoom from '@/pages/server/waiting/ServerWaitingRoom.tsx'; // plugged.host
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
  // plugged.host: Minecraft servers only (lib/domain/games.ts, docs/features/minecraft.md)
  {
    name: () => getTranslations().t('pages.server.minecraft.content.title', {}),
    icon: faPuzzlePiece,
    path: '/minecraft/content',
    element: ServerMinecraftContent,
    permission: 'minecraft.content',
  },
  {
    name: undefined,
    path: '/minecraft/content/browse',
    element: ServerMinecraftBrowse,
    permission: 'minecraft.content',
  },
  {
    name: undefined,
    path: '/minecraft/content/modpacks',
    element: ServerMinecraftModpacks,
    permission: 'minecraft.content',
  },
  {
    name: undefined,
    path: '/minecraft/setup',
    element: ServerMinecraftSetup,
    permission: 'minecraft.settings',
  },
  {
    name: () => getTranslations().t('pages.server.minecraft.settings.title', {}),
    icon: faSliders,
    path: '/minecraft/settings',
    element: ServerMinecraftSettings,
    permission: 'minecraft.settings',
  },
  {
    name: () => getTranslations().t('pages.server.minecraft.players.title', {}),
    icon: faUserGroup,
    path: '/minecraft/players',
    element: ServerMinecraftPlayers,
    permission: 'minecraft.players',
  },
  {
    name: () => getTranslations().t('pages.server.minecraft.worlds.title', {}),
    icon: faEarthAmericas,
    path: '/minecraft/worlds',
    element: ServerMinecraftWorlds,
    permission: 'minecraft.worlds',
  },
  // plugged.host: FiveM and RedM servers only (lib/domain/games.ts, docs/features/fivem.md)
  {
    name: () => getTranslations().t('pages.server.fivem.config.title', {}),
    icon: faFileCode,
    path: '/fivem/config',
    element: ServerFivemConfig,
    permission: 'fivem.config',
  },
  {
    name: () => getTranslations().t('pages.server.fivem.frameworks.title', {}),
    icon: faLayerGroup,
    path: '/fivem/frameworks',
    element: ServerFivemFrameworks,
    permission: 'fivem.frameworks',
  },
  {
    name: () => getTranslations().t('pages.server.fivem.players.title', {}),
    icon: faUserGroup,
    path: '/fivem/players',
    element: ServerFivemPlayers,
    permission: 'fivem.players',
  },
  {
    name: () => getTranslations().t('pages.server.fivem.artifacts.title', {}),
    icon: faCubesStacked,
    path: '/fivem/artifacts',
    element: ServerFivemArtifacts,
    permission: 'fivem.artifacts',
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
  // plugged.host: servers whose game gets subdomains (lib/domain/games.ts, docs/features/subdomains.md)
  {
    name: () => getTranslations().t('pages.server.subdomains.title', {}),
    icon: faGlobe,
    path: '/subdomains',
    element: ServerSubdomains,
    permission: 'subdomains.read',
    filter: () => !!useGlobalStore.getState().settings.games?.subdomainsEnabled,
  },
  // plugged.host: a devbox with the server's files (docs/features/devboxes.md)
  {
    name: () => getTranslations().t('pages.server.devbox.title', {}),
    icon: faCode,
    path: '/devbox',
    element: ServerDevbox,
    permission: 'devbox.connect',
    filter: () => !!useGlobalStore.getState().settings.games?.devboxesEnabled,
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
  // plugged.host: a start in the waiting room (docs/features/ads.md#waiting-room)
  {
    name: undefined,
    path: '/waiting',
    element: ServerWaitingRoom,
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
