// plugged.host: which game pages a server shows, from its egg (docs/features/minecraft.md, fivem.md,
// subdomains.md)

/** What of a server's egg decides its game pages. */
export interface GameEgg {
  features: string[];
  /** `java` for a Minecraft server, `proxy` for Velocity and the like, `bedrock` (shared/src/minecraft/mod.rs). */
  minecraft?: 'java' | 'proxy' | 'bedrock' | null;
}

/** A Minecraft: Java Edition game server: the Minecraft pages; not a proxy, which carries `eula` too. */
export function isMinecraftServer(egg: GameEgg | undefined): boolean {
  return egg?.minecraft === 'java';
}

/** Any Minecraft (a server, a proxy, Bedrock): the console's log check. */
export function hasMinecraftLogs(egg: GameEgg | undefined): boolean {
  return !!egg?.minecraft;
}

const gameRoutes: [prefix: string, matches: (egg: GameEgg) => boolean][] = [
  ['/minecraft', isMinecraftServer],
  // FiveM and RedM eggs carry `cfx`
  ['/fivem', (egg) => egg.features.includes('cfx')],
  // as shared/src/subdomains/records.rs decides: a known game, an `srv-*` feature or `subdomain`
  [
    '/subdomains',
    (egg) =>
      isMinecraftServer(egg) ||
      egg.features.some((feature) => feature === 'cfx' || feature === 'subdomain' || feature.startsWith('srv-')),
  ],
];

/** Whether a server route belongs to a game the server's egg does not run. */
export function isOtherGameRoute(path: string, egg: GameEgg | undefined): boolean {
  const route = gameRoutes.find(([prefix]) => path === prefix || path.startsWith(`${prefix}/`));

  return !!route && !(egg && route[1](egg));
}
