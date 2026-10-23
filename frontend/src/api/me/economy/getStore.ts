// plugged.host: the Resource Store
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userStoreSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userStoreSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/credits');
  return parseFromApi(userStoreSchema, data.store);
};
