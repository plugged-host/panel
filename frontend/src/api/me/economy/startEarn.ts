// plugged.host: open a claim and get the sponsored link
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { earnProviderSchema, earnStartSchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (provider: z.infer<typeof earnProviderSchema>): Promise<z.infer<typeof earnStartSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/tokens/earn', { provider });
  return parseFromApi(earnStartSchema, data.earn);
};
