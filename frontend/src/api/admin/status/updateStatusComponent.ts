// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusComponentUpdateSchema } from '@/lib/schemas/admin/status.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (componentUuid: string, data: z.infer<typeof adminStatusComponentUpdateSchema>): Promise<void> => {
  await axiosInstance.patch(
    `/api/admin/status/components/${componentUuid}`,
    serializeForApi(adminStatusComponentUpdateSchema, data),
  );
};
