// plugged.host: change a user's credits, tokens and memberships
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUserEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface UserEconomyChange {
  credits?: number;
  tokens?: number;
  reason?: string;
  plus?: boolean;
  // null ends it now
  plusplusUntil?: Date | null;
}

export default async (userUuid: string, change: UserEconomyChange): Promise<z.infer<typeof adminUserEconomySchema>> => {
  const { data } = await axiosInstance.put(`/api/admin/users/${userUuid}/economy`, {
    credits: change.credits,
    tokens: change.tokens,
    reason: change.reason,
    plus: change.plus,
    ...(change.plusplusUntil !== undefined
      ? { plusplus_until: change.plusplusUntil ? change.plusplusUntil.toISOString() : null }
      : {}),
  });
  return parseFromApi(adminUserEconomySchema, data.economy);
};
