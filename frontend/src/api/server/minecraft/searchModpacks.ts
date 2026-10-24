// plugged.host: one page of modpacks the server can run (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftSearchPageSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface ModpackSearchParams {
  source: z.infer<typeof minecraftSourceSchema>;
  query: string;
  page: number;
}

export default async (
  uuid: string,
  params: ModpackSearchParams,
): Promise<z.infer<typeof minecraftSearchPageSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/modpacks/search`, {
    params: { source: params.source, query: params.query || undefined, page: params.page },
  });
  return parseFromApi(minecraftSearchPageSchema, data);
};
