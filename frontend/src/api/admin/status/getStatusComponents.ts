// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusComponentSchema } from '@/lib/schemas/admin/status.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminStatusComponentSchema>[]> => {
  const { data } = await axiosInstance.get('/api/admin/status/components');
  return parseFromApi(z.array(adminStatusComponentSchema), data.components);
};
