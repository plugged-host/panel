// plugged.host: Admin › Settings › Accounts › Reviews (docs/features/reviews.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminSettingsReviewsSchema } from '@/lib/schemas/admin/settings.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminSettingsReviewsSchema>): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    reviews: serializeForApi(adminSettingsReviewsSchema, data),
  });
};
