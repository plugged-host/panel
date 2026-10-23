// plugged.host: the sidebar's card: allowance and balances
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userBalanceSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userBalanceSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/balance');
  return parseFromApi(userBalanceSchema, data.balance);
};
