// plugged.host: a user's credits, tokens and memberships
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUserEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (userUuid: string): Promise<z.infer<typeof adminUserEconomySchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/users/${userUuid}/economy`);
  return parseFromApi(adminUserEconomySchema, data.economy);
};
