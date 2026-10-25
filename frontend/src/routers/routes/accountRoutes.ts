import { faDiscord } from '@fortawesome/free-brands-svg-icons'; // plugged.host
import {
  faBoxArchive, // plugged.host
  faBriefcase,
  faClipboard,
  faCloud,
  faCoins, // plugged.host
  faFingerprint,
  faHandshake, // plugged.host
  faKey,
  faKeyboard,
  faLock, // plugged.host
  faMedal, // plugged.host
  faStore, // plugged.host
  faUnlockKeyhole,
  faUser,
  faUserSecret,
} from '@fortawesome/free-solid-svg-icons';
import { lazy } from 'react';
import type { RouteDefinition } from 'shared';
import { getTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

// plugged.host: each account page is its own chunk, so opening the dashboard does not download
// every page it can lead to.
const DashboardAccount = lazy(() => import('@/pages/dashboard/account/DashboardAccount.tsx'));
const DashboardActivity = lazy(() => import('@/pages/dashboard/activity/DashboardActivity.tsx'));
const DashboardApiKeys = lazy(() => import('@/pages/dashboard/api-keys/DashboardApiKeys.tsx'));
const DashboardApiKeysCreate = lazy(() => import('@/pages/dashboard/api-keys/DashboardApiKeysCreate.tsx'));
const DashboardApiKeysUpdate = lazy(() => import('@/pages/dashboard/api-keys/DashboardApiKeysUpdate.tsx'));
const DashboardBackups = lazy(() => import('@/pages/dashboard/backups/DashboardBackups.tsx')); // plugged.host
const DashboardCommandSnippets = lazy(() => import('@/pages/dashboard/command-snippets/DashboardCommandSnippets.tsx'));
const DashboardDevboxOpen = lazy(() => import('@/pages/dashboard/devbox/DashboardDevboxOpen.tsx')); // plugged.host
const DashboardDiscord = lazy(() => import('@/pages/dashboard/discord/DashboardDiscord.tsx')); // plugged.host
const DashboardMembership = lazy(() => import('@/pages/dashboard/membership/DashboardMembership.tsx')); // plugged.host
const DashboardOAuthLinks = lazy(() => import('@/pages/dashboard/oauth-links/DashboardOAuthLinks.tsx'));
const DashboardPanelAccess = lazy(() => import('@/pages/dashboard/panel-access/DashboardPanelAccess.tsx')); // plugged.host
const DashboardPartners = lazy(() => import('@/pages/dashboard/partners/DashboardPartners.tsx')); // plugged.host
const DashboardSecurityKeys = lazy(() => import('@/pages/dashboard/security-keys/DashboardSecurityKeys.tsx'));
const DashboardSessions = lazy(() => import('@/pages/dashboard/sessions/DashboardSessions.tsx'));
const DashboardShortcuts = lazy(() => import('@/pages/dashboard/shortcuts/DashboardShortcuts.tsx'));
const DashboardSshKeys = lazy(() => import('@/pages/dashboard/ssh-keys/DashboardSshKeys.tsx'));
const DashboardStore = lazy(() => import('@/pages/dashboard/store/DashboardStore.tsx')); // plugged.host
const DashboardTokens = lazy(() => import('@/pages/dashboard/tokens/DashboardTokens.tsx')); // plugged.host

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
  // plugged.host: the economy (docs/features/economy.md)
  {
    name: () => getTranslations().t('pages.account.store.title', {}),
    icon: faStore,
    path: '/store',
    element: DashboardStore,
    filter: () => !!useGlobalStore.getState().settings.economy?.creditsEnabled,
  },
  {
    name: () => getTranslations().t('pages.account.tokens.title', {}),
    icon: faCoins,
    path: '/tokens',
    element: DashboardTokens,
    filter: () => !!useGlobalStore.getState().settings.economy?.tokensEnabled,
  },
  {
    name: () =>
      useGlobalStore.getState().settings.economy?.plusName || getTranslations().t('pages.account.membership.title', {}), // plugged.host: named like the Pelican panel's row
    icon: faMedal,
    path: '/membership',
    element: DashboardMembership,
    filter: () => !!useGlobalStore.getState().settings.economy?.membershipEnabled,
  },
  // plugged.host: the panel's own monthly price (docs/features/economy.md)
  {
    name: () => getTranslations().t('pages.account.panelAccess.title', {}),
    icon: faLock,
    path: '/panel-access',
    element: DashboardPanelAccess,
    filter: () => !!useGlobalStore.getState().settings.economy?.panelAccessEnabled,
  },
  // plugged.host: Discord rewards (docs/features/discord.md)
  {
    name: () => getTranslations().t('pages.account.discord.title', {}),
    icon: faDiscord,
    path: '/discord',
    element: DashboardDiscord,
    filter: () => !!useGlobalStore.getState().settings.discord?.enabled,
  },
  // plugged.host: the partner program (docs/features/partners.md)
  {
    name: () => getTranslations().t('pages.account.partners.title', {}),
    icon: faHandshake,
    path: '/partners',
    element: DashboardPartners,
    filter: () => !!useGlobalStore.getState().settings.partners?.enabled,
  },
  // plugged.host: back to a devbox in the browser (docs/features/devboxes.md)
  {
    name: undefined,
    path: '/devbox/open',
    element: DashboardDevboxOpen,
  },
  {
    name: () => getTranslations().t('pages.account.activity.title', {}),
    icon: faBriefcase,
    path: '/activity',
    element: DashboardActivity,
  },
];

export default routes;
