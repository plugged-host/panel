// plugged.host: buy steps of a resource at the price shown
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { storeResourceSchema, userStoreSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  resource: z.infer<typeof storeResourceSchema>,
  steps: number,
  price: number,
): Promise<z.infer<typeof userStoreSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/credits/store', { resource, steps, price });
  return parseFromApi(userStoreSchema, data.store);
};
