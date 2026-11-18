// plugged.host: a paid plan ordered here, its first month paid with noprofit.space Funds (docs/features/dedicated.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCreateDedicatedOrderSchema } from '@/lib/schemas/server/create.ts';
import { serverSchema } from '@/lib/schemas/server/server.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  request: z.infer<typeof serverCreateDedicatedOrderSchema>,
): Promise<z.infer<typeof serverSchema>> => {
  const { data } = await axiosInstance.post(
    '/api/client/servers/create/dedicated',
    serializeForApi(serverCreateDedicatedOrderSchema, request),
  );
  return parseFromApi(serverSchema, data.server);
};
