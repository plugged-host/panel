// plugged.host: asks Discord for new rewards now, once a minute (docs/features/discord.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { discordRewardsSchema } from '@/lib/schemas/user/discord.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof discordRewardsSchema>> => {
  const { data } = await axiosInstance.post('/api/client/account/discord/check');
  return parseFromApi(discordRewardsSchema, data.discord);
};
