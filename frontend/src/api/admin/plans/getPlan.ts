// plugged.host: Admin › Plans
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPlanSchema } from '@/lib/schemas/admin/plans.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (planUuid: string): Promise<z.infer<typeof adminPlanSchema>> => {
  const { data } = await axiosInstance.get(`/api/admin/plans/${planUuid}`);
  return parseFromApi(adminPlanSchema, data.plan);
};
