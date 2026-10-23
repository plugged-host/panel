import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeDeviceSchema } from '@/lib/schemas/admin/nodes.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  nestUuid: string,
  eggUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminNodeDeviceSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/nests/${nestUuid}/eggs/${eggUuid}/devices`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminNodeDeviceSchema, data.devices);
};
