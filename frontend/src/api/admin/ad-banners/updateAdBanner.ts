// plugged.host: Admin › Sponsored Banners
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminAdBannerUpdateSchema } from '@/lib/schemas/admin/adBanners.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, data: z.infer<typeof adminAdBannerUpdateSchema>): Promise<void> => {
  await axiosInstance.patch(`/api/admin/ad-banners/${uuid}`, serializeForApi(adminAdBannerUpdateSchema, data));
};
