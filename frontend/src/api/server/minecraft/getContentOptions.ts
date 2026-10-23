// plugged.host: what the Content page may offer for the server (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftContentOptionsSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftContentOptionsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/content`);
  return parseFromApi(minecraftContentOptionsSchema, data.content);
};
