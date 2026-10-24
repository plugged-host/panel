// plugged.host: the server.cfg txAdmin runs, as a form (docs/features/fivem.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { fivemConfigSchema } from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof fivemConfigSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/fivem/config`);
  return parseFromApi(fivemConfigSchema, data);
};
