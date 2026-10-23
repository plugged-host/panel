// plugged.host: buy or extend the second membership
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { economyPaySchema, userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (pay: z.infer<typeof economyPaySchema>): Promise<z.infer<typeof userEconomySchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/membership/plusplus', { pay });
  return parseFromApi(userEconomySchema, data.economy);
};
