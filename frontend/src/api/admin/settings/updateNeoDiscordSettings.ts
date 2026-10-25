// plugged.host: Admin › Settings › Accounts › neo on Discord (docs/features/neo-discord.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsNeoDiscordSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsNeoDiscordSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    neo_discord: serializeForApi(adminSettingsNeoDiscordSchema, data),
  });
};
