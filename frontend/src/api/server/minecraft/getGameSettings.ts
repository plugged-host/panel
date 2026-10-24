// plugged.host: the server's settings files as fields (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftConfigSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftConfigSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/settings`);
  return parseFromApi(minecraftConfigSchema, data);
};
