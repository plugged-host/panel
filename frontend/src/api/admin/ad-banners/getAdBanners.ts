// plugged.host: Admin › Sponsored Banners
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminAdBannerSchema } from '@/lib/schemas/admin/adBanners.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminAdBannerSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/ad-banners', {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminAdBannerSchema, data.ad_banners);
};
