// plugged.host: what the create page offers; with a size, which locations have room for it
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { ServerCreateSize, serverCreateOptionsSchema } from '@/lib/schemas/server/create.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (size?: ServerCreateSize): Promise<z.infer<typeof serverCreateOptionsSchema>> => {
  const { data } = await axiosInstance.get('/api/client/servers/create', { params: size });
  return parseFromApi(serverCreateOptionsSchema, data.options);
};
