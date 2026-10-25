// plugged.host: one page of modpacks the server can run (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import {
  minecraftLoaderSchema,
  minecraftSearchPageSchema,
  minecraftSourceSchema,
} from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface ModpackSearchParams {
  source: z.infer<typeof minecraftSourceSchema>;
  query: string;
  page: number;
  // plugged.host: the Plugins & Mods page's version filter and the setup's chosen loader
  gameVersion?: string | null;
  loader?: z.infer<typeof minecraftLoaderSchema> | null;
}

export default async (
  uuid: string,
  params: ModpackSearchParams,
): Promise<z.infer<typeof minecraftSearchPageSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/modpacks/search`, {
    params: {
      source: params.source,
      query: params.query || undefined,
      game_version: params.gameVersion || undefined,
      loader: params.loader || undefined,
      page: params.page,
    },
  });
  return parseFromApi(minecraftSearchPageSchema, data);
};
