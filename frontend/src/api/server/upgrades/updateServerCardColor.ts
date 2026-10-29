// plugged.host: changes the colour of the server's card, free once bought; null takes it off
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverUpgradesSchema } from '@/lib/schemas/server/upgrades.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string, color: string | null): Promise<z.infer<typeof serverUpgradesSchema>> => {
  const { data } = await axiosInstance.put(`/api/client/servers/${uuid}/upgrades/color`, { color });
  return parseFromApi(serverUpgradesSchema, data.upgrades);
};
