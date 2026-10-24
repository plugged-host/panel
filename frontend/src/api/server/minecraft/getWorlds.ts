// plugged.host: a Minecraft server's worlds (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftWorldsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/worlds`);
  return parseFromApi(minecraftWorldsSchema, data);
};
