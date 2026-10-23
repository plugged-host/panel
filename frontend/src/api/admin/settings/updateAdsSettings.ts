// plugged.host: Admin › Settings › Ads
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsAdsSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: Partial<z.infer<typeof adminSettingsAdsSchema>>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    ads: serializeForApi(adminSettingsAdsSchema.partial(), data),
  });
};
