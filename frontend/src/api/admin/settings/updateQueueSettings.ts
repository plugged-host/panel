// plugged.host: Admin › Settings › Hosting › Start Queue
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsQueueSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsQueueSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    queue: serializeForApi(adminSettingsQueueSchema, data),
  });
};
