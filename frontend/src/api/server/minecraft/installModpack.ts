// plugged.host: start installing a modpack, which reinstalls the server (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftModpackInstallSchema, minecraftSourceSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface ModpackInstall {
  source: z.infer<typeof minecraftSourceSchema>;
  project: string;
  version: string;
  clean: boolean;
}

export default async (
  uuid: string,
  install: ModpackInstall,
): Promise<z.infer<typeof minecraftModpackInstallSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/modpacks/install`, install);
  return parseFromApi(minecraftModpackInstallSchema, data);
};
