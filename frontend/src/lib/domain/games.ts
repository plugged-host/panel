// plugged.host: which game pages a server shows, from its egg's features (docs/features/minecraft.md, fivem.md,
// subdomains.md)

/** Minecraft eggs carry the `eula` feature, FiveM and RedM eggs `cfx`. */
const gameRoutes: [prefix: string, feature: (feature: string) => boolean][] = [
  ['/minecraft', (feature) => feature === 'eula'],
  ['/fivem', (feature) => feature === 'cfx'],
  // as shared/src/subdomains/records.rs decides: a known game, an `srv-*` feature or `subdomain`
  [
    '/subdomains',
    (feature) => feature === 'eula' || feature === 'cfx' || feature === 'subdomain' || feature.startsWith('srv-'),
  ],
];

/** Whether a server route belongs to a game the server's egg does not run. */
export function isOtherGameRoute(path: string, features: string[] | undefined): boolean {
  const route = gameRoutes.find(([prefix]) => path === prefix || path.startsWith(`${prefix}/`));

  return !!route && !(features ?? []).some(route[1]);
}
