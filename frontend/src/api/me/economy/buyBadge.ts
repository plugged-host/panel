// plugged.host: buy a profile badge with tokens, at the price shown (409 when it moved or it is owned)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userBadgesSchema } from '@/lib/schemas/user/badges.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (badge: string, price: number): Promise<z.infer<typeof userBadgesSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/badges', { badge, price });
  return parseFromApi(userBadgesSchema, data.badges);
};
