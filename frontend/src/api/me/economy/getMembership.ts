// plugged.host: the account's balances and memberships
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userEconomySchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/membership');
  return parseFromApi(userEconomySchema, data.economy);
};
