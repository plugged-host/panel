import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminNodeDatabaseHostSchema } from '@/lib/schemas/admin/nodes.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  nodeUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminNodeDatabaseHostSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/nodes/${nodeUuid}/database-hosts`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminNodeDatabaseHostSchema, data.database_hosts);
};
