// plugged.host: the installed plugins or mods, identifying a few more per call (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftInstalledListSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftInstalledListSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/content/installed`);
  return parseFromApi(minecraftInstalledListSchema, data);
};
