// plugged.host: which game pages a server shows, from its egg's features (docs/features/minecraft.md)

/** Minecraft eggs carry the `eula` feature. */
const gameRoutes: [prefix: string, feature: (feature: string) => boolean][] = [
  ['/minecraft', (feature) => feature === 'eula'],
];

/** Whether a server route belongs to a game the server's egg does not run. */
export function isOtherGameRoute(path: string, features: string[] | undefined): boolean {
  const route = gameRoutes.find(([prefix]) => path === prefix || path.startsWith(`${prefix}/`));

  return !!route && !(features ?? []).some(route[1]);
}
