// plugged.host: Admin › Status
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminStatusIncidentPostUpdateSchema } from '@/lib/schemas/admin/status.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (
  incidentUuid: string,
  data: z.infer<typeof adminStatusIncidentPostUpdateSchema>,
): Promise<void> => {
  await axiosInstance.post(
    `/api/admin/status/incidents/${incidentUuid}/updates`,
    serializeForApi(adminStatusIncidentPostUpdateSchema, data),
  );
};
