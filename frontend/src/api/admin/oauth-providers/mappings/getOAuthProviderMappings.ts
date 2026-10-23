import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { adminOAuthProviderMappingSchema } from '@/lib/schemas/admin/oauthProviders.ts';
import { parsePaginationFromApi } from '@/lib/serialization/api-transform.ts';

export default async (
  oauthProviderUuid: string,
  page: number,
  perPage?: number,
): Promise<Pagination<z.infer<typeof adminOAuthProviderMappingSchema>>> => {
  const { data } = await axiosInstance.get(`/api/admin/oauth-providers/${oauthProviderUuid}/mappings`, {
    params: { page, per_page: perPage },
  });
  return parsePaginationFromApi(adminOAuthProviderMappingSchema, data.oauth_mappings);
};
