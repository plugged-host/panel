// plugged.host: Admin › Settings › Welcome Tour
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsTourSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsTourSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    tour: serializeForApi(adminSettingsTourSchema, data),
  });
};
