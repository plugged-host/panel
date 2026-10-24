// plugged.host: Bedrock support (Geyser + Floodgate) on a Minecraft server (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftBedrockSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftBedrockSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/bedrock`);
  return parseFromApi(minecraftBedrockSchema, data);
};
