// plugged.host: the Discord bots' state, as the primary instance last stored it (docs/features/discord.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { discordBotStatusSchema } from '@/lib/schemas/admin/discord.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof discordBotStatusSchema>[] | null> => {
  const { data } = await axiosInstance.get('/api/admin/discord/status');
  return data.bots === null ? null : parseFromApi(z.array(discordBotStatusSchema), data.bots);
};
