// plugged.host: Admin › Settings › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsStatusSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsStatusSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    status: serializeForApi(adminSettingsStatusSchema, data),
  });
};
