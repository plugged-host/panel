// plugged.host: Admin › Plans
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPlanSchema } from '@/lib/schemas/admin/plans.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminPlanSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/plans', {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminPlanSchema, data.plans);
};
