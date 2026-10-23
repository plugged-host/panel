// plugged.host: a player creating a server
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCreateSchema } from '@/lib/schemas/server/create.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (request: z.infer<typeof serverCreateSchema>): Promise<z.infer<typeof serverSchema>> => {
  const { data } = await axiosInstance.post('/api/client/servers', serializeForApi(serverCreateSchema, request));
  return parseFromApi(serverSchema, data.server);
};
