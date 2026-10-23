// plugged.host: install one plugin or mod (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftInstalledSchema, minecraftInstallSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  data: z.infer<typeof minecraftInstallSchema>,
): Promise<z.infer<typeof minecraftInstalledSchema>> => {
  const { data: response } = await axiosInstance.post(
    `/api/client/servers/${uuid}/minecraft/content/install`,
    serializeForApi(minecraftInstallSchema, data),
  );
  return parseFromApi(minecraftInstalledSchema, response.installed);
};
