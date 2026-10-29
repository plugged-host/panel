// plugged.host: Admin › Settings › Economy › Server Upgrades (docs/features/server-upgrades.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsUpgradesSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsUpgradesSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    upgrades: serializeForApi(adminSettingsUpgradesSchema, data),
  });
};
