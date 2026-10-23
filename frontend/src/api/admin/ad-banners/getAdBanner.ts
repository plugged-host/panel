// plugged.host: Admin › Sponsored Banners
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminAdBannerSchema } from '@/lib/schemas/admin/adBanners.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof adminAdBannerSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/ad-banners/${uuid}`);
  return parseFromApi(adminAdBannerSchema, data.ad_banner);
};
