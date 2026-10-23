// plugged.host: a project's builds that fit the server (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftKindSchema, minecraftSourceSchema, minecraftVersionsSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  uuid: string,
  source: z.infer<typeof minecraftSourceSchema>,
  project: string,
  kind: z.infer<typeof minecraftKindSchema>,
): Promise<z.infer<typeof minecraftVersionsSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/content/versions`, {
    params: { source, project, kind },
  });
  return parseFromApi(minecraftVersionsSchema, data);
};
