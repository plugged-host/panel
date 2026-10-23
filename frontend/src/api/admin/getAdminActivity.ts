import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { activitySchema } from '@/lib/schemas/activity.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  userUuid: string | null,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof activitySchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/activity', {
    params: { user: userUuid, page, per_page: perPage, search },
  });
  return parsePaginationFromApi(activitySchema, data.activities);
};
