// plugged.host: the paid plans with CPU threads and memory of their own (docs/features/dedicated.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverCreateDedicatedPlansSchema } from '@/lib/schemas/server/create.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof serverCreateDedicatedPlansSchema>> => {
  const { data } = await axiosInstance.get('/api/client/servers/create/dedicated');
  return parseFromApi(serverCreateDedicatedPlansSchema, data);
};
