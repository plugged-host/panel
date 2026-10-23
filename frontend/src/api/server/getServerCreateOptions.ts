// plugged.host: what the create page offers
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCreateOptionsSchema } from '@/lib/schemas/server/create.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof serverCreateOptionsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/servers/create');
  return parseFromApi(serverCreateOptionsSchema, data.options);
};
