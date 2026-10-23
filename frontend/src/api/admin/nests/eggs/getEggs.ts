import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminEggSchema } from '@/lib/schemas/admin/eggs.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  nestUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminEggSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/nests/${nestUuid}/eggs`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminEggSchema, data.eggs);
};
