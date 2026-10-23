// plugged.host: Admin › Settings › Economy
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsEconomySchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsEconomySchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    economy: serializeForApi(adminSettingsEconomySchema, data),
  });
};
