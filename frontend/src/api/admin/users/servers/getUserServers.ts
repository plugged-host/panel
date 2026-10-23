import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminServerSchema } from '@/lib/schemas/admin/servers.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  userUuid: string,
  page: number,
  search?: string,
  owned?: boolean,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminServerSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/users/${userUuid}/servers`, {
    params: { page, per_page: perPage, search, owned },
  });
  return parsePaginationFromApi(adminServerSchema, data.servers);
};
