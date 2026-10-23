// plugged.host: one page of plugins, mods or modpacks (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftKindSchema, minecraftSearchSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface ContentSearchParams {
  source: z.infer<typeof minecraftSourceSchema>;
  kind: z.infer<typeof minecraftKindSchema> | null;
  query: string;
  // null: the server's version; '': any version
  gameVersion: string | null;
  page: number;
}

export default async (uuid: string, params: ContentSearchParams): Promise<z.infer<typeof minecraftSearchSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/content/search`, {
    params: {
      source: params.source,
      kind: params.kind ?? undefined,
      query: params.query || undefined,
      game_version: params.gameVersion ?? undefined,
      page: params.page,
    },
  });
  return parseFromApi(minecraftSearchSchema, data);
};
