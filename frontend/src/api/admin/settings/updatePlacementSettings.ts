// plugged.host: Admin › Settings › Hosting › Placement
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsPlacementSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsPlacementSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    placement: serializeForApi(adminSettingsPlacementSchema, data),
  });
};
