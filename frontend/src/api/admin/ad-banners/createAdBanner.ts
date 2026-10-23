// plugged.host: Admin › Sponsored Banners
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminAdBannerSchema, adminAdBannerUpdateSchema } from '@/lib/schemas/admin/adBanners.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  bannerData: z.infer<typeof adminAdBannerUpdateSchema>,
): Promise<z.infer<typeof adminAdBannerSchema>> => {
  const { data } = await axiosInstance.post(
    '/api/admin/ad-banners',
    serializeForApi(adminAdBannerUpdateSchema, bannerData),
  );
  return parseFromApi(adminAdBannerSchema, data.ad_banner);
};
