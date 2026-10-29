// plugged.host: show one of the account's badges next to its name, or none
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userBadgesSchema } from '@/lib/schemas/user/badges.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (badge: string | null): Promise<z.infer<typeof userBadgesSchema>> => {
  const { data } = await axiosInstance.put('/api/client/account/badges', { badge });
  return parseFromApi(userBadgesSchema, data.badges);
};
