// plugged.host: Admin › Settings › Hosting › Games
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsGamesSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsGamesSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    games: serializeForApi(adminSettingsGamesSchema, data),
  });
};
