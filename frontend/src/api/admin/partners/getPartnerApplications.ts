// plugged.host: Admin › Partners (docs/features/partners.md)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminPartnerApplicationSchema, adminPartnerFilter } from '@/lib/schemas/admin/partners.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  page: number,
  search?: string,
  perPage?: number,
  status?: z.infer<typeof adminPartnerFilter> | null,
): Promise<Pagination<z.infer<typeof adminPartnerApplicationSchema>>> => {
  const { data } = await axiosInstance.get('/api/admin/partners', {
    params: { page, per_page: perPage, search, status: status ?? undefined },
  });
  return parsePaginationFromApi(adminPartnerApplicationSchema, data.applications);
};
