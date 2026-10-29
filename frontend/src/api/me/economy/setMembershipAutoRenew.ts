// plugged.host: switch a membership's automatic renewal on or off
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { membershipKindSchema, userEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  membership: z.infer<typeof membershipKindSchema>,
  enabled: boolean,
): Promise<z.infer<typeof userEconomySchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/membership/auto-renew', { membership, enabled });
  return parseFromApi(userEconomySchema, data.economy);
};
