// plugged.host: buy a membership for another account, by its username or email, at the price shown
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { membershipKindSchema, userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  recipient: string,
  membership: z.infer<typeof membershipKindSchema>,
  price: number,
): Promise<z.infer<typeof userEconomySchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/membership/gift', { recipient, membership, price });
  return parseFromApi(userEconomySchema, data.economy);
};
