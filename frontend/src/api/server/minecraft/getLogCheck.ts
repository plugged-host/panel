// plugged.host: the console's automatic log check on mclo.gs (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftLogCheckResponseSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftLogCheckResponseSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/log-check`);
  return parseFromApi(minecraftLogCheckResponseSchema, data);
};
