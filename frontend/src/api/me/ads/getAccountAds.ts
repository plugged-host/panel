// plugged.host: whether this account sees ads, and the sponsored banners shown now
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { accountAdsSchema } from '@/lib/schemas/user/ads.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof accountAdsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/ads');
  return parseFromApi(accountAdsSchema, data);
};
