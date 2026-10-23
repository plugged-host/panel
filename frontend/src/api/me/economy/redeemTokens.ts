// plugged.host: settle a claim the player came back with
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { redemptionSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (claim: string, params: Record<string, string>): Promise<z.infer<typeof redemptionSchema>> => {
  // sent as they came: the provider's parameter names must not be re-cased
  const { data } = await axiosInstance.post('/api/client/account/tokens/redeem', { claim, params });
  return parseFromApi(redemptionSchema, data.redemption);
};
