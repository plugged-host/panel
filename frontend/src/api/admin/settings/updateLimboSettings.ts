// plugged.host: Admin › Settings › Hosting › Limbo
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsLimboSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsLimboSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    limbo: serializeForApi(adminSettingsLimboSchema, data),
  });
};
