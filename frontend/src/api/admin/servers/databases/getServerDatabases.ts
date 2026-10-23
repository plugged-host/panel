import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerServerDatabaseSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  serverUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminServerServerDatabaseSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/servers/${serverUuid}/databases`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminServerServerDatabaseSchema, data.databases);
};
