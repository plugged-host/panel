// plugged.host: a modpack's versions, newest first (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftSourceSchema, minecraftVersionSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  source: z.infer<typeof minecraftSourceSchema>,
  project: string,
): Promise<z.infer<typeof minecraftVersionSchema>[]> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/modpacks/versions`, {
    params: { source, project },
  });
  return parseFromApi(z.array(minecraftVersionSchema), data.versions);
};
