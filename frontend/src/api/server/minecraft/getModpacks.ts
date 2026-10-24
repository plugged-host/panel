// plugged.host: what the Modpacks tab offers and the install in progress (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftModpacksSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftModpacksSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/modpacks`);
  return parseFromApi(minecraftModpacksSchema, data);
};
