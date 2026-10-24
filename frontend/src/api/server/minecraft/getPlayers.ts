// plugged.host: the whitelist, operators and bans (docs/features/minecraft.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { minecraftRosterSchema } from '@/lib/schemas/server/minecraft.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof minecraftRosterSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/minecraft/players`);
  return parseFromApi(minecraftRosterSchema, data);
};
