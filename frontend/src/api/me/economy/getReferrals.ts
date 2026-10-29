// plugged.host: the account's referral link and friends (docs/features/referrals.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userReferralsSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userReferralsSchema> | null> => {
  const { data } = await axiosInstance.get('/api/client/account/tokens/referrals');
  return data.referrals ? parseFromApi(userReferralsSchema, data.referrals) : null;
};
