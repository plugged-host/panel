// plugged.host: Admin › Plans
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPlanSchema, adminPlanUpdateSchema } from '@/lib/schemas/admin/plans.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';

export default async (planData: z.infer<typeof adminPlanUpdateSchema>): Promise<z.infer<typeof adminPlanSchema>> => {
  const { data } = await axiosInstance.post('/api/admin/plans', serializeForApi(adminPlanUpdateSchema, planData));
  return parseFromApi(adminPlanSchema, data.plan);
};
