// plugged.host: the Pelican panel's page paths, each sent to the page that replaced it, so old bookmarks and links keep working (docs/features/old-paths.md).

/** Under `/`: Pelican's app panel and plugged.host's own pages on it. */
export const legacyDashboardRedirects: Record<string, string> = {
  '/profile': '/account',
  '/create-server': '/create',
  '/store': '/account/store',
  '/plus': '/account/membership',
  // the Pelican panel already sent its old token store here
  '/token-store': '/account/membership',
  '/tokens': '/account/tokens',
  '/welcome': '/',
};

/** Under `/server/:id` (Pelican's short server id is the same as the panel's). */
export const legacyServerRedirects: Record<string, string> = {
  '/console': '',
  '/installing': '',
  '/suspended': '',
  '/start': '',
  '/allocations': '/network',
  '/activity-logs': '/activity',
  '/users': '/subusers',
  '/content': '/minecraft/content',
  '/setup': '/minecraft/setup',
  '/game-settings': '/minecraft/settings',
  '/worlds': '/minecraft/worlds',
  '/players': '/minecraft/players',
  '/server-config': '/fivem/config',
  '/frameworks': '/fivem/frameworks',
  '/player-list': '/fivem/players',
  '/artifacts': '/fivem/artifacts',
  '/enhanced': '/fivem/enhanced',
};
