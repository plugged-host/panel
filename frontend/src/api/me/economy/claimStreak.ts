// plugged.host: claim today's streak tokens (docs/features/economy.md#daily-streak)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { streakClaimSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof streakClaimSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/tokens/streak');
  return parseFromApi(streakClaimSchema, data.claim);
};
