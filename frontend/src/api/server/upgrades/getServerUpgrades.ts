// plugged.host: what can be bought for the server, and what runs (docs/features/server-upgrades.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverUpgradesSchema } from '@/lib/schemas/server/upgrades.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverUpgradesSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/upgrades`);
  return parseFromApi(serverUpgradesSchema, data.upgrades);
};
