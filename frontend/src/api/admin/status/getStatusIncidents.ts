// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusIncidentSchema } from '@/lib/schemas/admin/status.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof adminStatusIncidentSchema>[]> => {
  const { data } = await axiosInstance.get('/api/admin/status/incidents');
  return parseFromApi(z.array(adminStatusIncidentSchema), data.incidents);
};
