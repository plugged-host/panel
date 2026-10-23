import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsUpdatesSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsUpdatesSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    updates: serializeForApi(adminSettingsUpdatesSchema, data),
  });
};
