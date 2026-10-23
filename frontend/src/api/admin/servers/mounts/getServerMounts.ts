import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerMountSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  serverUuid: string,
  page: number,
  search?: string,
  perPage = 100,
): Promise<Pagination<z.infer<typeof adminServerMountSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/servers/${serverUuid}/mounts`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminServerMountSchema, data.mounts);
};
