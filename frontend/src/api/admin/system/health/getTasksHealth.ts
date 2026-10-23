// plugged.host: Admin › Overview › Health: background task heartbeats.
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const tasksHealthSchema = z.object({
  primary: z.boolean(),
  tasks: z.array(
    z.object({
      name: z.string(),
      lastRun: z.coerce.date(),
      error: z.string().nullable(),
    }),
  ),
});

export default async (): Promise<z.infer<typeof tasksHealthSchema>> => {
  const { data } = await axiosInstance.get('/api/admin/system/health/tasks');
  return parseFromApi(tasksHealthSchema, data);
};
