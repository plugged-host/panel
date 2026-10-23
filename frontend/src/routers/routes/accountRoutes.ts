import {
  faBoxArchive, // plugged.host
  faBriefcase,
  faClipboard,
  faCloud,
  faFingerprint,
  faKey,
  faKeyboard,
  faUnlockKeyhole,
  faUser,
  faUserSecret,
} from '@fortawesome/free-solid-svg-icons';
import type { RouteDefinition } from 'shared';
import DashboardAccount from '@/pages/dashboard/account/DashboardAccount.tsx';
import DashboardActivity from '@/pages/dashboard/activity/DashboardActivity.tsx';
import DashboardApiKeys from '@/pages/dashboard/api-keys/DashboardApiKeys.tsx';
import DashboardApiKeysCreate from '@/pages/dashboard/api-keys/DashboardApiKeysCreate.tsx';
import DashboardApiKeysUpdate from '@/pages/dashboard/api-keys/DashboardApiKeysUpdate.tsx';
import DashboardBackups from '@/pages/dashboard/backups/DashboardBackups.tsx'; // plugged.host
import DashboardCommandSnippets from '@/pages/dashboard/command-snippets/DashboardCommandSnippets.tsx';
import DashboardOAuthLinks from '@/pages/dashboard/oauth-links/DashboardOAuthLinks.tsx';
import DashboardSecurityKeys from '@/pages/dashboard/security-keys/DashboardSecurityKeys.tsx';
import DashboardSessions from '@/pages/dashboard/sessions/DashboardSessions.tsx';
import DashboardShortcuts from '@/pages/dashboard/shortcuts/DashboardShortcuts.tsx';
import DashboardSshKeys from '@/pages/dashboard/ssh-keys/DashboardSshKeys.tsx';
import { getTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

// plugged.host: no security keys or OAuth links while noprofit.space owns sign-in
const noprofitConnected = () => !!useGlobalStore.getState().settings.noprofit?.enabled;

const routes: RouteDefinition[] = [
  {
    name: () => getTranslations().t('pages.account.account.title', {}),
    icon: faUser,
    path: '/',
    element: DashboardAccount,
    exact: true,
  },
  {
    name: () => getTranslations().t('pages.account.securityKeys.title', {}),
    icon: faUnlockKeyhole,
    path: '/security-keys',
    element: DashboardSecurityKeys,
    filter: () => !noprofitConnected(),
  },
  {
    name: () => getTranslations().t('pages.account.apiKeys.title', {}),
    icon: faCloud,
    path: '/api-keys',
    element: DashboardApiKeys,
  },
  {
    name: undefined,
    path: '/api-keys/create',
    element: DashboardApiKeysCreate,
  },
  {
    name: undefined,
    path: '/api-keys/update',
    element: DashboardApiKeysUpdate,
  },
  {
    name: () => getTranslations().t('pages.account.sshKeys.title', {}),
    icon: faKey,
    path: '/ssh-keys',
    element: DashboardSshKeys,
  },
  {
    name: () => getTranslations().t('pages.account.commandSnippets.title', {}),
    icon: faClipboard,
    path: '/command-snippets',
    element: DashboardCommandSnippets,
  },
  {
    name: () => getTranslations().t('pages.account.oauthLinks.title', {}),
    icon: faFingerprint,
    path: '/oauth-links',
    element: DashboardOAuthLinks,
    filter: () => !noprofitConnected(),
  },
  {
    name: () => getTranslations().t('pages.account.sessions.title', {}),
    icon: faUserSecret,
    path: '/sessions',
    element: DashboardSessions,
  },
  {
    name: () => getTranslations().t('pages.account.shortcuts.title', {}),
    icon: faKeyboard,
    path: '/shortcuts',
    element: DashboardShortcuts,
  },
  // plugged.host: the newest copy of each server, deleted ones included (docs/features/backups-page.md)
  {
    name: () => getTranslations().t('pages.account.backups.title', {}),
    icon: faBoxArchive,
    path: '/backups',
    element: DashboardBackups,
  },
  {
    name: () => getTranslations().t('pages.account.activity.title', {}),
    icon: faBriefcase,
    path: '/activity',
    element: DashboardActivity,
  },
];

export default routes;
