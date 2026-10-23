// plugged.host: the Tokens page
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userTokensSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userTokensSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/tokens');
  return parseFromApi(userTokensSchema, data.tokens);
};
