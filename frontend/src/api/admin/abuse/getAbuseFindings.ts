// plugged.host: Admin › Abuse
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { abuseFindingStatus, adminAbuseFindingSchema } from '@/lib/schemas/admin/abuse.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
  status?: z.infer<typeof abuseFindingStatus> | null,
): Promise<Pagination<z.infer<typeof adminAbuseFindingSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/abuse/findings', {
    params: { page, per_page: perPage, search, status: status ?? undefined },
  });
  return parsePaginationFromApi(adminAbuseFindingSchema, data.findings);
};
