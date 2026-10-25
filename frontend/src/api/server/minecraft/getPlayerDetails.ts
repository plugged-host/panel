// plugged.host: one player's saved data and statistics (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftPlayerDetailsSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, player: string): Promise<z.infer<typeof minecraftPlayerDetailsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/players/${player}`);
  return parseFromApi(minecraftPlayerDetailsSchema, data);
};
