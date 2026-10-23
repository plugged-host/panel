import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userOAuthLinkSchema } from '@/lib/schemas/user/oAuth.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (page: number, perPage?: number): Promise<Pagination<z.infer<typeof userOAuthLinkSchema>>> => {
  const { data } = await axiosInstance.get('/api/client/account/oauth-links', {
    params: { page, per_page: perPage },
  });
  return parsePaginationFromApi(userOAuthLinkSchema, data.oauth_links);
};
