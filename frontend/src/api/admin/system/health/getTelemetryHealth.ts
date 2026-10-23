// plugged.host: Admin › Overview › Health: nodes whose telemetry sample is missing or old.
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

const telemetryHealthSchema = z.object({
  total: z.number(),
  staleAfterSeconds: z.number(),
  stale: z.array(
    z.object({
      uuid: z.string(),
      name: z.string(),
      sampled: z.coerce.date().nullable(),
    }),
  ),
});

export default async (): Promise<z.infer<typeof telemetryHealthSchema>> => {
  const { data } = await axiosInstance.get('/api/admin/system/health/telemetry');
  return parseFromApi(telemetryHealthSchema, data);
};
