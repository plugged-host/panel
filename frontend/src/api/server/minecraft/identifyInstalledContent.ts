// plugged.host: identify one installed plugin or mod again (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftInstalledJarSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, file: string): Promise<z.infer<typeof minecraftInstalledJarSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/content/installed/identify`, {
    file,
  });
  return parseFromApi(minecraftInstalledJarSchema, data.installed);
};
