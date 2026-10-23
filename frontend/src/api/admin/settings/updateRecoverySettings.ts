// plugged.host: Admin › Settings › Hosting › Recovery
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsRecoverySchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsRecoverySchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    recovery: serializeForApi(adminSettingsRecoverySchema, data),
  });
};
