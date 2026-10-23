import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerDatabaseSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  databaseHostUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminServerDatabaseSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/database-hosts/${databaseHostUuid}/databases`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminServerDatabaseSchema, data.databases);
};
