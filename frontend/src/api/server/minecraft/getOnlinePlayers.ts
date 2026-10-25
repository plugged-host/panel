// plugged.host: who is on the server right now, from the console's `list` (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftOnlineSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftOnlineSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/players/online`);
  return parseFromApi(minecraftOnlineSchema, data);
};
