// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusIncidentCreateSchema } from '@/lib/schemas/admin/status.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminStatusIncidentCreateSchema>): Promise<void> => {
  await axiosInstance.post('/api/admin/status/incidents', serializeForApi(adminStatusIncidentCreateSchema, data));
};
