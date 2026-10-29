// plugged.host: the account's profile badges and what one costs
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userBadgesSchema } from '@/lib/schemas/user/badges.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userBadgesSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/badges');
  return parseFromApi(userBadgesSchema, data.badges);
};
