// plugged.host: change a user's credits, tokens and memberships
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUserEconomySchema } from '@/lib/schemas/user/economy.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface UserEconomyChange {
  credits?: number;
  tokens?: number;
  reason?: string;
  // null ends it now
  plusUntil?: Date | null;
  plusplusUntil?: Date | null;
}

export default async (userUuid: string, change: UserEconomyChange): Promise<z.infer<typeof adminUserEconomySchema>> => {
  const { data } = await axiosInstance.put(`/api/admin/users/${userUuid}/economy`, {
    credits: change.credits,
    tokens: change.tokens,
    reason: change.reason,
    ...(change.plusUntil !== undefined ? { plus_until: change.plusUntil ? change.plusUntil.toISOString() : null } : {}),
    ...(change.plusplusUntil !== undefined
      ? { plusplus_until: change.plusplusUntil ? change.plusplusUntil.toISOString() : null }
      : {}),
  });
  return parseFromApi(adminUserEconomySchema, data.economy);
};
