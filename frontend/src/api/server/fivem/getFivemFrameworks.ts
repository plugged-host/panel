// plugged.host: txAdmin's recipes for a FiveM server and its install, if any (docs/features/fivem.md#frameworks)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { fivemFrameworksSchema } from '@/lib/schemas/server/fivem.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof fivemFrameworksSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/fivem/frameworks`);
  return parseFromApi(fivemFrameworksSchema, data);
};
