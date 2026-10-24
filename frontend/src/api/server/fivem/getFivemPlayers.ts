// plugged.host: who is on a FiveM server now (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { fivemPlayersSchema } from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof fivemPlayersSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/fivem/players`);
  return parseFromApi(fivemPlayersSchema, data);
};
