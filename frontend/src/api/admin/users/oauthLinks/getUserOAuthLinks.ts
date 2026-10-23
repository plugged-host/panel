import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminUserOAuthLinkSchema } from '@/lib/schemas/admin/users.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  userUuid: string,
  page: number,
  search?: string,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminUserOAuthLinkSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/users/${userUuid}/oauth-links`, {
    params: { page, per_page: perPage, search },
  });
  return parsePaginationFromApi(adminUserOAuthLinkSchema, data.oauth_links);
};
