// plugged.host: a parked server's state, cost and where it can go
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverLimboSchema } from '@/lib/schemas/server/limbo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (uuid: string): Promise<z.infer<typeof serverLimboSchema>> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/limbo`);
  return parseFromApi(serverLimboSchema, data.limbo);
};
