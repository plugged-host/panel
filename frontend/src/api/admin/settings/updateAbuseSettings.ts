// plugged.host: Admin › Settings › Abuse
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsAbuseSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsAbuseSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    abuse: serializeForApi(adminSettingsAbuseSchema, data),
  });
};
