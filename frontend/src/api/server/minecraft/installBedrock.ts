// plugged.host: install or update Geyser + Floodgate and hold a Bedrock port (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftBedrockInstalledSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftBedrockInstalledSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/minecraft/bedrock`);
  return parseFromApi(minecraftBedrockInstalledSchema, data);
};
