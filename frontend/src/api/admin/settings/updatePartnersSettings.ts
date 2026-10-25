// plugged.host: Admin › Settings › Economy › Partners (docs/features/partners.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsPartnersSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsPartnersSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    partners: serializeForApi(adminSettingsPartnersSchema, data),
  });
};
