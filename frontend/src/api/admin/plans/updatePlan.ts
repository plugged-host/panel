// plugged.host: Admin › Plans
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPlanUpdateSchema } from '@/lib/schemas/admin/plans.ts';
import { serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (planUuid: string, data: z.infer<typeof adminPlanUpdateSchema>): Promise<void> => {
  await axiosInstance.patch(`/api/admin/plans/${planUuid}`, serializeForApi(adminPlanUpdateSchema, data));
};
