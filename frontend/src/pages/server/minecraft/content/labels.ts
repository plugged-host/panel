// plugged.host: the names of Minecraft loaders and content sources (docs/features/minecraft.md)
import { z } from 'zod';
import {
  minecraftLoaderSchema,
  minecraftSourceSchema,
  minecraftVersionSchema,
} from '@/lib/schemas/server/minecraft.ts';

export const minecraftLoaderLabels: Record<z.infer<typeof minecraftLoaderSchema>, string> = {
  paper: 'Paper',
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
};

export const minecraftSourceLabels: Record<z.infer<typeof minecraftSourceSchema>, string> = {
  modrinth: 'Modrinth',
  curseforge: 'CurseForge',
  spigotmc: 'SpigotMC',
};

/**
 * A build as the Pelican panel lists it: "2.21.1 — 1.21.4, 1.21.3, 1.21.1… (beta)", the number, up
 * to three Minecraft versions, and the channel when it is not a release.
 */
export function minecraftVersionLabel(version: z.infer<typeof minecraftVersionSchema>): string {
  const games = version.gameVersions.length
    ? ` — ${version.gameVersions.slice(0, 3).join(', ')}${version.gameVersions.length > 3 ? '…' : ''}`
    : '';
  const channel = version.channel && version.channel !== 'release' ? ` (${version.channel})` : '';

  return `${version.number || version.name}${games}${channel}`;
}
