// plugged.host: buy the first membership with tokens, at the price shown (409 when it moved)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (price: number): Promise<z.infer<typeof userEconomySchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/membership/plus', { price });
  return parseFromApi(userEconomySchema, data.economy);
};
