// plugged.host: Admin › Settings › Hosting › Minecraft (the games section's Minecraft fields)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsMinecraftSchema } from '@/lib/schemas/admin/settings.ts';

export default async (data: z.infer<typeof adminSettingsMinecraftSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    games: {
      // empty keeps the stored key
      minecraft_curseforge_key: data.minecraftCurseforgeKey.trim() || undefined,
      minecraft_spigot: data.minecraftSpigot,
      minecraft_user_agent: data.minecraftUserAgent,
    },
  });
};
