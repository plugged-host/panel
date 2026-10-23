// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusOverrideSchema } from '@/lib/schemas/admin/status.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminStatusOverrideSchema>[]> => {
  const { data } = await axiosInstance.get('/api/admin/status/overrides');
  return parseFromApi(z.array(adminStatusOverrideSchema), data.overrides);
};
