// plugged.host: the Discord rewards page (docs/features/discord.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { discordRewardsSchema } from '@/lib/schemas/user/discord.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof discordRewardsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/account/discord');
  return parseFromApi(discordRewardsSchema, data.discord);
};
