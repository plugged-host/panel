// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusOverrideCreateSchema } from '@/lib/schemas/admin/status.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (data: z.infer<typeof adminStatusOverrideCreateSchema>): Promise<void> => {
  await axiosInstance.post('/api/admin/status/overrides', serializeForApi(adminStatusOverrideCreateSchema, data));
};
