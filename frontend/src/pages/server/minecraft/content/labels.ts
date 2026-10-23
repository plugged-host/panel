// plugged.host: the names of Minecraft loaders and content sources (docs/features/minecraft.md)
import { z } from 'zod';
import { minecraftLoaderSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';

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
